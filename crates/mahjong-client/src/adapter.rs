//! ローカルアダプター
//!
//! サーバとクライアントを同一プロセス内で直接接続する。
//! 将来的にWebSocket版に差し替え可能。

use mahjong_server::protocol::{ClientAction, ServerEvent};
use mahjong_server::round::TurnPhase;
use mahjong_server::table::{GameSettings, Table};

/// ローカルアダプター: サーバを内蔵し、直接通信する
pub struct LocalAdapter {
    table: Table,
    /// 人間プレイヤーのインデックス（0 = 東家/親）
    pub human_player: usize,
}

impl LocalAdapter {
    pub fn new() -> Self {
        LocalAdapter {
            table: Table::new(GameSettings::default()),
            human_player: 0,
        }
    }

    /// ゲームを開始する（最初の局を開始）
    pub fn start_game(&mut self) {
        self.table.start_round();
    }

    /// 人間プレイヤー向けのイベントを取得する
    pub fn poll_events(&mut self, player_idx: usize) -> Vec<ServerEvent> {
        let all_events = self.table.drain_events();
        all_events
            .into_iter()
            .filter(|(idx, _)| *idx == player_idx)
            .map(|(_, event)| event)
            .collect()
    }

    /// 人間プレイヤーのアクションを送信する
    pub fn send_action(&mut self, action: ClientAction) {
        self.table.handle_action(self.human_player, action);
    }

    /// ゲームを1ティック進める
    /// - 人間プレイヤーの手番ならツモフェーズのみ実行
    /// - CPUプレイヤーの手番なら自動進行
    /// - 鳴き待ちならCPUプレイヤーを自動パスさせる
    /// - リーチ中のCPUプレイヤーはツモ和了判定後にツモ切り
    pub fn tick(&mut self) {
        let human = self.human_player;
        let round = match self.table.current_round_mut() {
            Some(r) => r,
            None => return,
        };

        if round.is_over() {
            return;
        }

        match round.phase {
            TurnPhase::Draw => {
                if round.current_player == human {
                    // 人間プレイヤーのツモ
                    round.do_draw();
                } else {
                    // CPUプレイヤー: ツモ和了チェック → ツモ切り
                    round.do_draw();
                    if !round.is_over() && round.phase == TurnPhase::WaitForDiscard {
                        // ツモ和了チェック
                        if round.can_tsumo() {
                            round.do_tsumo();
                        } else {
                            // ツモ切り
                            round.do_discard(None);
                        }
                    }
                }
            }
            TurnPhase::WaitForDiscard => {
                // 人間プレイヤーの打牌待ち → 何もしない（入力で処理）
                // CPUプレイヤーなら既に上記Drawで処理済み
                if round.current_player != human {
                    // リーチ中の人間プレイヤー以外が打牌待ちになることは通常ない
                    // （鳴き後の打牌待ちの場合のみ）
                    round.do_discard(None);
                }
            }
            TurnPhase::WaitForCalls => {
                // CPUプレイヤーを自動パスさせる
                // 人間プレイヤーに鳴き候補がなければ自動的に全員パスで進行
                round.auto_pass_cpu(human);
            }
            TurnPhase::RoundOver => {}
        }
    }

    /// 現在の局が終了しているか
    #[allow(dead_code)]
    pub fn is_round_over(&self) -> bool {
        match self.table.current_round() {
            Some(r) => r.is_over(),
            None => true,
        }
    }

    /// 次の局を開始する
    pub fn next_round(&mut self) {
        self.table.finish_round();
        if !self.table.is_game_over {
            self.table.start_round();
        }
    }

    /// ゲームが終了しているか
    pub fn is_game_over(&self) -> bool {
        self.table.is_game_over
    }

    /// 人間プレイヤーの手番か（打牌待ち）
    #[allow(dead_code)]
    pub fn is_human_turn(&self) -> bool {
        match self.table.current_round() {
            Some(r) => {
                r.current_player == self.human_player
                    && r.phase == TurnPhase::WaitForDiscard
            }
            None => false,
        }
    }

    /// 鳴き待ちフェーズで人間プレイヤーに鳴き候補があるか
    #[allow(dead_code)]
    pub fn is_human_call_pending(&self) -> bool {
        match self.table.current_round() {
            Some(r) => {
                if r.phase != TurnPhase::WaitForCalls {
                    return false;
                }
                if let Some(ref cs) = r.call_state {
                    !cs.responded[self.human_player]
                } else {
                    false
                }
            }
            None => false,
        }
    }
}
