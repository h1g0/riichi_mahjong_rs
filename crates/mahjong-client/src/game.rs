//! ゲーム状態管理
//!
//! サーバから受信したイベントに基づいてクライアント側の状態を管理する。

use macroquad::prelude::*;
use mahjong_core::hand::Hand;
use mahjong_core::hand_info::hand_analyzer::HandAnalyzer;
use mahjong_core::tile::{Tile, Wind};
use mahjong_server::protocol::{AvailableCall, CallType, ClientAction, DrawReason, ServerEvent};

/// 副露（鳴き）の表示情報
#[derive(Debug, Clone)]
pub struct MeldInfo {
    #[allow(dead_code)]
    pub call_type: CallType,
    pub tiles: Vec<Tile>,
}

/// 捨て牌の表示情報
#[derive(Debug, Clone)]
pub struct DiscardInfo {
    pub tile: Tile,
    pub is_tsumogiri: bool,
}

/// クライアント側のゲーム状態
pub struct GameState {
    /// 自分の座席の風
    pub seat_wind: Option<Wind>,
    /// 自分の手牌
    pub hand: Vec<Tile>,
    /// ツモ牌（直近にツモった牌）
    pub drawn: Option<Tile>,
    /// 各プレイヤーの捨て牌（自分=0, 下家=1, 対面=2, 上家=3）
    pub discards: [Vec<DiscardInfo>; 4],
    /// 各プレイヤーの点数
    pub scores: [i32; 4],
    /// 場風
    pub prevailing_wind: Option<Wind>,
    /// ドラ表示牌
    pub dora_indicators: Vec<Tile>,
    /// 山の残り枚数
    pub remaining_tiles: usize,
    /// 選択中の牌のインデックス
    pub selected_tile: Option<usize>,
    /// ツモ牌が選択中か
    pub selected_drawn: bool,
    /// ツモ和了可能か
    pub can_tsumo: bool,
    /// リーチ宣言可能か
    pub can_riichi: bool,
    /// 自分がリーチ中か
    pub is_riichi: bool,
    /// リーチ宣言のための打牌選択中か
    pub riichi_selection_mode: bool,
    /// リーチ可能な手牌インデックス
    pub riichi_selectable_tiles: Vec<usize>,
    /// ツモ牌切りでリーチ可能か
    pub riichi_selectable_drawn: bool,
    /// 局の結果メッセージ
    pub result_message: Option<String>,
    /// 自分の手番か
    pub is_my_turn: bool,
    /// ゲームフェーズ
    pub phase: GamePhase,
    /// 鳴き可能な選択肢
    pub available_calls: Vec<AvailableCall>,
    /// 鳴き対象の牌
    pub call_target_tile: Option<Tile>,
    /// 鳴き対象の捨てたプレイヤー
    pub call_discarder: Option<Wind>,
    /// 自分の副露（鳴き）一覧
    pub melds: Vec<MeldInfo>,
    /// 局番号（0=東1局, 1=東2局, ...）
    pub round_number: usize,
    /// 本場数
    pub honba: usize,
}

/// ゲームフェーズ
#[derive(Debug, Clone, PartialEq)]
pub enum GamePhase {
    /// ゲーム開始前
    WaitingForStart,
    /// 対局中
    Playing,
    /// 局終了（結果表示中）
    RoundResult,
    /// ゲーム終了
    GameOver,
}

impl GameState {
    pub fn new() -> Self {
        GameState {
            seat_wind: None,
            hand: Vec::new(),
            drawn: None,
            discards: [Vec::new(), Vec::new(), Vec::new(), Vec::new()],
            scores: [25000; 4],
            prevailing_wind: None,
            dora_indicators: Vec::new(),
            remaining_tiles: 70,
            selected_tile: None,
            selected_drawn: false,
            can_tsumo: false,
            can_riichi: false,
            is_riichi: false,
            riichi_selection_mode: false,
            riichi_selectable_tiles: Vec::new(),
            riichi_selectable_drawn: false,
            result_message: None,
            is_my_turn: false,
            phase: GamePhase::WaitingForStart,
            available_calls: Vec::new(),
            call_target_tile: None,
            call_discarder: None,
            melds: Vec::new(),
            round_number: 0,
            honba: 0,
        }
    }

    /// サーバイベントを処理する
    pub fn handle_event(&mut self, event: ServerEvent) {
        match event {
            ServerEvent::GameStarted {
                seat_wind,
                hand,
                scores,
                prevailing_wind,
                dora_indicators,
                round_number,
                honba,
            } => {
                self.seat_wind = Some(seat_wind);
                self.hand = hand;
                self.hand.sort();
                self.drawn = None;
                self.scores = scores;
                self.prevailing_wind = Some(prevailing_wind);
                self.dora_indicators = dora_indicators;
                self.discards = [Vec::new(), Vec::new(), Vec::new(), Vec::new()];
                self.result_message = None;
                self.phase = GamePhase::Playing;
                self.available_calls.clear();
                self.call_target_tile = None;
                self.call_discarder = None;
                self.can_tsumo = false;
                self.can_riichi = false;
                self.is_riichi = false;
                self.clear_riichi_selection();
                self.melds.clear();
                self.round_number = round_number;
                self.honba = honba;
            }

            ServerEvent::TileDrawn {
                tile,
                remaining_tiles,
                can_tsumo,
                can_riichi,
            } => {
                self.drawn = Some(tile);
                self.remaining_tiles = remaining_tiles;
                self.is_my_turn = true;
                self.can_tsumo = can_tsumo;
                self.can_riichi = can_riichi;
                self.clear_riichi_selection();
                self.available_calls.clear();
                self.call_target_tile = None;
            }

            ServerEvent::OtherPlayerDrew {
                player: _,
                remaining_tiles,
            } => {
                self.remaining_tiles = remaining_tiles;
            }

            ServerEvent::TileDiscarded {
                player,
                tile,
                is_tsumogiri,
            } => {
                let relative_idx = self.relative_player_index(player);
                self.discards[relative_idx].push(DiscardInfo {
                    tile,
                    is_tsumogiri,
                });

                // 自分が捨てた場合
                if Some(player) == self.seat_wind {
                    self.is_my_turn = false;
                    self.drawn = None;
                    self.selected_tile = None;
                    self.selected_drawn = false;
                    self.clear_riichi_selection();
                }
            }

            ServerEvent::CallAvailable {
                tile,
                discarder,
                calls,
            } => {
                self.available_calls = calls;
                self.call_target_tile = Some(tile);
                self.call_discarder = Some(discarder);
            }

            ServerEvent::PlayerCalled {
                player,
                call_type,
                called_tile: _,
                tiles,
            } => {
                // 鳴き選択肢をクリア
                self.available_calls.clear();
                self.call_target_tile = None;
                self.call_discarder = None;

                // 自分が鳴いた場合、副露情報を保存し打牌待ちへ
                if Some(player) == self.seat_wind {
                    match call_type {
                        CallType::Ron => {
                            // ロンの場合は局終了イベントが続く
                        }
                        CallType::Pon | CallType::Chi | CallType::Daiminkan => {
                            // 副露情報を保存
                            self.melds.push(MeldInfo {
                                call_type: call_type.clone(),
                                tiles: tiles.clone(),
                            });
                            // ポン/チー/カンの場合、自分の手牌を更新
                            // （サーバ側で処理済みなので、次のイベントで反映）
                            self.is_my_turn = true;
                            self.drawn = None; // 鳴き後はdrawnなし
                            self.clear_riichi_selection();
                        }
                    }
                }
            }

            ServerEvent::PlayerRiichi { player } => {
                // 自分がリーチした場合
                if Some(player) == self.seat_wind {
                    self.is_riichi = true;
                    self.can_riichi = false;
                    self.clear_riichi_selection();
                }
            }

            ServerEvent::HandUpdated { hand } => {
                self.hand = hand;
                self.hand.sort();
            }

            ServerEvent::RoundWon {
                winner,
                loser,
                winning_tile,
                scores,
                yaku_list,
                han,
                fu,
                score_points,
                rank_name,
                uradora_indicators,
            } => {
                self.scores = scores;
                let winner_name = self.wind_to_name(winner);
                let win_type = if loser.is_some() { "ロン" } else { "ツモ" };
                let loser_text = if let Some(l) = loser {
                    format!("（{}が放銃）", self.wind_to_name(l))
                } else {
                    String::new()
                };

                // 役一覧を構築
                let mut yaku_text = String::new();
                for (name, y_han) in &yaku_list {
                    if !yaku_text.is_empty() {
                        yaku_text.push_str("  ");
                    }
                    yaku_text.push_str(&format!("{} {}翻", name, y_han));
                }

                // 点数表示
                let rank_display = if rank_name.is_empty() {
                    format!("{}符{}翻", fu, han)
                } else {
                    format!("{}符{}翻 {}", fu, han, rank_name)
                };

                let uradora_text = if uradora_indicators.is_empty() {
                    String::new()
                } else {
                    let tiles: Vec<String> = uradora_indicators
                        .iter()
                        .map(|t| tile_to_string(*t))
                        .collect();
                    format!("\n裏ドラ表示: {}", tiles.join(" "))
                };

                let msg = format!(
                    "{}が{}和了！{}\n和了牌: {}{}\n{}\n{} → {}点",
                    winner_name,
                    win_type,
                    loser_text,
                    tile_to_string(winning_tile),
                    uradora_text,
                    yaku_text,
                    rank_display,
                    score_points
                );
                self.result_message = Some(msg);
                self.phase = GamePhase::RoundResult;
                self.is_my_turn = false;
                self.available_calls.clear();
                self.clear_riichi_selection();
            }

            ServerEvent::RoundDraw { scores, reason, tenpai } => {
                self.scores = scores;
                let reason_text = match reason {
                    DrawReason::Exhaustive => "荒牌流局",
                    DrawReason::FourWinds => "四風連打",
                    DrawReason::FourRiichi => "四家立直",
                    DrawReason::NineTerminals => "九種九牌",
                };
                let mut msg = format!("流局（{}）", reason_text);

                if !tenpai.is_empty() {
                    let tenpai_names: Vec<&str> = tenpai
                        .iter()
                        .map(|w| self.wind_to_name(*w))
                        .collect();
                    msg.push_str(&format!("\nテンパイ: {}", tenpai_names.join(", ")));
                }

                self.result_message = Some(msg);
                self.phase = GamePhase::RoundResult;
                self.is_my_turn = false;
                self.available_calls.clear();
                self.clear_riichi_selection();
            }
        }
    }

    fn clear_riichi_selection(&mut self) {
        self.riichi_selection_mode = false;
        self.riichi_selectable_tiles.clear();
        self.riichi_selectable_drawn = false;
        self.selected_tile = None;
        self.selected_drawn = false;
    }

    fn can_discard_for_riichi(&self, tile: Option<Tile>) -> bool {
        if self.drawn.is_none() {
            return false;
        }

        let mut hand = Hand::new(self.hand.clone(), self.drawn);
        match tile {
            Some(target) => {
                let drawn = hand.drawn();
                let tiles = hand.tiles_mut();
                let Some(idx) = tiles.iter().position(|t| *t == target) else {
                    return false;
                };
                tiles.remove(idx);
                if let Some(drawn_tile) = drawn {
                    tiles.push(drawn_tile);
                    tiles.sort();
                }
                hand.set_drawn(None);
            }
            None => {
                hand.set_drawn(None);
            }
        }

        match HandAnalyzer::new(&hand) {
            Ok(analyzer) => analyzer.shanten == 0,
            Err(_) => false,
        }
    }

    fn enter_riichi_selection(&mut self) {
        self.riichi_selection_mode = true;
        self.selected_tile = None;
        self.selected_drawn = false;
        self.riichi_selectable_tiles = self
            .hand
            .iter()
            .enumerate()
            .filter_map(|(idx, &tile)| self.can_discard_for_riichi(Some(tile)).then_some(idx))
            .collect();
        self.riichi_selectable_drawn = self.can_discard_for_riichi(None);
    }

    fn apply_local_discard_from_hand(&mut self, idx: usize) -> Tile {
        let discarded_tile = self.hand[idx];
        self.selected_tile = None;
        self.selected_drawn = false;
        if let Some(drawn_tile) = self.drawn.take() {
            self.hand.remove(idx);
            self.hand.push(drawn_tile);
            self.hand.sort();
        } else {
            self.hand.remove(idx);
        }
        discarded_tile
    }

    /// 入力処理: クリックで牌を選択し、アクションを返す
    pub fn handle_input(&mut self) -> Option<ClientAction> {
        if self.phase != GamePhase::Playing {
            return None;
        }

        if !self.available_calls.is_empty() {
            return self.handle_call_input();
        }

        if !self.is_my_turn {
            return None;
        }

        if self.is_riichi && self.drawn.is_some() && !self.can_tsumo {
            self.drawn.take();
            return Some(ClientAction::Discard { tile: None });
        }

        if is_mouse_button_pressed(MouseButton::Left) {
            let (mx, my) = mouse_position();

            if self.can_tsumo {
                let tsumo_x = 900.0;
                let tsumo_y = 720.0;
                let btn_w = 80.0;
                let btn_h = 40.0;
                if mx >= tsumo_x
                    && mx <= tsumo_x + btn_w
                    && my >= tsumo_y
                    && my <= tsumo_y + btn_h
                {
                    return Some(ClientAction::Tsumo);
                }
            }

            if self.can_riichi {
                let riichi_x = 1000.0;
                let riichi_y = 720.0;
                let btn_w = 80.0;
                let btn_h = 40.0;
                if mx >= riichi_x
                    && mx <= riichi_x + btn_w
                    && my >= riichi_y
                    && my <= riichi_y + btn_h
                {
                    if self.riichi_selection_mode {
                        self.clear_riichi_selection();
                    } else {
                        self.enter_riichi_selection();
                    }
                    return None;
                }
            }

            if self.is_riichi {
                return None;
            }

            let hand_start_x = 100.0;
            let hand_y = 680.0;
            let tile_w = 48.0;
            let tile_h = 68.0;
            let hand_len = self.hand.len();

            for i in 0..hand_len {
                let x = hand_start_x + i as f32 * tile_w;
                if mx >= x && mx <= x + tile_w && my >= hand_y && my <= hand_y + tile_h {
                    if self.riichi_selection_mode && !self.riichi_selectable_tiles.contains(&i) {
                        return None;
                    }

                    if self.selected_tile == Some(i) {
                        let discarded_tile = self.apply_local_discard_from_hand(i);
                        if self.riichi_selection_mode {
                            self.clear_riichi_selection();
                            return Some(ClientAction::Riichi {
                                tile: Some(discarded_tile),
                            });
                        }
                        return Some(ClientAction::Discard {
                            tile: Some(discarded_tile),
                        });
                    }

                    self.selected_tile = Some(i);
                    self.selected_drawn = false;
                    return None;
                }
            }

            if self.drawn.is_some() {
                let drawn_x = hand_start_x + hand_len as f32 * tile_w + 20.0;
                if mx >= drawn_x
                    && mx <= drawn_x + tile_w
                    && my >= hand_y
                    && my <= hand_y + tile_h
                {
                    if self.riichi_selection_mode && !self.riichi_selectable_drawn {
                        return None;
                    }

                    if self.selected_drawn {
                        self.selected_drawn = false;
                        self.drawn.take();
                        if self.riichi_selection_mode {
                            self.clear_riichi_selection();
                            return Some(ClientAction::Riichi { tile: None });
                        }
                        return Some(ClientAction::Discard { tile: None });
                    }

                    self.selected_drawn = true;
                    self.selected_tile = None;
                    return None;
                }
            }
        }

        None
    }

    /// 鳴きボタンの入力処理
    fn handle_call_input(&mut self) -> Option<ClientAction> {
        if !is_mouse_button_pressed(MouseButton::Left) {
            return None;
        }

        let (mx, my) = mouse_position();

        // 鳴きボタンの配置（画面下部中央付近）
        let base_x = 400.0;
        let base_y = 620.0;
        let btn_w = 100.0;
        let btn_h = 40.0;
        let btn_spacing = 10.0;

        let mut btn_idx = 0;

        for call in &self.available_calls {
            let x = base_x + btn_idx as f32 * (btn_w + btn_spacing);
            if mx >= x && mx <= x + btn_w && my >= base_y && my <= base_y + btn_h {
                match call {
                    AvailableCall::Ron => {
                        self.available_calls.clear();
                        return Some(ClientAction::Ron);
                    }
                    AvailableCall::Pon => {
                        self.available_calls.clear();
                        return Some(ClientAction::Pon);
                    }
                    AvailableCall::Chi { options } => {
                        // 最初の選択肢を使う（複数ある場合はMVPでは最初を選択）
                        if let Some(&tiles) = options.first() {
                            self.available_calls.clear();
                            return Some(ClientAction::Chi { tiles });
                        }
                    }
                }
            }
            btn_idx += 1;
        }

        // パスボタン（最後に配置）
        let pass_x = base_x + btn_idx as f32 * (btn_w + btn_spacing);
        if mx >= pass_x && mx <= pass_x + btn_w && my >= base_y && my <= base_y + btn_h {
            self.available_calls.clear();
            self.call_target_tile = None;
            return Some(ClientAction::Pass);
        }

        None
    }

    /// 風牌を相対位置（自分=0, 下家=1, 対面=2, 上家=3）に変換
    fn relative_player_index(&self, wind: Wind) -> usize {
        let my_idx = self
            .seat_wind
            .map(|w| w.to_index())
            .unwrap_or(0);
        let their_idx = wind.to_index();
        (their_idx + 4 - my_idx) % 4
    }

    /// 風牌を日本語の名前に変換
    fn wind_to_name(&self, wind: Wind) -> &'static str {
        match wind {
            Wind::East => "東家",
            Wind::South => "南家",
            Wind::West => "西家",
            Wind::North => "北家",
        }
    }
}

/// 牌を文字列に変換
pub fn tile_to_string(tile: Tile) -> String {
    let tile_type = tile.get();
    let names = [
        "一萬", "二萬", "三萬", "四萬", "五萬", "六萬", "七萬", "八萬", "九萬",
        "一筒", "二筒", "三筒", "四筒", "五筒", "六筒", "七筒", "八筒", "九筒",
        "一索", "二索", "三索", "四索", "五索", "六索", "七索", "八索", "九索",
        "東", "南", "西", "北", "白", "發", "中",
    ];
    if (tile_type as usize) < names.len() {
        let name = names[tile_type as usize].to_string();
        if tile.is_red_dora() {
            format!("{}(赤)", name)
        } else {
            name
        }
    } else {
        "?".to_string()
    }
}




#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_enter_riichi_selection_marks_only_tenpai_discards() {
        let mut state = GameState::new();
        let hand = Hand::from("123m123p123s45z67m 8m");
        state.hand = hand.tiles().to_vec();
        state.hand.sort();
        state.drawn = hand.drawn();
        state.enter_riichi_selection();

        assert_eq!(state.riichi_selectable_tiles.len(), 2);
        assert_eq!(state.hand[state.riichi_selectable_tiles[0]], Tile::new(Tile::Z4));
        assert_eq!(state.hand[state.riichi_selectable_tiles[1]], Tile::new(Tile::Z5));
        assert!(!state.riichi_selectable_drawn);
    }

    #[test]
    fn test_can_discard_for_riichi_rejects_non_tenpai_discard() {
        let mut state = GameState::new();
        let hand = Hand::from("123m123p123s45z67m 8m");
        state.hand = hand.tiles().to_vec();
        state.hand.sort();
        state.drawn = hand.drawn();

        assert!(!state.can_discard_for_riichi(None));
        assert!(state.can_discard_for_riichi(Some(Tile::new(Tile::Z4))));
        assert!(state.can_discard_for_riichi(Some(Tile::new(Tile::Z5))));
    }
}


