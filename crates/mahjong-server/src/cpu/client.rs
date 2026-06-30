//! CPUクライアント
//!
//! ServerEvent を受信して ClientAction を返す。
//! プレイヤーと全く同じプロトコルでサーバとやり取りする。

use mahjong_core::hand::Hand;
use mahjong_core::hand_info::hand_analyzer::calc_shanten_number;
use mahjong_core::hand_info::meld::{Meld, MeldFrom, MeldType};
use mahjong_core::tile::Tile;
use serde::{Deserialize, Serialize};

use crate::protocol::{AvailableCall, ClientAction, ServerEvent};

use super::evaluator;
use super::heuristics;
use super::state::CpuGameState;

/// CPUの強さレベル
///
/// `Weak < Normal < Strong` の順序を持つ。
/// 定石（heuristics）の「弱以上」「中以上」などの有効化判定に使用する。
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum CpuLevel {
    /// 初心者: 向聴数のみ考慮、防御なし、ミスあり
    Weak,
    /// 中級者: 有効牌数考慮、基本防御
    Normal,
    /// 上級者: 打点考慮、筋/壁/現物の高度な防御
    Strong,
}

impl CpuLevel {
    /// 表示用の名称
    pub fn display_name(&self) -> &'static str {
        match self {
            CpuLevel::Weak => "Weak",
            CpuLevel::Normal => "Normal",
            CpuLevel::Strong => "Strong",
        }
    }

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
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
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

impl CpuPersonality {
    /// 表示用の名称
    pub fn display_name(&self) -> &'static str {
        match self {
            CpuPersonality::Balanced => "Balanced",
            CpuPersonality::Speedy => "Speedy",
            CpuPersonality::HighValue => "HighValue",
            CpuPersonality::Defensive => "Defensive",
        }
    }
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
    /// 定石（heuristics）を適用するか
    ///
    /// 通常は true。false にすると定石導入前の挙動になるため、
    /// シミュレーションでの新旧比較（A/B テスト）に使用する。
    pub heuristics_enabled: bool,
}

impl CpuConfig {
    /// 指定した強さと性格で設定を作成する
    pub fn new(level: CpuLevel, personality: CpuPersonality) -> Self {
        let params = PersonalityParams::from_personality(personality);
        CpuConfig {
            level,
            personality,
            params,
            heuristics_enabled: true,
        }
    }

    /// 定石を無効化した設定を返す（シミュレーションでの新旧比較用）
    pub fn without_heuristics(mut self) -> Self {
        self.heuristics_enabled = false;
        self
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
        // 聴牌を維持できる打牌が見つからない場合はリーチせず通常打牌に進む
        // （不正なリーチ宣言はサーバに拒否され、局が進行不能になる）
        if self.state.can_riichi
            && let Some(tile) = self.select_riichi_tile()
        {
            // 定石判定（#168〜#172）。Neutral の場合は従来の積極度判断に委ねる
            let declare = if self.config.heuristics_enabled {
                let ctx = heuristics::CallContext {
                    state: &self.state,
                    config: &self.config,
                };
                match heuristics::judge_riichi(&ctx, tile) {
                    heuristics::RiichiJudgement::Declare => true,
                    heuristics::RiichiJudgement::Damaten => false,
                    heuristics::RiichiJudgement::Neutral => self.should_riichi(),
                }
            } else {
                self.should_riichi()
            };
            if declare {
                return ClientAction::Riichi { tile };
            }
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
        if let Some(tile) =
            evaluator::select_best_discard(&candidates, &self.config, attacking, &self.state)
        {
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
        // 喰い替え禁止牌（直前の鳴きが対象）。これらは捨てるとサーバに拒否されるため除外する。
        let forbidden = self
            .state
            .my_melds()
            .last()
            .map(|meld| meld.forbidden_swap_tiles())
            .unwrap_or_default();

        // 鳴き後はツモ牌がないので、手牌から選ぶ
        let candidates: Vec<_> = evaluator::evaluate_discards(&self.state, &self.config)
            .into_iter()
            .filter(|c| !forbidden.contains(&c.tile.get()))
            .collect();
        let attacking = self.should_attack();

        if let Some(tile) =
            evaluator::select_best_discard(&candidates, &self.config, attacking, &self.state)
        {
            ClientAction::Discard { tile: Some(tile) }
        } else if let Some(&tile) = self
            .state
            .my_hand
            .iter()
            .rev()
            .find(|t| !forbidden.contains(&t.get()))
        {
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

        // 大明カンは常にパス（ドラ増加リスクがあり、打点メリットが薄い）

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
    ///
    /// 定石有効時は待ち枚数が最大になる宣言牌を選び、同数なら安全度で比較する。
    /// 定石無効時は従来どおり安全度のみで選ぶ。
    ///
    /// 戻り値:
    /// - `Some(Some(tile))`: tile を手出ししてリーチ
    /// - `Some(None)`: ツモ切りリーチ
    /// - `None`: 聴牌を維持できる打牌がない（リーチ不可）
    fn select_riichi_tile(&self) -> Option<Option<Tile>> {
        // テンパイを維持する牌を選ぶ
        let mut all_tiles = self.state.my_hand.clone();
        if let Some(drawn) = self.state.my_drawn {
            all_tiles.push(drawn);
        }

        // 暗カンがある場合も正しく判定できるよう、副露を含めて向聴数を計算する
        let melds = self.state.my_melds_for_analysis();
        let visible = self.state.visible_tile_counts();
        let mut best: Option<(Tile, u32, f64)> = None;

        for (i, &tile) in all_tiles.iter().enumerate() {
            let mut remaining: Vec<Tile> = all_tiles.clone();
            remaining.remove(i);

            // 捨てた後にテンパイを維持するか
            let hand = Hand::new_with_melds(remaining.clone(), melds.clone(), None);
            let shanten = calc_shanten_number(&hand);

            if shanten.is_ready() {
                // 待ち枚数（定石有効時のみ考慮）
                let waits = if self.config.heuristics_enabled {
                    heuristics::remaining_wait_count(&remaining, &melds, &visible)
                } else {
                    0
                };
                // 安全度で比較
                let safety = super::defense::evaluate_safety(tile, &self.state, &self.config);
                let is_better = match best {
                    Some((_, best_waits, best_safety)) => {
                        waits > best_waits || (waits == best_waits && safety > best_safety)
                    }
                    None => true,
                };
                if is_better {
                    best = Some((tile, waits, safety));
                }
            }
        }

        best.map(|(tile, _, _)| {
            // ツモ牌ならNone（ツモ切りリーチ）
            if self.state.my_drawn == Some(tile) {
                None
            } else {
                Some(tile)
            }
        })
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

        let ctx = heuristics::CallContext {
            state: &self.state,
            config: &self.config,
        };

        for (tile_type, &count) in counts.iter().enumerate() {
            if count == 4 {
                // 定石判定（中以上）: 手を壊すカン・他家リーチ後のカンを抑制
                if heuristics::judge_ankan(&ctx, tile_type as u32)
                    == heuristics::CallJudgement::Forbid
                {
                    continue;
                }

                // 定石無効時の従来動作: Strongのみテンパイ維持を確認
                if !self.config.heuristics_enabled && self.config.level == CpuLevel::Strong {
                    let remaining: Vec<Tile> = all_tiles
                        .iter()
                        .filter(|t| t.get() != tile_type as u32)
                        .copied()
                        .collect();
                    // カン後の形: 既存の副露 + 新しい槓子（解析用に3枚）を面子として数える
                    let mut melds = self.state.my_melds_for_analysis();
                    melds.push(Meld {
                        tiles: vec![Tile::new(tile_type as u32); 3],
                        category: MeldType::Kan,
                        from: MeldFrom::Myself,
                        called_tile: None,
                    });
                    let hand = Hand::new_with_melds(remaining, melds, None);
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

        // 自分の向聴数を計算（副露も面子として数える）
        let mut all_tiles = self.state.my_hand.clone();
        if let Some(drawn) = self.state.my_drawn {
            all_tiles.push(drawn);
        }
        let hand = Hand::new_with_melds(all_tiles, self.state.my_melds_for_analysis(), None);
        let shanten = calc_shanten_number(&hand);

        // 脅威の数: リーチ者 + 定石有効時は3副露以上の他家（#180: 聴牌濃厚）
        let riichi_count = self.state.player_riichi.iter().filter(|&&r| r).count();
        let threat_count = if self.config.heuristics_enabled {
            let my_idx = CpuGameState::wind_to_index(self.state.my_seat_wind);
            let melded_threats = (0..4)
                .filter(|&i| {
                    i != my_idx
                        && !self.state.player_riichi[i]
                        && self.state.player_melds[i].len() >= 3
                })
                .count();
            riichi_count + melded_threats
        } else {
            riichi_count
        };

        // 終盤の遠い手は降りる（#183, 弱以上）:
        // 残りツモが少ない2向聴以下の手は、脅威の有無や打点によらず押さない
        if self.config.heuristics_enabled && self.state.remaining_tiles <= 12 && shanten >= 2 {
            return false;
        }

        // 押し引きの定石判定（#178, 中以上）:
        // 良形・高打点・親の聴牌は押し、愚形安手の聴牌は降りる
        let ctx = heuristics::CallContext {
            state: &self.state,
            config: &self.config,
        };
        match heuristics::judge_push(&ctx, threat_count) {
            heuristics::PushJudgement::Push => return true,
            heuristics::PushJudgement::Fold => return false,
            heuristics::PushJudgement::Neutral => {}
        }

        // テンパイなら基本的に攻撃
        if shanten.is_ready_or_won() {
            return true;
        }

        // 防御を使わないレベルなら常に攻撃
        // ただし定石有効時は弱レベルでも撤退判断を行う（#173: ベタオリは弱以上）
        if !self.config.level.uses_defense() && !self.config.heuristics_enabled {
            return true;
        }

        // 撤退判断
        // 脅威2人以上 → 撤退寄り
        if threat_count >= 2 && shanten >= 2 {
            return params.retreat_threshold < 0.3;
        }

        // 脅威1人 + 自分が2向聴以上 → 性格次第
        if threat_count >= 1 && shanten >= 2 {
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
    /// 定石有効時は国士無双の見込みで判断する（#158/#159/#160）:
    /// - 么九牌10種以上なら国士無双を狙って続行
    /// - 9種でも高打点型、または大きく負けている場合は続行
    /// - それ以外は流局を宣言する
    ///
    /// 定石無効時は従来どおり高打点型のみ続行する。
    fn decide_nine_terminals(&self) -> ClientAction {
        if self.config.heuristics_enabled {
            let mut counts = [0u8; 34];
            for t in &self.state.my_hand {
                counts[t.get() as usize] += 1;
            }
            if let Some(drawn) = self.state.my_drawn {
                counts[drawn.get() as usize] += 1;
            }
            let kinds = super::defense::ORPHAN_TYPES
                .iter()
                .filter(|&&t| counts[t as usize] > 0)
                .count();

            let continue_kokushi = kinds >= 10
                || (kinds >= 9
                    && (self.config.personality == CpuPersonality::HighValue
                        || heuristics::is_far_behind(&self.state)));
            return ClientAction::NineTerminals {
                declare: !continue_kokushi,
            };
        }

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

        // 向聴数が下がらないポンはしない（全レベル共通）
        if !self.call_reduces_shanten_pon(called_tile) {
            return false;
        }

        // 定石判定: 役なし鳴き禁止・裸単騎回避・役牌早ポン
        // （定石無効時は Neutral が返り、従来の判断に進む）
        let ctx = heuristics::CallContext {
            state: &self.state,
            config: &self.config,
        };
        match heuristics::judge_pon(&ctx, called_tile) {
            heuristics::CallJudgement::Forbid => return false,
            heuristics::CallJudgement::Encourage => return true,
            heuristics::CallJudgement::Neutral => {}
        }

        // Weakレベル: 向聴数が下がるなら鳴く
        if self.config.level == CpuLevel::Weak {
            return true;
        }

        // 鳴き積極度が低ければパス
        if params.call_aggressiveness < 0.3 {
            return false;
        }

        // 鳴いた後に役がありそうか（簡易チェック）
        let tt = called_tile.get();

        // 役牌のポンは積極的に
        if is_yakuhai(tt, self.state.my_seat_wind, self.state.round_wind) {
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
        let ctx = heuristics::CallContext {
            state: &self.state,
            config: &self.config,
        };

        // 各選択肢で向聴数が下がるか確認
        for &opt in options {
            if self.call_reduces_shanten_chi(called_tile, opt) {
                // 定石判定: 役なし鳴き禁止・裸単騎回避
                match heuristics::judge_chi(&ctx, called_tile, opt) {
                    heuristics::CallJudgement::Forbid => continue,
                    heuristics::CallJudgement::Encourage => return Some(opt),
                    heuristics::CallJudgement::Neutral => {}
                }

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

    /// ポンした場合に向聴数が下がるか
    fn call_reduces_shanten_pon(&self, called_tile: Tile) -> bool {
        // 現在の向聴数（既存の副露も含めて計算しないと比較が非対称になる）
        let melds = self.state.my_melds_for_analysis();
        let current_hand = Hand::new_with_melds(self.state.my_hand.clone(), melds.clone(), None);
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
        let mut melds = melds;
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
        // 現在の向聴数（既存の副露も含めて計算しないと比較が非対称になる）
        let melds = self.state.my_melds_for_analysis();
        let current_hand = Hand::new_with_melds(self.state.my_hand.clone(), melds.clone(), None);
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
        let mut melds = melds;
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
pub(crate) fn is_yakuhai(
    tile_type: u32,
    seat_wind: mahjong_core::tile::Wind,
    round_wind: mahjong_core::tile::Wind,
) -> bool {
    use mahjong_core::tile::Tile as T;
    // 三元牌、または場風・自風（Wind の判別値は対応する牌種と一致する）
    matches!(tile_type, T::Z5..=T::Z7)
        || tile_type == round_wind as u32
        || tile_type == seat_wind as u32
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
#[path = "client_tests.rs"]
mod tests;
