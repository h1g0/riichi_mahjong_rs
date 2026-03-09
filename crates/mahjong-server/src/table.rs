//! 卓の状態管理
//!
//! 半荘（東風戦/東南戦）を通した状態を管理する。
//! 局の生成・進行・終了判定を行う。

use mahjong_core::tile::Wind;

use crate::protocol::{ClientAction, ServerEvent};
use crate::round::{CallResponse, Round, RoundResult, TurnPhase};

/// ゲームの設定
#[derive(Debug, Clone)]
pub struct GameSettings {
    /// 初期持ち点
    pub initial_score: i32,
    /// 東風戦(1)か東南戦(2)か
    pub round_count: u8,
}

impl Default for GameSettings {
    fn default() -> Self {
        GameSettings {
            initial_score: 25000,
            round_count: 1, // 東風戦
        }
    }
}

/// 卓の状態
pub struct Table {
    /// ゲーム設定
    pub settings: GameSettings,
    /// 現在の局
    pub round: Option<Round>,
    /// 場風
    pub prevailing_wind: Wind,
    /// 局番号（0-based: 東1局=0, 東2局=1, ...）
    pub round_number: usize,
    /// 本場数
    pub honba: usize,
    /// 親のプレイヤーインデックス（0-3）
    pub dealer: usize,
    /// 各プレイヤーの点数
    pub scores: [i32; 4],
    /// ゲームが終了したか
    pub is_game_over: bool,
}

impl Table {
    /// 新しい卓を作成する
    pub fn new(settings: GameSettings) -> Self {
        let initial_score = settings.initial_score;
        Table {
            settings,
            round: None,
            prevailing_wind: Wind::East,
            round_number: 0,
            honba: 0,
            dealer: 0,
            scores: [initial_score; 4],
            is_game_over: false,
        }
    }

    /// 新しい局を開始する
    pub fn start_round(&mut self) {
        let round = Round::new(
            self.prevailing_wind,
            self.dealer,
            self.scores,
            self.honba,
            self.round_number,
        );
        self.round = Some(round);
    }

    /// 現在の局への参照を取得する
    pub fn current_round(&self) -> Option<&Round> {
        self.round.as_ref()
    }

    /// 現在の局への可変参照を取得する
    pub fn current_round_mut(&mut self) -> Option<&mut Round> {
        self.round.as_mut()
    }

    /// イベントを取り出す
    pub fn drain_events(&mut self) -> Vec<(usize, ServerEvent)> {
        match self.round.as_mut() {
            Some(round) => round.drain_events(),
            None => Vec::new(),
        }
    }

    /// クライアントアクションを処理する
    pub fn handle_action(&mut self, player_idx: usize, action: ClientAction) -> bool {
        let round = match self.round.as_mut() {
            Some(r) => r,
            None => return false,
        };

        match action {
            // === 手番アクション（current_player のみ） ===
            ClientAction::Discard { tile } => {
                if round.current_player != player_idx {
                    return false;
                }
                if round.phase != TurnPhase::WaitForDiscard {
                    return false;
                }
                round.do_discard(tile)
            }
            ClientAction::Tsumo => {
                if round.current_player != player_idx {
                    return false;
                }
                round.do_tsumo()
            }
            ClientAction::Riichi { tile } => {
                if round.current_player != player_idx {
                    return false;
                }
                round.do_riichi(tile)
            }

            // === 鳴きアクション（WaitForCalls フェーズで対象プレイヤーのみ） ===
            ClientAction::Ron => {
                round.respond_to_call(player_idx, CallResponse::Ron)
            }
            ClientAction::Pon => {
                round.respond_to_call(player_idx, CallResponse::Pon)
            }
            ClientAction::Chi { tiles } => {
                round.respond_to_call(
                    player_idx,
                    CallResponse::Chi {
                        hand_tile_types: tiles,
                    },
                )
            }
            ClientAction::Pass => {
                round.respond_to_call(player_idx, CallResponse::Pass)
            }

            // カンは Phase 9 で実装予定
            ClientAction::Kan { .. } => false,
        }
    }

    /// 自動プレイヤーのターンを進める
    pub fn advance_auto_player(&mut self) -> bool {
        match self.round.as_mut() {
            Some(round) => round.advance_auto_player(),
            None => false,
        }
    }

    /// 局が終了した場合に後処理を行う
    /// 点数更新、親交代、局の進行を処理する
    pub fn finish_round(&mut self) {
        let result = {
            let round = match self.round.as_ref() {
                Some(r) if r.is_over() => r,
                _ => return,
            };
            round.result.clone()
        };

        let round = self.round.as_ref().unwrap();
        self.scores = round.get_scores();

        match result {
            Some(RoundResult::ExhaustiveDraw) | Some(RoundResult::SpecialDraw) => {
                // 流局: 本場を増やし、親交代して局を進める
                self.honba += 1;
                self.dealer = (self.dealer + 1) % 4;
                self.advance_round_number();
            }
            Some(RoundResult::Tsumo { winner, .. }) | Some(RoundResult::Ron { winner, .. }) => {
                if winner == self.dealer {
                    // 親が上がった場合は連荘（同じ局、本場+1）
                    self.honba += 1;
                } else {
                    // 親が上がっていなければ親交代、本場リセット
                    self.honba = 0;
                    self.dealer = (self.dealer + 1) % 4;
                    self.advance_round_number();
                }
            }
            None => {}
        }

        self.round = None;
    }

    /// 局番号を進める
    fn advance_round_number(&mut self) {
        self.round_number += 1;
        let max_rounds = self.settings.round_count as usize * 4;
        if self.round_number >= max_rounds {
            self.is_game_over = true;
        }

        // 場風を更新
        self.prevailing_wind = Wind::from_index(self.round_number / 4);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_table_new() {
        let table = Table::new(GameSettings::default());
        assert_eq!(table.prevailing_wind, Wind::East);
        assert_eq!(table.dealer, 0);
        assert_eq!(table.scores, [25000; 4]);
        assert!(!table.is_game_over);
        assert!(table.round.is_none());
    }

    #[test]
    fn test_table_start_round() {
        let mut table = Table::new(GameSettings::default());
        table.start_round();
        assert!(table.round.is_some());

        let round = table.current_round().unwrap();
        assert_eq!(round.prevailing_wind, Wind::East);
        assert_eq!(round.current_player, 0);
    }

    #[test]
    fn test_table_play_round_to_end() {
        let mut table = Table::new(GameSettings::default());
        table.start_round();

        // 全員ツモ切りで局を最後まで進める
        let round = table.current_round_mut().unwrap();
        round.play_to_end();

        assert!(table.current_round().unwrap().is_over());

        table.finish_round();
        assert!(table.round.is_none());
        assert_eq!(table.honba, 1); // 流局なので本場が増える
    }

    #[test]
    fn test_table_handle_discard() {
        let mut table = Table::new(GameSettings::default());
        table.start_round();
        table.drain_events();

        // ツモフェーズ
        {
            let round = table.current_round_mut().unwrap();
            round.do_draw();
        }
        table.drain_events();

        // プレイヤー0がツモ切り
        assert!(table.handle_action(0, ClientAction::Discard { tile: None }));

        // WaitForCallsの場合は全員パスさせる
        {
            let round = table.current_round_mut().unwrap();
            if round.phase == TurnPhase::WaitForCalls {
                for i in 0..4 {
                    if let Some(ref cs) = round.call_state {
                        if !cs.responded[i] {
                            round.respond_to_call(i, CallResponse::Pass);
                            if round.call_state.is_none() {
                                break;
                            }
                        }
                    }
                }
            }
        }

        // 手番がプレイヤー1に移る
        let round = table.current_round().unwrap();
        assert_eq!(round.current_player, 1);
    }

    #[test]
    fn test_table_wrong_player_action() {
        let mut table = Table::new(GameSettings::default());
        table.start_round();
        table.drain_events();

        {
            let round = table.current_round_mut().unwrap();
            round.do_draw();
        }

        // プレイヤー1は手番ではないのでfalse
        assert!(!table.handle_action(1, ClientAction::Discard { tile: None }));
    }

    #[test]
    fn test_table_east_wind_game() {
        let mut table = Table::new(GameSettings {
            initial_score: 25000,
            round_count: 1, // 東風戦（4局）
        });

        // 4局連続で流局させる
        for _ in 0..4 {
            table.start_round();
            let round = table.current_round_mut().unwrap();
            round.play_to_end();
            table.finish_round();
        }

        assert!(table.is_game_over);
    }
}
