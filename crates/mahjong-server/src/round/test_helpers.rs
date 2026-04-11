//! テスト用ヘルパー関数

use super::{CallResponse, Round, TurnPhase};

impl Round {
    /// 局を最後まで自動進行する（全員ツモ切り・鳴きなし）
    pub fn play_to_end(&mut self) {
        while self.phase != TurnPhase::RoundOver {
            match self.phase {
                TurnPhase::Draw => {
                    self.do_draw();
                }
                TurnPhase::WaitForDiscard => {
                    self.do_discard(None);
                }
                TurnPhase::WaitForCalls => {
                    // 全員パス
                    for i in 0..4 {
                        if let Some(ref cs) = self.call_state
                            && !cs.responded[i]
                        {
                            self.respond_to_call(i, CallResponse::Pass);
                            if self.call_state.is_none() {
                                break;
                            }
                        }
                    }
                }
                TurnPhase::WaitForNineTerminals => {
                    self.do_nine_terminals(self.current_player, true);
                }
                TurnPhase::RoundOver => break,
            }
        }
    }

    /// WaitForCalls フェーズでCPUプレイヤーを全員パスさせる
    ///
    /// human_player 以外のプレイヤーで鳴き候補がある者を自動パスさせる。
    /// 全員パスしたらフェーズが自動進行する。
    pub fn auto_pass_cpu(&mut self, human_player: usize) {
        if self.phase != TurnPhase::WaitForCalls {
            return;
        }

        for i in 0..4 {
            if i == human_player {
                continue;
            }
            if let Some(ref call_state) = self.call_state
                && !call_state.responded[i]
            {
                self.respond_to_call(i, CallResponse::Pass);
                if self.call_state.is_none() {
                    return;
                }
            }
        }
    }
}
