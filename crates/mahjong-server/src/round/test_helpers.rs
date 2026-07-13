//! Test helpers.

use super::{CallResponse, Round, TurnPhase};

impl Round {
    /// Auto-plays the hand to the end: everyone discards the drawn tile
    /// and never calls.
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

    /// Makes every CPU player pass during the WaitForCalls phase.
    ///
    /// Auto-passes each player other than `human_player` that has a pending
    /// call option; when all have passed the phase advances on its own.
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
