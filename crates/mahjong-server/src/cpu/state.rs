//! CPUが保持するゲーム状態
//!
//! ServerEvent のストリームだけから構築される、プレイヤー視点のゲーム状態。
//! プレイヤーが画面から読み取れる情報と同等の情報のみを保持する。

use mahjong_core::hand_info::meld::{Meld, MeldFrom, MeldType};
use mahjong_core::tile::{Tile, Wind};

use crate::protocol::{AvailableCall, CallType, ServerEvent};

/// CPUが保持するゲーム状態（全て ServerEvent から構築）
#[derive(Debug, Clone)]
pub struct CpuGameState {
    // --- 自分の情報 ---
    /// 自分の手牌
    pub my_hand: Vec<Tile>,
    /// ツモった牌
    pub my_drawn: Option<Tile>,
    /// 自分の座席の風
    pub my_seat_wind: Wind,
    /// 自分がリーチしているか
    pub is_riichi: bool,

    // --- TileDrawn イベントで受け取るフラグ ---
    /// ツモ和了可能か
    pub can_tsumo: bool,
    /// リーチ宣言可能か
    pub can_riichi: bool,
    /// フリテン状態か
    pub is_furiten: bool,

    // --- 公開情報（全員分）---
    /// 各プレイヤーの得点
    pub scores: [i32; 4],
    /// 各プレイヤーの捨て牌（風のインデックス順: 東=0, 南=1, 西=2, 北=3）
    pub all_discards: [Vec<Tile>; 4],
    /// 各プレイヤーのリーチ状態
    pub player_riichi: [bool; 4],
    /// 各プレイヤーの副露情報
    pub player_melds: [Vec<Meld>; 4],
    /// ドラ表示牌
    pub dora_indicators: Vec<Tile>,
    /// 場風
    pub prevailing_wind: Wind,
    /// 山の残り枚数
    pub remaining_tiles: usize,
    /// 本場数
    pub honba: usize,
    /// 供託リーチ棒
    pub riichi_sticks: usize,

    // --- 鳴き関連 ---
    /// 現在利用可能な鳴きアクション
    pub pending_calls: Vec<AvailableCall>,
    /// 鳴き対象の牌
    pub pending_call_tile: Option<Tile>,

    // --- 鳴き後打牌フラグ ---
    /// 鳴き後に打牌が必要か
    pub need_discard_after_call: bool,
    /// 直前の鳴きがカン系（嶺上ツモ待ち）か
    pub pending_kan_draw: bool,
}

impl CpuGameState {
    /// 空の初期状態を作成する
    pub fn new() -> Self {
        CpuGameState {
            my_hand: Vec::new(),
            my_drawn: None,
            my_seat_wind: Wind::East,
            is_riichi: false,
            can_tsumo: false,
            can_riichi: false,
            is_furiten: false,
            scores: [0; 4],
            all_discards: [Vec::new(), Vec::new(), Vec::new(), Vec::new()],
            player_riichi: [false; 4],
            player_melds: [Vec::new(), Vec::new(), Vec::new(), Vec::new()],
            dora_indicators: Vec::new(),
            prevailing_wind: Wind::East,
            remaining_tiles: 0,
            honba: 0,
            riichi_sticks: 0,
            pending_calls: Vec::new(),
            pending_call_tile: None,
            need_discard_after_call: false,
            pending_kan_draw: false,
        }
    }

    /// 風からプレイヤーインデックス（0=東, 1=南, 2=西, 3=北）を取得する
    pub fn wind_to_index(wind: Wind) -> usize {
        match wind {
            Wind::East => 0,
            Wind::South => 1,
            Wind::West => 2,
            Wind::North => 3,
        }
    }

    /// ServerEvent を処理してゲーム状態を更新する
    pub fn update(&mut self, event: &ServerEvent) {
        match event {
            ServerEvent::GameStarted {
                seat_wind,
                hand,
                scores,
                prevailing_wind,
                dora_indicators,
                honba,
                riichi_sticks,
                ..
            } => {
                // 新しい局の開始: 状態をリセット
                self.my_hand = hand.clone();
                self.my_drawn = None;
                self.my_seat_wind = *seat_wind;
                self.is_riichi = false;
                self.can_tsumo = false;
                self.can_riichi = false;
                self.is_furiten = false;
                self.scores = *scores;
                self.all_discards = [Vec::new(), Vec::new(), Vec::new(), Vec::new()];
                self.player_riichi = [false; 4];
                self.player_melds = [Vec::new(), Vec::new(), Vec::new(), Vec::new()];
                self.dora_indicators = dora_indicators.clone();
                self.prevailing_wind = *prevailing_wind;
                self.remaining_tiles = 70; // 136 - 14(王牌) - 13*4(配牌) = 70
                self.honba = *honba;
                self.riichi_sticks = *riichi_sticks;
                self.pending_calls.clear();
                self.pending_call_tile = None;
                self.need_discard_after_call = false;
                self.pending_kan_draw = false;
            }

            ServerEvent::TileDrawn {
                tile,
                remaining_tiles,
                can_tsumo,
                can_riichi,
                is_furiten,
            } => {
                self.my_drawn = Some(*tile);
                self.remaining_tiles = *remaining_tiles;
                self.can_tsumo = *can_tsumo;
                self.can_riichi = *can_riichi;
                self.is_furiten = *is_furiten;
                self.need_discard_after_call = false;
            }

            ServerEvent::OtherPlayerDrew {
                remaining_tiles, ..
            } => {
                self.remaining_tiles = *remaining_tiles;
            }

            ServerEvent::TileDiscarded {
                player,
                tile,
                is_tsumogiri,
            } => {
                let idx = Self::wind_to_index(*player);
                self.all_discards[idx].push(*tile);

                // 自分が捨てた場合、手牌を正しく更新する
                if *player == self.my_seat_wind {
                    if *is_tsumogiri {
                        // ツモ切り: ツモ牌を捨てるだけ
                        self.my_drawn = None;
                    } else {
                        // 手出し: 手牌から捨てた牌を除去し、ツモ牌を手牌に加える
                        if let Some(pos) = self.my_hand.iter().position(|t| *t == *tile) {
                            self.my_hand.remove(pos);
                        }
                        if let Some(drawn) = self.my_drawn {
                            self.my_hand.push(drawn);
                            self.my_hand.sort();
                        }
                        self.my_drawn = None;
                    }
                }
            }

            ServerEvent::CallAvailable {
                tile, calls, ..
            } => {
                self.pending_calls = calls.clone();
                self.pending_call_tile = Some(*tile);
            }

            ServerEvent::PlayerCalled {
                player,
                call_type,
                tiles,
                called_tile,
                ..
            } => {
                let idx = Self::wind_to_index(*player);
                let category = match call_type {
                    CallType::Chi => MeldType::Chi,
                    CallType::Pon => MeldType::Pon,
                    CallType::Ankan | CallType::Daiminkan => MeldType::Kan,
                    CallType::Kakan => MeldType::Kakan,
                    CallType::Ron => MeldType::Pon, // フォールバック（使われない）
                };
                let from = match call_type {
                    CallType::Ankan => MeldFrom::Myself,
                    _ => MeldFrom::Unknown,
                };
                self.player_melds[idx].push(Meld {
                    tiles: tiles.clone(),
                    category,
                    from,
                    called_tile: Some(*called_tile),
                });

                self.pending_calls.clear();
                self.pending_call_tile = None;
                // カン系（嶺上ツモ待ち）かどうかを記録する
                self.pending_kan_draw = matches!(
                    call_type,
                    CallType::Ankan | CallType::Daiminkan | CallType::Kakan
                );
            }

            ServerEvent::PlayerRiichi {
                player,
                scores,
                riichi_sticks,
            } => {
                let idx = Self::wind_to_index(*player);
                self.player_riichi[idx] = true;
                self.scores = *scores;
                self.riichi_sticks = *riichi_sticks;
                if *player == self.my_seat_wind {
                    self.is_riichi = true;
                }
            }

            ServerEvent::DoraIndicatorsUpdated { dora_indicators } => {
                self.dora_indicators = dora_indicators.clone();
            }

            ServerEvent::HandUpdated { hand } => {
                self.my_hand = hand.clone();
                self.my_drawn = None;
                if self.pending_kan_draw {
                    // カン系: 嶺上ツモ（TileDrawn）が来るまで打牌不要
                    self.pending_kan_draw = false;
                } else {
                    // ポン/チー後: 打牌が必要
                    self.need_discard_after_call = true;
                }
            }

            ServerEvent::RoundWon { scores, .. } => {
                self.scores = *scores;
            }

            ServerEvent::RoundDraw { scores, .. } => {
                self.scores = *scores;
            }

            ServerEvent::NineTerminalsAvailable => {
                // 状態更新不要（decide_nine_terminals で対応）
            }
        }
    }

    /// 場に見えている牌の枚数を種類ごとにカウントする
    /// （自分の手牌 + 全員の捨て牌 + 全員の副露）
    pub fn visible_tile_counts(&self) -> [u8; 34] {
        let mut counts = [0u8; 34];

        // 自分の手牌
        for tile in &self.my_hand {
            counts[tile.get() as usize] += 1;
        }
        if let Some(drawn) = self.my_drawn {
            counts[drawn.get() as usize] += 1;
        }

        // 全員の捨て牌
        for discards in &self.all_discards {
            for tile in discards {
                counts[tile.get() as usize] += 1;
            }
        }

        // 全員の副露
        for melds in &self.player_melds {
            for meld in melds {
                for tile in &meld.tiles {
                    counts[tile.get() as usize] += 1;
                }
            }
        }

        // ドラ表示牌
        for tile in &self.dora_indicators {
            counts[tile.get() as usize] += 1;
        }

        counts
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::ServerEvent;
    use mahjong_core::tile::{Tile, Wind};

    #[test]
    fn test_game_started_initializes_state() {
        let mut state = CpuGameState::new();
        let hand = vec![
            Tile::new(Tile::M1),
            Tile::new(Tile::M2),
            Tile::new(Tile::M3),
        ];

        state.update(&ServerEvent::GameStarted {
            seat_wind: Wind::South,
            hand: hand.clone(),
            scores: [25000; 4],
            prevailing_wind: Wind::East,
            dora_indicators: vec![Tile::new(Tile::M5)],
            round_number: 0,
            honba: 0,
            riichi_sticks: 0,
        });

        assert_eq!(state.my_seat_wind, Wind::South);
        assert_eq!(state.my_hand, hand);
        assert_eq!(state.scores, [25000; 4]);
        assert_eq!(state.prevailing_wind, Wind::East);
        assert_eq!(state.dora_indicators.len(), 1);
    }

    #[test]
    fn test_tile_drawn_updates_state() {
        let mut state = CpuGameState::new();
        state.update(&ServerEvent::TileDrawn {
            tile: Tile::new(Tile::P5),
            remaining_tiles: 50,
            can_tsumo: false,
            can_riichi: true,
            is_furiten: false,
        });

        assert_eq!(state.my_drawn, Some(Tile::new(Tile::P5)));
        assert_eq!(state.remaining_tiles, 50);
        assert!(!state.can_tsumo);
        assert!(state.can_riichi);
    }

    #[test]
    fn test_tile_discarded_updates_discards() {
        let mut state = CpuGameState::new();
        state.my_seat_wind = Wind::East;

        state.update(&ServerEvent::TileDiscarded {
            player: Wind::South,
            tile: Tile::new(Tile::Z1),
            is_tsumogiri: false,
        });

        assert_eq!(state.all_discards[1].len(), 1);
        assert_eq!(state.all_discards[1][0], Tile::new(Tile::Z1));
    }

    #[test]
    fn test_player_riichi_updates_state() {
        let mut state = CpuGameState::new();
        state.my_seat_wind = Wind::East;

        state.update(&ServerEvent::PlayerRiichi {
            player: Wind::East,
            scores: [24000, 25000, 25000, 25000],
            riichi_sticks: 1,
        });

        assert!(state.is_riichi);
        assert!(state.player_riichi[0]);
        assert_eq!(state.scores[0], 24000);
        assert_eq!(state.riichi_sticks, 1);
    }

    #[test]
    fn test_visible_tile_counts() {
        let mut state = CpuGameState::new();
        state.my_hand = vec![Tile::new(Tile::M1), Tile::new(Tile::M1)];
        state.all_discards[0] = vec![Tile::new(Tile::M1)];

        let counts = state.visible_tile_counts();
        assert_eq!(counts[Tile::M1 as usize], 3);
    }

    #[test]
    fn test_self_discard_hand_updates_my_hand() {
        let mut state = CpuGameState::new();
        state.my_seat_wind = Wind::East;
        state.my_hand = vec![
            Tile::new(Tile::M1),
            Tile::new(Tile::M2),
            Tile::new(Tile::M3),
        ];
        state.my_drawn = Some(Tile::new(Tile::P5));

        // 手出し: M1を捨てる → 手牌からM1が消え、P5が手牌に入る
        state.update(&ServerEvent::TileDiscarded {
            player: Wind::East,
            tile: Tile::new(Tile::M1),
            is_tsumogiri: false,
        });

        assert!(state.my_drawn.is_none());
        assert_eq!(state.my_hand.len(), 3);
        assert!(!state.my_hand.contains(&Tile::new(Tile::M1)));
        assert!(state.my_hand.contains(&Tile::new(Tile::P5)));
    }

    #[test]
    fn test_self_tsumogiri_keeps_my_hand() {
        let mut state = CpuGameState::new();
        state.my_seat_wind = Wind::East;
        state.my_hand = vec![
            Tile::new(Tile::M1),
            Tile::new(Tile::M2),
            Tile::new(Tile::M3),
        ];
        state.my_drawn = Some(Tile::new(Tile::P5));

        // ツモ切り: P5を捨てる → 手牌はそのまま
        state.update(&ServerEvent::TileDiscarded {
            player: Wind::East,
            tile: Tile::new(Tile::P5),
            is_tsumogiri: true,
        });

        assert!(state.my_drawn.is_none());
        assert_eq!(state.my_hand.len(), 3);
        assert!(state.my_hand.contains(&Tile::new(Tile::M1)));
        assert!(!state.my_hand.contains(&Tile::new(Tile::P5)));
    }
}
