//! CPUクライアント
//!
//! ServerEvent を受信して ClientAction を返す。
//! プレイヤーと全く同じプロトコルでサーバとやり取りする。

use mahjong_core::hand::Hand;
use mahjong_core::hand_info::hand_analyzer::calc_shanten_number;
use mahjong_core::hand_info::meld::{Meld, MeldFrom, MeldType};
use mahjong_core::tile::Tile;

use crate::protocol::{AvailableCall, ClientAction, ServerEvent};

use super::evaluator;
use super::state::CpuGameState;

/// CPUの強さレベル
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CpuLevel {
    /// 初心者: 向聴数のみ考慮、防御なし、ミスあり
    Weak,
    /// 中級者: 有効牌数考慮、基本防御
    Normal,
    /// 上級者: 打点考慮、筋/壁/現物の高度な防御
    Strong,
}

impl CpuLevel {
    /// 有効牌数を考慮するか
    pub fn uses_acceptance_count(&self) -> bool {
        matches!(self, CpuLevel::Normal | CpuLevel::Strong)
    }

    /// 打点推定を使うか
    pub fn uses_value_estimation(&self) -> bool {
        matches!(self, CpuLevel::Strong)
    }

    /// 防御戦略を使うか
    pub fn uses_defense(&self) -> bool {
        matches!(self, CpuLevel::Normal | CpuLevel::Strong)
    }

    /// ミスをするか（最善手以外を選ぶ可能性）
    pub fn should_make_mistake(&self) -> bool {
        matches!(self, CpuLevel::Weak)
    }
}

/// CPUの性格（攻撃スタイル）
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CpuPersonality {
    /// バランス型
    Balanced,
    /// 速攻型（タンヤオ・ピンフ等の早い和了り重視）
    Speedy,
    /// 高打点型（面前リーチ・役牌・ドラ重視）
    HighValue,
    /// 守備型（安全打優先、放銃回避重視）
    Defensive,
}

/// 性格ごとのパラメータ
#[derive(Debug, Clone)]
pub struct PersonalityParams {
    /// 鳴き積極度（0.0=鳴かない, 1.0=積極的に鳴く）
    pub call_aggressiveness: f64,
    /// 打点重み（高いほど打点を重視）
    pub value_weight: f64,
    /// 速度重み（高いほど早い和了りを重視）
    pub speed_weight: f64,
    /// 撤退閾値（高いほど早く撤退する）
    pub retreat_threshold: f64,
    /// リーチ積極度（0.0=リーチしない, 1.0=積極的にリーチ）
    pub riichi_aggressiveness: f64,
}

/// CPU設定（性格と強さの組み合わせ）
#[derive(Debug, Clone)]
pub struct CpuConfig {
    /// 強さレベル
    pub level: CpuLevel,
    /// 性格
    pub personality: CpuPersonality,
    /// 性格パラメータ
    pub params: PersonalityParams,
}

impl CpuConfig {
    /// 指定した強さと性格で設定を作成する
    pub fn new(level: CpuLevel, personality: CpuPersonality) -> Self {
        let params = PersonalityParams::from_personality(personality);
        CpuConfig {
            level,
            personality,
            params,
        }
    }
}

/// CPUクライアント: ServerEvent を処理して ClientAction を返す
pub struct CpuClient {
    /// CPU設定
    pub config: CpuConfig,
    /// ゲーム状態（イベントから構築）
    pub state: CpuGameState,
}

impl CpuClient {
    /// 新しいCPUクライアントを作成する
    pub fn new(config: CpuConfig) -> Self {
        CpuClient {
            config,
            state: CpuGameState::new(),
        }
    }

    /// ServerEvent を処理し、必要なら ClientAction を返す
    ///
    /// CPUはこのメソッドだけでサーバとやり取りする。
    /// 人間プレイヤーが画面を見て操作するのと同様に、
    /// イベントから情報を得て判断する。
    pub fn handle_event(&mut self, event: &ServerEvent) -> Option<ClientAction> {
        // 1. イベントに応じて内部状態を更新
        self.state.update(event);

        // 2. アクションが必要なイベントなら判断して返す
        match event {
            ServerEvent::TileDrawn { .. } => Some(self.decide_on_draw()),
            ServerEvent::CallAvailable { .. } => Some(self.decide_call()),
            ServerEvent::HandUpdated { .. } => {
                if self.state.need_discard_after_call {
                    self.state.need_discard_after_call = false;
                    Some(self.decide_discard_after_call())
                } else {
                    None
                }
            }
            ServerEvent::NineTerminalsAvailable => Some(self.decide_nine_terminals()),
            _ => None,
        }
    }

    /// ツモ後の判断（ツモ和了/リーチ/カン/打牌）
    fn decide_on_draw(&self) -> ClientAction {
        // ツモ和了可能なら常に和了する
        if self.state.can_tsumo {
            return ClientAction::Tsumo;
        }

        // リーチ中はツモ切りのみ
        if self.state.is_riichi {
            return ClientAction::Discard { tile: None };
        }

        // リーチ可能か検討
        if self.state.can_riichi && self.should_riichi() {
            let tile = self.select_riichi_tile();
            return ClientAction::Riichi { tile };
        }

        // 暗カン検討
        if let Some(kan_action) = self.consider_ankan() {
            return kan_action;
        }

        // 打牌選択
        self.decide_discard()
    }

    /// 打牌を選択する
    fn decide_discard(&self) -> ClientAction {
        let candidates = evaluator::evaluate_discards(&self.state, &self.config);

        let attacking = self.should_attack();
        if let Some(tile) = evaluator::select_best_discard(&candidates, &self.config, attacking) {
            // ツモ牌と同じならツモ切り
            if self.state.my_drawn == Some(tile) {
                ClientAction::Discard { tile: None }
            } else {
                ClientAction::Discard { tile: Some(tile) }
            }
        } else {
            // フォールバック: ツモ切り
            ClientAction::Discard { tile: None }
        }
    }

    /// 鳴き後の打牌を選択する
    fn decide_discard_after_call(&self) -> ClientAction {
        // 鳴き後はツモ牌がないので、手牌から選ぶ
        let candidates = evaluator::evaluate_discards(&self.state, &self.config);
        let attacking = self.should_attack();

        if let Some(tile) = evaluator::select_best_discard(&candidates, &self.config, attacking) {
            ClientAction::Discard { tile: Some(tile) }
        } else if let Some(&tile) = self.state.my_hand.last() {
            ClientAction::Discard { tile: Some(tile) }
        } else {
            ClientAction::Discard { tile: None }
        }
    }

    /// 鳴き判断（ロン/ポン/チー/パス）
    fn decide_call(&self) -> ClientAction {
        let calls = &self.state.pending_calls;

        // ロン可能なら常に和了する
        if calls.iter().any(|c| matches!(c, AvailableCall::Ron)) {
            return ClientAction::Ron;
        }

        // ポン判断
        for call in calls {
            if let AvailableCall::Pon { options } = call {
                if self.should_pon() {
                    // 赤ドラを含む組み合わせを優先する
                    let tiles = options
                        .iter()
                        .find(|o| o[0].is_red_dora() || o[1].is_red_dora())
                        .copied()
                        .unwrap_or(options[0]);
                    return ClientAction::Pon { tiles };
                }
                break;
            }
        }

        // 大明カン判断
        if calls.iter().any(|c| matches!(c, AvailableCall::Daiminkan)) {
            // 基本的にはパス（ドラ増加リスクがある）
            // Strongレベルかつ高打点型のみ検討
            if self.config.level == CpuLevel::Strong
                && self.config.personality == CpuPersonality::HighValue
            {
                // 簡易判断: パスしておく（将来拡張可能）
            }
        }

        // チー判断
        for call in calls {
            if let AvailableCall::Chi { options } = call
                && let Some(tiles) = self.select_chi_option(options)
            {
                return ClientAction::Chi { tiles };
            }
        }

        ClientAction::Pass
    }

    /// リーチすべきか判断する
    fn should_riichi(&self) -> bool {
        let params = &self.config.params;

        // リーチ積極度に基づく基本判断
        // 簡易的に: 積極度 0.5 以上ならリーチ
        // ただしリーチ者が既にいる場合は慎重に
        let riichi_count = self.state.player_riichi.iter().filter(|&&r| r).count();

        if riichi_count >= 2 && params.riichi_aggressiveness < 0.8 {
            return false;
        }

        if riichi_count >= 1 && params.riichi_aggressiveness < 0.4 {
            return false;
        }

        // 残り山が少ない場合はリーチしない
        if self.state.remaining_tiles < 10 && params.riichi_aggressiveness < 0.9 {
            return false;
        }

        params.riichi_aggressiveness >= 0.4
    }

    /// リーチ宣言牌を選ぶ
    fn select_riichi_tile(&self) -> Option<Tile> {
        // テンパイを維持する牌を選ぶ
        let mut all_tiles = self.state.my_hand.clone();
        if let Some(drawn) = self.state.my_drawn {
            all_tiles.push(drawn);
        }

        let mut best: Option<(Tile, f64)> = None;

        for (i, &tile) in all_tiles.iter().enumerate() {
            let mut remaining: Vec<Tile> = all_tiles.clone();
            remaining.remove(i);

            // 捨てた後にテンパイを維持するか
            let hand = Hand::new(remaining, None);
            let shanten = calc_shanten_number(&hand);

            if shanten.is_ready() {
                // 安全度で比較
                let safety = super::defense::evaluate_safety(tile, &self.state);
                let is_better = match best {
                    Some((_, best_safety)) => safety > best_safety,
                    None => true,
                };
                if is_better {
                    best = Some((tile, safety));
                }
            }
        }

        best.and_then(|(tile, _)| {
            // ツモ牌ならNone（ツモ切りリーチ）
            if self.state.my_drawn == Some(tile) {
                None
            } else {
                Some(tile)
            }
        })
        // Note: この返り値はOption<Tile>で、Noneの場合ツモ切りリーチ
    }

    /// 暗カンを検討する
    fn consider_ankan(&self) -> Option<ClientAction> {
        let mut all_tiles = self.state.my_hand.clone();
        if let Some(drawn) = self.state.my_drawn {
            all_tiles.push(drawn);
        }

        // 4枚揃っている牌種を探す
        let mut counts = [0u8; 34];
        for tile in &all_tiles {
            counts[tile.get() as usize] += 1;
        }

        for (tile_type, &count) in counts.iter().enumerate() {
            if count == 4 {
                // 暗カンしてもテンパイが崩れないか確認
                // （簡易的に: Strong以外は常にカン、Strongは向聴数を確認）
                if self.config.level == CpuLevel::Strong {
                    // 暗カン後の手牌で向聴数を確認（簡易チェック）
                    let remaining: Vec<Tile> = all_tiles
                        .iter()
                        .filter(|t| t.get() != tile_type as u32)
                        .copied()
                        .collect();
                    let hand = Hand::new(remaining, None);
                    if !calc_shanten_number(&hand).is_ready_or_won() {
                        continue; // テンパイが崩れるのでカンしない
                    }
                }
                return Some(ClientAction::Kan {
                    tile_index: tile_type,
                });
            }
        }

        None
    }

    /// 攻撃続行か撤退かを判断する
    fn should_attack(&self) -> bool {
        let params = &self.config.params;

        // 自分の向聴数を計算
        let mut all_tiles = self.state.my_hand.clone();
        if let Some(drawn) = self.state.my_drawn {
            all_tiles.push(drawn);
        }
        let hand = Hand::new(all_tiles, None);
        let shanten = calc_shanten_number(&hand);

        // テンパイなら基本的に攻撃
        if shanten.is_ready_or_won() {
            return true;
        }

        // リーチ者の数
        let riichi_count = self.state.player_riichi.iter().filter(|&&r| r).count();

        // 防御を使わないレベルなら常に攻撃
        if !self.config.level.uses_defense() {
            return true;
        }

        // 撤退判断
        // 2人以上リーチ → 撤退寄り
        if riichi_count >= 2 && shanten >= 2 {
            return params.retreat_threshold < 0.3;
        }

        // 1人リーチ + 自分が2向聴以上 → 性格次第
        if riichi_count >= 1 && shanten >= 2 {
            return params.retreat_threshold < 0.5;
        }

        // 残り山が少ない + 向聴数が高い → 撤退
        if self.state.remaining_tiles < 15 && shanten >= 2 {
            return params.retreat_threshold < 0.4;
        }

        true
    }

    /// 九種九牌を宣言すべきか判断する
    ///
    /// 高打点型は国士無双を狙うため続行、それ以外は流局する。
    fn decide_nine_terminals(&self) -> ClientAction {
        let declare = self.config.personality != CpuPersonality::HighValue;
        ClientAction::NineTerminals { declare }
    }

    /// ポンすべきか判断する
    fn should_pon(&self) -> bool {
        let params = &self.config.params;
        let called_tile = match self.state.pending_call_tile {
            Some(t) => t,
            None => return false,
        };

        // Weakレベル: 向聴数が下がるなら常に鳴く
        if self.config.level == CpuLevel::Weak {
            return self.call_reduces_shanten_pon(called_tile);
        }

        // 鳴き積極度が低ければパス
        if params.call_aggressiveness < 0.3 {
            return false;
        }

        // 向聴数が下がるか
        if !self.call_reduces_shanten_pon(called_tile) {
            return false;
        }

        // 鳴いた後に役がありそうか（簡易チェック）
        let tt = called_tile.get();

        // 役牌のポンは積極的に
        if is_yakuhai(tt, self.state.my_seat_wind, self.state.prevailing_wind) {
            return true;
        }

        // タンヤオ志向: 中張牌のポンは積極的
        if self.config.personality == CpuPersonality::Speedy && is_tanyao_tile(tt) {
            return params.call_aggressiveness >= 0.5;
        }

        // 高打点志向: 門前維持を重視するのでポンは控えめ
        if self.config.personality == CpuPersonality::HighValue {
            return false;
        }

        params.call_aggressiveness >= 0.5
    }

    /// チーの選択肢から最適なものを選ぶ（鳴くべきでなければNone）
    fn select_chi_option(&self, options: &[[Tile; 2]]) -> Option<[Tile; 2]> {
        let params = &self.config.params;

        // 高打点志向は基本的にチーしない
        if self.config.personality == CpuPersonality::HighValue {
            return None;
        }

        // 鳴き積極度が低ければパス
        if params.call_aggressiveness < 0.4 {
            return None;
        }

        let called_tile = self.state.pending_call_tile?;

        // 各選択肢で向聴数が下がるか確認
        for &opt in options {
            if self.call_reduces_shanten_chi(called_tile, opt) {
                // Speedy型は積極的にチー
                if self.config.personality == CpuPersonality::Speedy {
                    return Some(opt);
                }
                // 他の型は鳴き積極度で判断
                if params.call_aggressiveness >= 0.5 {
                    return Some(opt);
                }
            }
        }

        None
    }

    /// 既存の副露を取得する
    fn build_existing_melds(&self) -> Vec<Meld> {
        let my_idx = super::state::CpuGameState::wind_to_index(self.state.my_seat_wind);
        self.state.player_melds[my_idx]
            .iter()
            .map(|open| {
                // 手分析用に3枚に切り詰め
                let mut o = open.clone();
                if o.tiles.len() > 3 {
                    o.tiles.truncate(3);
                }
                o
            })
            .collect()
    }

    /// ポンした場合に向聴数が下がるか
    fn call_reduces_shanten_pon(&self, called_tile: Tile) -> bool {
        // 現在の向聴数
        let current_hand = Hand::new(self.state.my_hand.clone(), None);
        let current_shanten = calc_shanten_number(&current_hand);

        // ポン後の手牌（同じ種類の2枚を除去）
        let tt = called_tile.get();
        let mut remaining = self.state.my_hand.clone();
        let mut removed = 0;
        remaining.retain(|t| {
            if t.get() == tt && removed < 2 {
                removed += 1;
                false
            } else {
                true
            }
        });

        if removed < 2 {
            return false;
        }

        // 既存の副露 + 今回のポンを含めた Hand を作成
        let mut melds = self.build_existing_melds();
        melds.push(Meld {
            tiles: vec![called_tile, called_tile, called_tile],
            category: MeldType::Pon,
            from: MeldFrom::Unknown,
            called_tile: Some(called_tile),
        });

        let new_hand = Hand::new_with_melds(remaining, melds, None);
        calc_shanten_number(&new_hand) < current_shanten
    }

    /// チーした場合に向聴数が下がるか
    fn call_reduces_shanten_chi(&self, called_tile: Tile, hand_tiles: [Tile; 2]) -> bool {
        let current_hand = Hand::new(self.state.my_hand.clone(), None);
        let current_shanten = calc_shanten_number(&current_hand);

        // チー後の手牌（指定の2枚を除去。赤ドラも区別して一致させる）
        let mut remaining = self.state.my_hand.clone();
        let mut chi_tiles_for_meld = Vec::new();
        for &target in &hand_tiles {
            if let Some(pos) = remaining.iter().position(|t| *t == target) {
                chi_tiles_for_meld.push(remaining.remove(pos));
            } else {
                return false;
            }
        }

        // 既存の副露 + 今回のチーを含めた Hand を作成
        let mut melds = self.build_existing_melds();
        melds.push(Meld {
            tiles: vec![called_tile, chi_tiles_for_meld[0], chi_tiles_for_meld[1]],
            category: MeldType::Chi,
            from: MeldFrom::Previous,
            called_tile: Some(called_tile),
        });

        let new_hand = Hand::new_with_melds(remaining, melds, None);
        calc_shanten_number(&new_hand) < current_shanten
    }
}

/// 役牌かどうか判定
fn is_yakuhai(
    tile_type: u32,
    seat_wind: mahjong_core::tile::Wind,
    prevailing_wind: mahjong_core::tile::Wind,
) -> bool {
    use mahjong_core::tile::Tile as T;
    // 三元牌
    if tile_type == T::Z5 || tile_type == T::Z6 || tile_type == T::Z7 {
        return true;
    }
    // 場風
    let pw = match prevailing_wind {
        mahjong_core::tile::Wind::East => T::Z1,
        mahjong_core::tile::Wind::South => T::Z2,
        mahjong_core::tile::Wind::West => T::Z3,
        mahjong_core::tile::Wind::North => T::Z4,
    };
    if tile_type == pw {
        return true;
    }
    // 自風
    let sw = match seat_wind {
        mahjong_core::tile::Wind::East => T::Z1,
        mahjong_core::tile::Wind::South => T::Z2,
        mahjong_core::tile::Wind::West => T::Z3,
        mahjong_core::tile::Wind::North => T::Z4,
    };
    tile_type == sw
}

/// タンヤオ有効牌（中張牌: 2-8）か
fn is_tanyao_tile(tile_type: u32) -> bool {
    if tile_type >= 27 {
        return false;
    }
    let num = tile_type % 9;
    (1..=7).contains(&num)
}

#[cfg(test)]
mod tests {
    use super::*;
    use mahjong_core::tile::Wind;

    fn game_started_event(seat_wind: Wind, hand: Vec<Tile>) -> ServerEvent {
        ServerEvent::GameStarted {
            seat_wind,
            hand,
            scores: [25000; 4],
            prevailing_wind: Wind::East,
            dora_indicators: vec![],
            round_number: 0,
            honba: 0,
            riichi_sticks: 0,
        }
    }

    #[test]
    fn test_cpu_config_creation() {
        let config = CpuConfig::new(CpuLevel::Normal, CpuPersonality::Balanced);
        assert_eq!(config.level, CpuLevel::Normal);
        assert_eq!(config.personality, CpuPersonality::Balanced);
    }

    #[test]
    fn test_level_capabilities() {
        assert!(!CpuLevel::Weak.uses_acceptance_count());
        assert!(CpuLevel::Normal.uses_acceptance_count());
        assert!(CpuLevel::Strong.uses_acceptance_count());

        assert!(!CpuLevel::Weak.uses_value_estimation());
        assert!(!CpuLevel::Normal.uses_value_estimation());
        assert!(CpuLevel::Strong.uses_value_estimation());

        assert!(CpuLevel::Weak.should_make_mistake());
        assert!(!CpuLevel::Normal.should_make_mistake());
    }

    #[test]
    fn test_is_yakuhai() {
        assert!(is_yakuhai(Tile::Z5, Wind::East, Wind::East)); // 白
        assert!(is_yakuhai(Tile::Z6, Wind::East, Wind::East)); // 發
        assert!(is_yakuhai(Tile::Z7, Wind::East, Wind::East)); // 中
        assert!(is_yakuhai(Tile::Z1, Wind::East, Wind::East)); // 東（場風+自風）
        assert!(!is_yakuhai(Tile::Z2, Wind::East, Wind::East)); // 南（場風でも自風でもない）
    }

    #[test]
    fn test_is_yakuhai_seat_and_prevailing_wind() {
        // 自風が南のとき、Z2（南）は役牌
        assert!(is_yakuhai(Tile::Z2, Wind::South, Wind::East));
        // 場風が南のとき、Z2（南）は役牌
        assert!(is_yakuhai(Tile::Z2, Wind::East, Wind::South));
        // どちらでもないとき、Z2 は役牌でない
        assert!(!is_yakuhai(Tile::Z2, Wind::East, Wind::East));
        // 三元牌は常に役牌
        assert!(is_yakuhai(Tile::Z5, Wind::North, Wind::West));
        assert!(is_yakuhai(Tile::Z6, Wind::North, Wind::West));
        assert!(is_yakuhai(Tile::Z7, Wind::North, Wind::West));
    }

    #[test]
    fn test_is_tanyao_tile() {
        // 端牌・字牌は非タンヤオ
        assert!(!is_tanyao_tile(Tile::M1));
        assert!(!is_tanyao_tile(Tile::M9));
        assert!(!is_tanyao_tile(Tile::P1));
        assert!(!is_tanyao_tile(Tile::P9));
        assert!(!is_tanyao_tile(Tile::S1));
        assert!(!is_tanyao_tile(Tile::S9));
        assert!(!is_tanyao_tile(Tile::Z1));
        assert!(!is_tanyao_tile(Tile::Z7));
        // 中張牌はタンヤオ
        assert!(is_tanyao_tile(Tile::M2));
        assert!(is_tanyao_tile(Tile::M8));
        assert!(is_tanyao_tile(Tile::P5));
        assert!(is_tanyao_tile(Tile::S7));
    }

    #[test]
    fn test_tsumo_action() {
        let config = CpuConfig::new(CpuLevel::Normal, CpuPersonality::Balanced);
        let mut client = CpuClient::new(config);

        client.handle_event(&game_started_event(
            Wind::East,
            vec![
                Tile::new(Tile::M1),
                Tile::new(Tile::M2),
                Tile::new(Tile::M3),
                Tile::new(Tile::P4),
                Tile::new(Tile::P5),
                Tile::new(Tile::P6),
                Tile::new(Tile::S7),
                Tile::new(Tile::S8),
                Tile::new(Tile::S9),
                Tile::new(Tile::Z1),
                Tile::new(Tile::Z1),
                Tile::new(Tile::Z1),
                Tile::new(Tile::Z2),
            ],
        ));

        let action = client.handle_event(&ServerEvent::TileDrawn {
            tile: Tile::new(Tile::Z2),
            remaining_tiles: 50,
            can_tsumo: true,
            can_riichi: false,
            is_furiten: false,
        });

        assert!(matches!(action, Some(ClientAction::Tsumo)));
    }

    #[test]
    fn test_ron_action() {
        let config = CpuConfig::new(CpuLevel::Weak, CpuPersonality::Balanced);
        let mut client = CpuClient::new(config);

        client.handle_event(&game_started_event(Wind::South, vec![]));

        let action = client.handle_event(&ServerEvent::CallAvailable {
            tile: Tile::new(Tile::M1),
            discarder: Wind::East,
            calls: vec![AvailableCall::Ron],
        });

        assert!(matches!(action, Some(ClientAction::Ron)));
    }

    #[test]
    fn test_discard_when_in_riichi_state() {
        // リーチ中はcan_tsumo=falseのときツモ切りを返す
        let config = CpuConfig::new(CpuLevel::Normal, CpuPersonality::Balanced);
        let mut client = CpuClient::new(config);

        client.handle_event(&game_started_event(
            Wind::East,
            vec![
                Tile::new(Tile::M1),
                Tile::new(Tile::M2),
                Tile::new(Tile::M3),
                Tile::new(Tile::P4),
                Tile::new(Tile::P5),
                Tile::new(Tile::P6),
                Tile::new(Tile::S7),
                Tile::new(Tile::S8),
                Tile::new(Tile::S9),
                Tile::new(Tile::Z1),
                Tile::new(Tile::Z1),
                Tile::new(Tile::Z1),
                Tile::new(Tile::Z2),
            ],
        ));
        client.handle_event(&ServerEvent::PlayerRiichi {
            player: Wind::East,
            scores: [24000, 25000, 25000, 25000],
            riichi_sticks: 1,
        });

        let action = client.handle_event(&ServerEvent::TileDrawn {
            tile: Tile::new(Tile::M5),
            remaining_tiles: 30,
            can_tsumo: false,
            can_riichi: false,
            is_furiten: false,
        });

        assert!(matches!(action, Some(ClientAction::Discard { tile: None })));
    }

    #[test]
    fn test_riichi_action_when_can_riichi() {
        // can_riichi=true かつリーチ積極度が十分なら Riichi を返す
        let config = CpuConfig::new(CpuLevel::Normal, CpuPersonality::Balanced);
        let mut client = CpuClient::new(config);

        // テンパイ1枚前の手牌（Z2待ち）
        client.handle_event(&game_started_event(
            Wind::East,
            vec![
                Tile::new(Tile::M1),
                Tile::new(Tile::M2),
                Tile::new(Tile::M3),
                Tile::new(Tile::P4),
                Tile::new(Tile::P5),
                Tile::new(Tile::P6),
                Tile::new(Tile::S7),
                Tile::new(Tile::S8),
                Tile::new(Tile::S9),
                Tile::new(Tile::Z1),
                Tile::new(Tile::Z1),
                Tile::new(Tile::Z1),
                Tile::new(Tile::Z2),
            ],
        ));

        // Z3をツモ → Z2Z3の順子形成でもテンパイにならないが、
        // can_riichi フラグをサーバが立てている想定
        let action = client.handle_event(&ServerEvent::TileDrawn {
            tile: Tile::new(Tile::Z3),
            remaining_tiles: 30,
            can_tsumo: false,
            can_riichi: true,
            is_furiten: false,
        });

        assert!(matches!(action, Some(ClientAction::Riichi { .. })));
    }

    #[test]
    fn test_discard_action_when_no_special_state() {
        // ツモ和了不可・リーチ不可のとき Discard を返す
        let config = CpuConfig::new(CpuLevel::Normal, CpuPersonality::Balanced);
        let mut client = CpuClient::new(config);

        client.handle_event(&game_started_event(
            Wind::East,
            vec![
                Tile::new(Tile::M1),
                Tile::new(Tile::M2),
                Tile::new(Tile::M3),
                Tile::new(Tile::P4),
                Tile::new(Tile::P5),
                Tile::new(Tile::P6),
                Tile::new(Tile::S7),
                Tile::new(Tile::S8),
                Tile::new(Tile::S9),
                Tile::new(Tile::Z2),
                Tile::new(Tile::Z3),
                Tile::new(Tile::Z4),
                Tile::new(Tile::Z5),
            ],
        ));

        let action = client.handle_event(&ServerEvent::TileDrawn {
            tile: Tile::new(Tile::Z6),
            remaining_tiles: 30,
            can_tsumo: false,
            can_riichi: false,
            is_furiten: false,
        });

        assert!(matches!(action, Some(ClientAction::Discard { .. })));
    }

    #[test]
    fn test_nine_terminals_high_value_continues() {
        // HighValue は国士狙いで続行（declare=false）
        let config = CpuConfig::new(CpuLevel::Strong, CpuPersonality::HighValue);
        let mut client = CpuClient::new(config);

        let action = client.handle_event(&ServerEvent::NineTerminalsAvailable);

        assert!(matches!(
            action,
            Some(ClientAction::NineTerminals { declare: false })
        ));
    }

    #[test]
    fn test_nine_terminals_non_high_value_declares() {
        // HighValue 以外は流局宣言（declare=true）
        for personality in [
            CpuPersonality::Balanced,
            CpuPersonality::Speedy,
            CpuPersonality::Defensive,
        ] {
            let config = CpuConfig::new(CpuLevel::Normal, personality);
            let mut client = CpuClient::new(config);

            let action = client.handle_event(&ServerEvent::NineTerminalsAvailable);

            assert!(
                matches!(action, Some(ClientAction::NineTerminals { declare: true })),
                "personality {personality:?} should declare nine terminals"
            );
        }
    }

    #[test]
    fn test_handle_event_returns_none_for_non_actionable() {
        // 打牌・他プレイヤーツモ等はアクション不要なので None
        let config = CpuConfig::new(CpuLevel::Normal, CpuPersonality::Balanced);
        let mut client = CpuClient::new(config);

        let events = [
            ServerEvent::TileDiscarded {
                player: Wind::South,
                tile: Tile::new(Tile::M1),
                is_tsumogiri: false,
            },
            ServerEvent::OtherPlayerDrew {
                player: Wind::South,
                remaining_tiles: 50,
            },
            ServerEvent::PlayerRiichi {
                player: Wind::South,
                scores: [25000; 4],
                riichi_sticks: 1,
            },
        ];

        for event in &events {
            assert!(
                client.handle_event(event).is_none(),
                "expected None for {event:?}"
            );
        }
    }

    #[test]
    fn test_pass_when_chi_only_and_high_value() {
        // HighValue はチーしない → Pass を返す
        let config = CpuConfig::new(CpuLevel::Normal, CpuPersonality::HighValue);
        let mut client = CpuClient::new(config);

        client.handle_event(&game_started_event(
            Wind::South,
            vec![
                Tile::new(Tile::M2),
                Tile::new(Tile::M3),
                Tile::new(Tile::P4),
                Tile::new(Tile::P5),
                Tile::new(Tile::P6),
                Tile::new(Tile::S7),
                Tile::new(Tile::S8),
                Tile::new(Tile::S9),
                Tile::new(Tile::Z1),
                Tile::new(Tile::Z1),
                Tile::new(Tile::Z1),
                Tile::new(Tile::Z2),
                Tile::new(Tile::Z2),
            ],
        ));

        let action = client.handle_event(&ServerEvent::CallAvailable {
            tile: Tile::new(Tile::M1),
            discarder: Wind::East,
            calls: vec![AvailableCall::Chi {
                options: vec![[Tile::new(Tile::M2), Tile::new(Tile::M3)]],
            }],
        });

        assert!(matches!(action, Some(ClientAction::Pass)));
    }

    #[test]
    fn test_pon_yakuhai_normal_level() {
        // 役牌ポンは向聴数が下がれば Normal レベルでも鳴く
        let config = CpuConfig::new(CpuLevel::Normal, CpuPersonality::Balanced);
        let mut client = CpuClient::new(config);

        // Z5（白）×2 を持つ一向聴の手牌: M123+P456+S789完成+Z5Z5雀頭+Z2Z3孤立
        // → Z5 ポンで向聴数 1→0 に下がる
        client.handle_event(&game_started_event(
            Wind::East,
            vec![
                Tile::new(Tile::Z5),
                Tile::new(Tile::Z5),
                Tile::new(Tile::M1),
                Tile::new(Tile::M2),
                Tile::new(Tile::M3),
                Tile::new(Tile::P4),
                Tile::new(Tile::P5),
                Tile::new(Tile::P6),
                Tile::new(Tile::S7),
                Tile::new(Tile::S8),
                Tile::new(Tile::S9),
                Tile::new(Tile::Z2),
                Tile::new(Tile::Z3),
            ],
        ));

        let action = client.handle_event(&ServerEvent::CallAvailable {
            tile: Tile::new(Tile::Z5),
            discarder: Wind::South,
            calls: vec![AvailableCall::Pon {
                options: vec![[Tile::new(Tile::Z5), Tile::new(Tile::Z5)]],
            }],
        });

        assert!(matches!(action, Some(ClientAction::Pon { .. })));
    }

    #[test]
    fn test_pon_not_called_when_shanten_does_not_decrease() {
        // ポンで向聴数が下がらない場合は Pass
        // 国士無双テンパイ（13孤立牌+対子）では向聴数=0だが、
        // Z5 をポンすると closed=false になり nm 向聴数が大幅に上がる
        let config = CpuConfig::new(CpuLevel::Normal, CpuPersonality::Balanced);
        let mut client = CpuClient::new(config);

        // 12種の孤立牌 + Z5×2: 国士无双テンパイ(向聴数=0)
        // Z5 ポン後は closed 制約外れて向聴数が大幅に上昇する
        client.handle_event(&game_started_event(
            Wind::East,
            vec![
                Tile::new(Tile::M1),
                Tile::new(Tile::M9),
                Tile::new(Tile::P1),
                Tile::new(Tile::P9),
                Tile::new(Tile::S1),
                Tile::new(Tile::S9),
                Tile::new(Tile::Z1),
                Tile::new(Tile::Z2),
                Tile::new(Tile::Z3),
                Tile::new(Tile::Z4),
                Tile::new(Tile::Z5),
                Tile::new(Tile::Z5),
                Tile::new(Tile::Z7),
            ],
        ));

        let action = client.handle_event(&ServerEvent::CallAvailable {
            tile: Tile::new(Tile::Z5),
            discarder: Wind::South,
            calls: vec![AvailableCall::Pon {
                options: vec![[Tile::new(Tile::Z5), Tile::new(Tile::Z5)]],
            }],
        });

        assert!(matches!(action, Some(ClientAction::Pass)));
    }

    #[test]
    fn test_pass_when_daiminkan_only_non_strong_high_value() {
        // 大明カンの場合、Strong+HighValue 以外はパス
        let config = CpuConfig::new(CpuLevel::Normal, CpuPersonality::Balanced);
        let mut client = CpuClient::new(config);

        client.handle_event(&game_started_event(Wind::South, vec![]));

        let action = client.handle_event(&ServerEvent::CallAvailable {
            tile: Tile::new(Tile::M1),
            discarder: Wind::East,
            calls: vec![AvailableCall::Daiminkan],
        });

        assert!(matches!(action, Some(ClientAction::Pass)));
    }
}
