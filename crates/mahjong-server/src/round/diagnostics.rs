//! デバッグ診断ログ
//!
//! `#[cfg(debug_assertions)]` でのみ有効なログ出力機能。

use mahjong_core::hand_info::hand_analyzer::HandAnalyzer;

use crate::scoring;

use super::Round;

impl Round {
    /// デバッグ用に自分のツモ時の判定状態を出力する
    pub(super) fn log_draw_diagnostics(
        &self,
        player_idx: usize,
        source: &str,
        can_tsumo: bool,
        can_riichi: bool,
    ) {
        if player_idx != 0 {
            return;
        }

        let player = &self.players[player_idx];
        let analyzer = HandAnalyzer::new(&player.hand);
        let win_result = scoring::check_win(
            player,
            self.prevailing_wind,
            true,
            self.wall.is_empty(),
            self.last_draw_was_dead_wall,
        );
        let riichi_discards: Vec<String> = player
            .hand
            .tiles()
            .iter()
            .copied()
            .filter(|&tile| self.can_player_riichi_with_discard(player_idx, Some(tile)))
            .map(|tile| tile.to_string())
            .collect();
        let can_riichi_with_drawn = self
            .can_player_riichi_with_discard(player_idx, None)
            .then(|| String::from("tsumo"));

        match analyzer {
            Ok(analyzer) => {
                let yaku_summary = win_result
                    .score_result
                    .as_ref()
                    .map(|score| {
                        score
                            .yaku_list
                            .iter()
                            .map(|(name, han)| format!("{}:{}", name, han))
                            .collect::<Vec<_>>()
                            .join(",")
                    })
                    .unwrap_or_default();
                let drawn = player
                    .hand
                    .drawn()
                    .map(|tile| tile.to_string())
                    .unwrap_or_else(|| String::from("none"));
                let mut riichi_options = riichi_discards;
                if let Some(drawn_label) = can_riichi_with_drawn {
                    riichi_options.push(drawn_label);
                }

                eprintln!(
                    "[draw-diag] source={} hand={} drawn={} shanten={} can_tsumo={} is_win={} can_riichi={} riichi_discards=[{}] yaku=[{}] remaining={} score={}",
                    source,
                    player.hand.to_string(),
                    drawn,
                    analyzer.shanten,
                    can_tsumo,
                    win_result.is_win,
                    can_riichi,
                    riichi_options.join(","),
                    yaku_summary,
                    self.wall.remaining(),
                    player.score,
                );
            }
            Err(err) => {
                eprintln!(
                    "[draw-diag] source={} hand={} analyzer_error={} can_tsumo={} can_riichi={}",
                    source,
                    player.hand.to_string(),
                    err,
                    can_tsumo,
                    can_riichi,
                );
            }
        }
    }
}
