//! 卓の状態管理
//!
//! 半荘（東風戦/東南戦）を通した状態を管理する。
//! 局の生成・進行・終了判定を行う。

use mahjong_core::settings::Settings;
use mahjong_core::tile::{Tile, Wind};

use crate::protocol::{ClientAction, ServerEvent};
use crate::round::{CallResponse, Round, RoundResult, TurnPhase};

/// ゲームの設定
#[derive(Debug, Clone)]
pub struct GameSettings {
    /// 初期持ち点
    pub initial_score: i32,
    /// 東風戦(1)か東南戦(2)か
    pub round_count: u8,
    /// ルール設定
    pub rules: Settings,
}

impl Default for GameSettings {
    fn default() -> Self {
        GameSettings {
            initial_score: 25000,
            round_count: 1, // 東風戦
            rules: Settings::new(),
        }
    }
}

impl GameSettings {
    /// ルール設定から標準の持ち点でゲーム設定を作る
    ///
    /// 持ち点はルールから決まる（四麻25000点・三麻35000点）。
    /// ルーム作成（`CreateRoom`）やローカル対局設定からの変換に使う。
    pub fn with_rules(round_count: u8, rules: Settings) -> Self {
        let initial_score = if rules.three_player { 35000 } else { 25000 };
        GameSettings {
            initial_score,
            round_count,
            rules,
        }
    }

    /// 三麻の標準設定を返す（35000点持ち・東風戦）
    pub fn sanma_default() -> Self {
        Self::with_rules(
            1, // 東風戦（東1〜3局）
            Settings {
                three_player: true,
                ..Settings::new()
            },
        )
    }
}

/// 卓の状態
pub struct Table {
    /// ゲーム設定
    pub settings: GameSettings,
    /// 現在の局
    pub round: Option<Round>,
    /// 場風
    pub round_wind: Wind,
    /// 局番号（0-based: 東1局=0, 東2局=1, ...）
    pub round_number: usize,
    /// 本場数
    pub honba: usize,
    /// 場に出ている供託リーチ棒の本数
    pub riichi_sticks: usize,
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
        let player_count = settings.rules.player_count();
        // 三麻ではダミー席（シート3）の点数は常に0
        let scores = std::array::from_fn(|i| if i < player_count { initial_score } else { 0 });
        Table {
            settings,
            round: None,
            round_wind: Wind::East,
            round_number: 0,
            honba: 0,
            riichi_sticks: 0,
            dealer: 0,
            scores,
            is_game_over: false,
        }
    }

    /// ゲーム全体の局数を返す
    ///
    /// 四麻: 東風戦=4, 東南戦=8
    /// 三麻: 東風戦=3, 東南戦=6
    fn total_rounds(&self) -> usize {
        self.settings.round_count as usize * self.player_count()
    }

    /// プレイヤー人数を返す（四麻=4、三麻=3）
    fn player_count(&self) -> usize {
        self.settings.rules.player_count()
    }

    /// 新しい局を開始する
    pub fn start_round(&mut self) {
        let round = Round::new(
            self.round_wind,
            self.dealer,
            self.scores,
            self.honba,
            self.riichi_sticks,
            self.round_number,
            self.total_rounds(),
            self.settings.rules.clone(),
        );
        self.round = Some(round);
    }

    /// シード値を指定して新しい局を開始する
    ///
    /// 牌山が決定的になるため、シミュレーション・テストでの再現に使用する。
    pub fn start_round_with_seed(&mut self, seed: u64) {
        let round = Round::new_with_seed(
            seed,
            self.round_wind,
            self.dealer,
            self.scores,
            self.honba,
            self.riichi_sticks,
            self.round_number,
            self.total_rounds(),
            self.settings.rules.clone(),
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
            ClientAction::Ron => round.respond_to_call(player_idx, CallResponse::Ron),
            ClientAction::Pon { tiles } => round.respond_to_call(
                player_idx,
                CallResponse::Pon {
                    hand_tile_types: tiles,
                },
            ),
            ClientAction::Chi { tiles } => round.respond_to_call(
                player_idx,
                CallResponse::Chi {
                    hand_tile_types: tiles,
                },
            ),
            ClientAction::Pass => round.respond_to_call(player_idx, CallResponse::Pass),

            ClientAction::Kan { tile_index } => {
                if round.phase == TurnPhase::WaitForCalls {
                    round.respond_to_call(player_idx, CallResponse::Daiminkan)
                } else if round.current_player == player_idx
                    && round.phase == TurnPhase::WaitForDiscard
                {
                    if tile_index >= Tile::LEN {
                        return false;
                    }
                    round.do_kan(tile_index as u32)
                } else {
                    false
                }
            }

            // === 北抜きアクション（三麻のみ） ===
            ClientAction::Pei => {
                if round.current_player != player_idx {
                    return false;
                }
                round.do_pei()
            }

            // === 九種九牌アクション ===
            ClientAction::NineTerminals { declare } => round.do_nine_terminals(player_idx, declare),
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
        let (result, scores, riichi_sticks) = {
            let round = match self.round.as_ref() {
                Some(r) if r.is_over() => r,
                _ => return,
            };
            (
                round.result.clone(),
                round.get_scores(),
                round.riichi_sticks,
            )
        };

        self.scores = scores;
        self.riichi_sticks = riichi_sticks;

        // 誰かが箱割れしていたらその時点でゲーム終了（0点は許容）
        if self.scores.iter().any(|&score| score < 0) {
            self.is_game_over = true;
            self.round = None;
            return;
        }

        match result {
            Some(RoundResult::ExhaustiveDraw { dealer_tenpai }) => {
                self.honba += 1;
                if dealer_tenpai {
                    // 親がテンパイなら連荘（親交代しない、局も進めない）
                } else {
                    // 親がノーテンなら親交代して局を進める
                    self.dealer = (self.dealer + 1) % self.player_count();
                    self.advance_round_number();
                }
            }
            Some(RoundResult::SpecialDraw) => {
                // 途中流局: 本場を増やし、局は進めない（連荘扱い）
                self.honba += 1;
            }
            Some(RoundResult::Tsumo { winner, .. }) => {
                if winner == self.dealer {
                    self.honba += 1;
                } else {
                    self.honba = 0;
                    self.dealer = (self.dealer + 1) % self.player_count();
                    self.advance_round_number();
                }
            }
            Some(RoundResult::Ron { winners, .. }) => {
                // 和了者の中に親がいれば連荘（1人ロンでも複数ロンでも共通）
                if winners.contains(&self.dealer) {
                    self.honba += 1;
                } else {
                    self.honba = 0;
                    self.dealer = (self.dealer + 1) % self.player_count();
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
        if self.round_number >= self.total_rounds() {
            self.is_game_over = true;
        }

        // 場風を更新（三麻は3局ごと、四麻は4局ごとに進む）
        self.round_wind = Wind::from_index(self.round_number / self.player_count());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_table_new() {
        let table = Table::new(GameSettings::default());
        assert_eq!(table.round_wind, Wind::East);
        assert_eq!(table.dealer, 0);
        assert_eq!(table.scores, [25000; 4]);
        assert_eq!(table.riichi_sticks, 0);
        assert!(!table.is_game_over);
        assert!(table.round.is_none());
    }

    #[test]
    fn test_table_start_round() {
        let mut table = Table::new(GameSettings::default());
        table.start_round();
        assert!(table.round.is_some());

        let round = table.current_round().unwrap();
        assert_eq!(round.round_wind, Wind::East);
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
    fn test_table_carries_riichi_sticks_across_draw() {
        let mut table = Table::new(GameSettings::default());
        table.riichi_sticks = 2;
        table.start_round();

        let round = table.current_round_mut().unwrap();
        round.riichi_sticks = 3;
        round.phase = TurnPhase::RoundOver;
        round.result = Some(RoundResult::ExhaustiveDraw {
            dealer_tenpai: false,
        });

        table.finish_round();
        assert_eq!(table.riichi_sticks, 3);
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
                    if let Some(ref cs) = round.call_state
                        && !cs.responded[i]
                    {
                        round.respond_to_call(i, CallResponse::Pass);
                        if round.call_state.is_none() {
                            break;
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
            ..Default::default()
        });

        // 4局連続でノーテン流局（親交代あり）させてゲーム終了を確認する
        for _ in 0..4 {
            table.start_round();
            let round = table.current_round_mut().unwrap();
            round.phase = TurnPhase::RoundOver;
            round.result = Some(RoundResult::ExhaustiveDraw {
                dealer_tenpai: false,
            });
            table.finish_round();
        }

        assert!(table.is_game_over);
    }

    #[test]
    fn test_game_settings_with_rules() {
        // ルール構造体を丸ごと引き継ぎ、持ち点はルールから決まる
        let rules = Settings {
            three_player: true,
            nuki_dora: false,
            triple_ron_draw: true,
            ..Settings::new()
        };
        let settings = GameSettings::with_rules(2, rules.clone());
        assert_eq!(settings.initial_score, 35000);
        assert_eq!(settings.round_count, 2);
        assert_eq!(settings.rules, rules);

        let four_player = GameSettings::with_rules(1, Settings::new());
        assert_eq!(four_player.initial_score, 25000);
    }

    #[test]
    fn test_sanma_table_new() {
        let table = Table::new(GameSettings::sanma_default());
        // 35000点持ち・ダミー席3は0点
        assert_eq!(table.scores, [35000, 35000, 35000, 0]);
        assert_eq!(table.round_wind, Wind::East);
        assert!(!table.is_game_over);
    }

    #[test]
    fn test_sanma_east_wind_game_is_three_rounds() {
        let mut table = Table::new(GameSettings::sanma_default());

        // 3局連続でノーテン流局（親交代あり）させてゲーム終了を確認する
        for i in 0..3 {
            assert!(
                !table.is_game_over,
                "{}局目の前にゲームが終了している",
                i + 1
            );
            table.start_round();
            let round = table.current_round_mut().unwrap();
            round.phase = TurnPhase::RoundOver;
            round.result = Some(RoundResult::ExhaustiveDraw {
                dealer_tenpai: false,
            });
            table.finish_round();
        }

        assert!(table.is_game_over, "三麻の東風戦が3局で終了しない");
    }

    #[test]
    fn test_sanma_dealer_rotation_wraps_at_three() {
        let mut table = Table::new(GameSettings {
            round_count: 2, // 東南戦（6局）にして親が一周するのを確認
            ..GameSettings::sanma_default()
        });

        let mut dealers = Vec::new();
        for _ in 0..4 {
            table.start_round();
            dealers.push(table.dealer);
            let round = table.current_round_mut().unwrap();
            round.phase = TurnPhase::RoundOver;
            round.result = Some(RoundResult::ExhaustiveDraw {
                dealer_tenpai: false,
            });
            table.finish_round();
        }

        // 親は 0→1→2→0 と3人で循環する
        assert_eq!(dealers, vec![0, 1, 2, 0]);
        // 東1〜3局の後は南入する
        assert_eq!(table.round_wind, Wind::South);
    }

    #[test]
    fn test_table_game_over_when_score_is_negative() {
        let mut table = Table::new(GameSettings::default());
        table.start_round();

        let round = table.current_round_mut().unwrap();
        round.players[0].score = -100;
        round.phase = TurnPhase::RoundOver;
        round.result = Some(RoundResult::Ron {
            winners: vec![1],
            loser: 0,
            winning_tile: Tile::new(Tile::M1),
        });

        table.finish_round();

        assert!(table.is_game_over);
        assert!(table.round.is_none());
        assert_eq!(table.scores[0], -100);
    }

    #[test]
    fn test_table_methods_without_round_are_noops() {
        let mut table = Table::new(GameSettings::default());

        assert!(table.current_round().is_none());
        assert!(table.current_round_mut().is_none());
        assert!(table.drain_events().is_empty());
        assert!(!table.advance_auto_player());
        assert!(!table.handle_action(0, ClientAction::Tsumo));

        table.finish_round();
        assert!(!table.is_game_over);
        assert_eq!(table.round_number, 0);
        assert_eq!(table.honba, 0);
    }

    #[test]
    fn test_table_drain_events_delegates_to_round() {
        let mut table = Table::new(GameSettings::default());
        table.start_round();

        let events = table.drain_events();
        assert!(!events.is_empty());
        assert!(table.drain_events().is_empty());
    }

    #[test]
    fn test_table_finish_round_ignores_active_round() {
        let mut table = Table::new(GameSettings::default());
        table.start_round();

        table.finish_round();

        assert!(table.round.is_some());
        assert_eq!(table.round_number, 0);
        assert_eq!(table.honba, 0);
    }

    #[test]
    fn test_table_finish_round_ignores_round_over_without_result() {
        let mut table = Table::new(GameSettings::default());
        table.start_round();
        table.current_round_mut().unwrap().phase = TurnPhase::RoundOver;

        table.finish_round();

        assert!(table.round.is_none());
        assert_eq!(table.round_number, 0);
        assert_eq!(table.honba, 0);
        assert!(!table.is_game_over);
    }

    #[test]
    fn test_table_finish_round_dealer_tenpai_draw_keeps_dealer() {
        let mut table = Table::new(GameSettings::default());
        table.start_round();

        let round = table.current_round_mut().unwrap();
        round.phase = TurnPhase::RoundOver;
        round.result = Some(RoundResult::ExhaustiveDraw {
            dealer_tenpai: true,
        });

        table.finish_round();

        assert_eq!(table.honba, 1);
        assert_eq!(table.dealer, 0);
        assert_eq!(table.round_number, 0);
        assert!(!table.is_game_over);
    }

    #[test]
    fn test_table_finish_round_special_draw_keeps_round_number() {
        let mut table = Table::new(GameSettings::default());
        table.start_round();

        let round = table.current_round_mut().unwrap();
        round.phase = TurnPhase::RoundOver;
        round.result = Some(RoundResult::SpecialDraw);

        table.finish_round();

        assert_eq!(table.honba, 1);
        assert_eq!(table.dealer, 0);
        assert_eq!(table.round_number, 0);
        assert!(!table.is_game_over);
    }

    #[test]
    fn test_table_finish_round_dealer_tsumo_continues() {
        let mut table = Table::new(GameSettings::default());
        table.start_round();

        let round = table.current_round_mut().unwrap();
        round.phase = TurnPhase::RoundOver;
        round.result = Some(RoundResult::Tsumo {
            winner: 0,
            winning_tile: Tile::new(Tile::M1),
        });

        table.finish_round();

        assert_eq!(table.honba, 1);
        assert_eq!(table.dealer, 0);
        assert_eq!(table.round_number, 0);
    }

    #[test]
    fn test_table_finish_round_child_tsumo_advances_dealer() {
        let mut table = Table::new(GameSettings::default());
        table.honba = 2;
        table.start_round();

        let round = table.current_round_mut().unwrap();
        round.phase = TurnPhase::RoundOver;
        round.result = Some(RoundResult::Tsumo {
            winner: 1,
            winning_tile: Tile::new(Tile::M1),
        });

        table.finish_round();

        assert_eq!(table.honba, 0);
        assert_eq!(table.dealer, 1);
        assert_eq!(table.round_number, 1);
    }

    #[test]
    fn test_table_finish_round_ron_with_dealer_winner_continues() {
        let mut table = Table::new(GameSettings::default());
        table.start_round();

        let round = table.current_round_mut().unwrap();
        round.phase = TurnPhase::RoundOver;
        round.result = Some(RoundResult::Ron {
            winners: vec![2, 0],
            loser: 1,
            winning_tile: Tile::new(Tile::M1),
        });

        table.finish_round();

        assert_eq!(table.honba, 1);
        assert_eq!(table.dealer, 0);
        assert_eq!(table.round_number, 0);
    }

    #[test]
    fn test_table_finish_round_ron_without_dealer_winner_advances() {
        let mut table = Table::new(GameSettings::default());
        table.honba = 2;
        table.start_round();

        let round = table.current_round_mut().unwrap();
        round.phase = TurnPhase::RoundOver;
        round.result = Some(RoundResult::Ron {
            winners: vec![1, 2],
            loser: 3,
            winning_tile: Tile::new(Tile::M1),
        });

        table.finish_round();

        assert_eq!(table.honba, 0);
        assert_eq!(table.dealer, 1);
        assert_eq!(table.round_number, 1);
    }

    #[test]
    fn test_table_advance_round_updates_prevailing_wind_in_south_game() {
        let mut table = Table::new(GameSettings {
            initial_score: 25000,
            round_count: 2,
            ..Default::default()
        });

        for _ in 0..4 {
            table.start_round();
            let round = table.current_round_mut().unwrap();
            round.phase = TurnPhase::RoundOver;
            round.result = Some(RoundResult::ExhaustiveDraw {
                dealer_tenpai: false,
            });
            table.finish_round();
        }

        assert!(!table.is_game_over);
        assert_eq!(table.round_number, 4);
        assert_eq!(table.round_wind, Wind::South);
        assert_eq!(table.dealer, 0);
    }

    #[test]
    fn test_table_handle_actions_reject_wrong_phase_or_player() {
        let mut table = Table::new(GameSettings::default());
        table.start_round();

        assert!(!table.handle_action(0, ClientAction::Discard { tile: None }));
        assert!(!table.handle_action(0, ClientAction::Tsumo));
        assert!(!table.handle_action(0, ClientAction::Riichi { tile: None }));
        assert!(!table.handle_action(0, ClientAction::Ron));
        assert!(!table.handle_action(
            0,
            ClientAction::Pon {
                tiles: [Tile::new(Tile::M1), Tile::new(Tile::M1)],
            },
        ));
        assert!(!table.handle_action(
            0,
            ClientAction::Chi {
                tiles: [Tile::new(Tile::M1), Tile::new(Tile::M2)],
            },
        ));
        assert!(!table.handle_action(0, ClientAction::Pass));
        assert!(!table.handle_action(
            0,
            ClientAction::Kan {
                tile_index: Tile::M1 as usize,
            },
        ));
        assert!(!table.handle_action(0, ClientAction::NineTerminals { declare: true }));

        let round = table.current_round_mut().unwrap();
        round.do_draw();

        assert!(!table.handle_action(1, ClientAction::Tsumo));
        assert!(!table.handle_action(1, ClientAction::Riichi { tile: None }));
        assert!(!table.handle_action(
            1,
            ClientAction::Kan {
                tile_index: Tile::M1 as usize,
            },
        ));
        assert!(!table.handle_action(
            0,
            ClientAction::Kan {
                tile_index: Tile::LEN,
            },
        ));
    }

    #[test]
    fn test_table_handle_nine_terminals_continue_and_declare() {
        let mut table = Table::new(GameSettings::default());
        table.start_round();
        table.current_round_mut().unwrap().phase = TurnPhase::WaitForNineTerminals;

        assert!(!table.handle_action(1, ClientAction::NineTerminals { declare: false }));
        assert!(table.handle_action(0, ClientAction::NineTerminals { declare: false }));
        assert_eq!(
            table.current_round().unwrap().phase,
            TurnPhase::WaitForDiscard
        );

        table.current_round_mut().unwrap().phase = TurnPhase::WaitForNineTerminals;
        assert!(table.handle_action(0, ClientAction::NineTerminals { declare: true }));
        assert_eq!(table.current_round().unwrap().phase, TurnPhase::RoundOver);
        assert!(matches!(
            table.current_round().unwrap().result,
            Some(RoundResult::SpecialDraw)
        ));
    }
}
