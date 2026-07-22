//! Server-event handling and result-screen state updates.

use super::*;

impl GameState {
    /// Applies a server event.
    pub fn handle_event(&mut self, event: ServerEvent) {
        match event {
            ServerEvent::GameStarted {
                seat_wind,
                hand,
                scores,
                round_wind,
                dora_indicators,
                round_number,
                total_rounds,
                honba,
                riichi_sticks,
                three_player,
                nuki_dora,
            } => {
                self.player_count = if three_player { 3 } else { 4 };
                let length = if total_rounds > self.player_count {
                    mahjong_server::table::GameLength::Hanchan
                } else {
                    mahjong_server::table::GameLength::EastOnly
                };
                // GameStarted is authoritative for both local games and
                // online joiners, whose local setup screen may still hold
                // its default mode.
                self.setup_state.mode = GameMode::from_parts(three_player, length);
                self.setup_state.rules.nuki_dora = nuki_dora;
                self.nuki_dora = nuki_dora;
                self.pei_counts = [0; 4];
                self.can_pei = false;
                self.seat_wind = Some(seat_wind);
                // Recover the starting dealer's seat by rewinding the
                // current dealer (derived from our wind) by the hand
                // number; continuations do not advance it, so this always
                // matches.
                let n = self.player_count;
                let dealer_seat = (self.my_seat + n - seat_wind.to_index()) % n;
                self.initial_dealer_seat = (dealer_seat + n - round_number % n) % n;
                self.hand = hand;
                self.hand.sort();
                self.drawn = None;
                self.self_tedashi_anim = None;
                self.scores = scores;
                self.round_wind = Some(round_wind);
                self.dora_indicators = dora_indicators;
                self.uradora_indicators = Vec::new();
                self.discards = [Vec::new(), Vec::new(), Vec::new(), Vec::new()];
                self.pending_riichi_player = None;
                self.result_message = None;
                self.message_result_kind = MessageResultKind::Draw;
                self.win_results.clear();
                self.win_result_index = 0;
                self.phase = GamePhase::Playing;
                self.available_calls.clear();
                self.chi_option_selecting = false;
                self.chi_pending_options.clear();
                self.pon_option_selecting = false;
                self.pon_pending_options.clear();
                self.nine_terminals_pending = false;
                self.call_target_tile = None;
                self.call_discarder = None;
                self.can_tsumo = false;
                self.can_riichi = false;
                self.self_kan_options.clear();
                self.is_my_turn = false;
                self.is_riichi = false;
                self.riichi_auto_discard_at = None;
                self.clear_riichi_selection();
                self.forbidden_discards.clear();
                self.selected_forbidden_swap = false;
                self.melds.clear();
                self.round_number = round_number;
                self.honba = honba;
                self.riichi_sticks = riichi_sticks;
                self.is_furiten = false;
                self.selected_would_cause_furiten = false;
                self.other_players = [
                    OtherPlayerHand::new(),
                    OtherPlayerHand::new(),
                    OtherPlayerHand::new(),
                ];
                self.exhaustive_draw_reveal = None;
                self.last_discarder = None;
                self.call_banners = [None; 4];
                self.turn_player = None;
            }

            ServerEvent::TileDrawn {
                tile,
                remaining_tiles,
                can_tsumo,
                can_riichi,
                is_furiten,
            } => {
                self.drawn = Some(tile);
                self.self_tedashi_anim = None;
                // Restart the auto-tsumogiri delay for the new draw.
                self.riichi_auto_discard_at = None;
                self.remaining_tiles = remaining_tiles;
                self.is_my_turn = true;
                self.turn_player = self.seat_wind;
                self.can_tsumo = can_tsumo;
                self.can_riichi = can_riichi;
                self.is_furiten = is_furiten;
                self.selected_would_cause_furiten = false;
                self.clear_riichi_selection();
                self.available_calls.clear();
                self.call_target_tile = None;
                self.refresh_self_kan_options();
                self.refresh_can_pei();
                // A draw lifts the swap-calling restriction.
                self.forbidden_discards.clear();
                self.selected_forbidden_swap = false;
            }

            ServerEvent::NineTerminalsAvailable => {
                self.nine_terminals_pending = true;
            }

            ServerEvent::OtherPlayerDrew {
                player,
                remaining_tiles,
            } => {
                self.remaining_tiles = remaining_tiles;
                self.turn_player = Some(player);
                let relative_idx = self.relative_player_index(player);
                if relative_idx > 0 {
                    // The drawn tile hangs to the right of the hand and
                    // is not part of its count.
                    self.other_players[relative_idx - 1].has_drawn = true;
                }
            }

            ServerEvent::TileDiscarded {
                player,
                tile,
                is_tsumogiri,
                hand_index,
            } => {
                self.last_discarder = Some(player);
                // A new discard clears any call_discarder left from an
                // earlier call offer; a stale value (after passing) used to
                // misattribute the next call's source.
                self.call_discarder = None;
                let relative_idx = self.relative_player_index(player);
                let is_riichi = self.pending_riichi_player == Some(player);
                if is_riichi {
                    self.pending_riichi_player = None;
                }
                self.discards[relative_idx].push(DiscardInfo {
                    tile,
                    is_tsumogiri,
                    is_riichi,
                    is_called: false,
                });

                if relative_idx > 0 {
                    let started_at = self.clock;
                    let other = &mut self.other_players[relative_idx - 1];
                    let had_drawn = other.has_drawn;
                    other.consume_tiles(1);
                    // On a hand discard, animate closing the gap; a
                    // tsumogiri leaves the hand untouched.
                    other.tedashi_anim = if is_tsumogiri {
                        None
                    } else {
                        hand_index.map(|gap_index| TedashiAnim {
                            gap_index,
                            had_drawn,
                            started_at,
                        })
                    };
                }

                if Some(player) == self.seat_wind {
                    self.is_my_turn = false;
                    self.drawn = None;
                    self.can_tsumo = false;
                    self.can_riichi = false;
                    self.nine_terminals_pending = false;
                    self.selected_tile = None;
                    self.selected_drawn = false;
                    self.clear_riichi_selection();
                    self.self_kan_options.clear();
                    self.can_pei = false;
                    // The discard completes, lifting the restriction.
                    self.forbidden_discards.clear();
                    self.selected_forbidden_swap = false;
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
                called_tile,
                tiles,
            } => {
                self.available_calls.clear();
                self.call_target_tile = None;
                self.refresh_self_kan_options();

                // The caller takes the turn (ron ends the hand instead).
                if !matches!(call_type, CallType::Ron) {
                    self.turn_player = Some(player);
                }

                let category = Self::call_type_to_meld_type(&call_type);

                let meld_from = match call_type {
                    CallType::Ankan => MeldFrom::Myself,
                    CallType::Kakan => MeldFrom::Myself,
                    _ => {
                        if let Some(discarder) = self.call_discarder.or(self.last_discarder) {
                            self.compute_meld_direction(player, discarder)
                        } else {
                            MeldFrom::Previous
                        }
                    }
                };

                // Mark the called tile so the discard pool dims it. The
                // taken tile is the last matching tile in the discarder's
                // pool.
                if matches!(
                    call_type,
                    CallType::Pon | CallType::Chi | CallType::Daiminkan
                ) && let Some(discarder) = self.call_discarder.or(self.last_discarder)
                {
                    let discarder_idx = self.relative_player_index(discarder);
                    if let Some(discard) = self.discards[discarder_idx]
                        .iter_mut()
                        .rev()
                        .find(|d| d.tile == called_tile && !d.is_called)
                    {
                        discard.is_called = true;
                    }
                }

                self.call_discarder = None;

                let relative_idx = self.relative_player_index(player);
                if relative_idx > 0 {
                    let other_idx = relative_idx - 1;
                    let other = &mut self.other_players[other_idx];
                    match call_type {
                        CallType::Ron => {}
                        CallType::Kakan => {
                            if let Some(meld) = other.melds.iter_mut().find(|m| {
                                m.category == MeldType::Pon
                                    && m.tiles.first().map(|t| t.get())
                                        == tiles.first().map(|t| t.get())
                            }) {
                                meld.category = MeldType::Kakan;
                                meld.tiles = tiles.clone();
                                // Keep the pon's original `from`.
                            } else {
                                other.melds.push(Meld {
                                    category,
                                    tiles: tiles.clone(),
                                    from: meld_from,
                                    called_tile: Some(called_tile),
                                });
                            }
                            // A kakan moves one tile from the hand or
                            // drawn tile into the meld.
                            other.consume_tiles(1);
                        }
                        CallType::Ankan => {
                            other.melds.push(Meld {
                                category,
                                tiles: tiles.clone(),
                                from: MeldFrom::Myself,
                                called_tile: None,
                            });
                            // An ankan moves four tiles into the meld.
                            other.consume_tiles(4);
                        }
                        CallType::Pon | CallType::Chi => {
                            other.melds.push(Meld {
                                category,
                                tiles: tiles.clone(),
                                from: meld_from,
                                called_tile: Some(called_tile),
                            });
                            other.consume_tiles(2);
                        }
                        CallType::Daiminkan => {
                            other.melds.push(Meld {
                                category,
                                tiles: tiles.clone(),
                                from: meld_from,
                                called_tile: Some(called_tile),
                            });
                            other.consume_tiles(3);
                        }
                    }
                }

                if Some(player) == self.seat_wind {
                    match call_type {
                        CallType::Ron => {}
                        CallType::Pon | CallType::Chi | CallType::Daiminkan => {
                            self.melds.push(Meld {
                                category,
                                tiles: tiles.clone(),
                                from: meld_from,
                                called_tile: Some(called_tile),
                            });
                            self.is_my_turn = true;
                            self.drawn = None;
                            self.clear_riichi_selection();
                            self.self_kan_options.clear();
                            // The post-call discard must respect the
                            // swap-calling rule (called quads draw a
                            // replacement instead).
                            self.forbidden_discards = match call_type {
                                CallType::Pon | CallType::Chi => self
                                    .melds
                                    .last()
                                    .map(|meld| meld.forbidden_swap_tiles())
                                    .unwrap_or_default(),
                                _ => Vec::new(),
                            };
                        }
                        CallType::Ankan => {
                            self.melds.push(Meld {
                                category,
                                tiles: tiles.clone(),
                                from: MeldFrom::Myself,
                                called_tile: None,
                            });
                            self.is_my_turn = true;
                            self.drawn = None;
                            self.clear_riichi_selection();
                            self.self_kan_options.clear();
                        }
                        CallType::Kakan => {
                            if let Some(meld) = self.melds.iter_mut().find(|meld| {
                                meld.category == MeldType::Pon
                                    && meld.tiles.first().map(|tile| tile.get())
                                        == tiles.first().map(|tile| tile.get())
                            }) {
                                meld.category = MeldType::Kakan;
                                meld.tiles = tiles.clone();
                            } else {
                                self.melds.push(Meld {
                                    category,
                                    tiles: tiles.clone(),
                                    from: meld_from,
                                    called_tile: Some(called_tile),
                                });
                            }
                            self.is_my_turn = true;
                            self.drawn = None;
                            self.clear_riichi_selection();
                            self.self_kan_options.clear();
                        }
                    }
                }
            }

            ServerEvent::DoraIndicatorsUpdated { dora_indicators } => {
                self.dora_indicators = dora_indicators;
            }

            ServerEvent::PeiDeclared { player, pei_counts } => {
                self.pei_counts = pei_counts;
                // An opponent's pei removes one North from their hidden
                // hand; the replacement draw restores the count.
                let relative_idx = self.relative_player_index(player);
                if relative_idx > 0 {
                    self.other_players[relative_idx - 1].consume_tiles(1);
                }
            }

            ServerEvent::PlayerRiichi {
                player,
                scores,
                riichi_sticks,
            } => {
                self.scores = scores;
                self.riichi_sticks = riichi_sticks;

                self.pending_riichi_player = Some(player);

                if Some(player) == self.seat_wind {
                    self.is_riichi = true;
                    self.can_riichi = false;
                    self.clear_riichi_selection();
                }
            }

            ServerEvent::HandUpdated { hand } => {
                self.hand = hand;
                self.hand.sort();
                self.self_tedashi_anim = None;
                self.refresh_self_kan_options();
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
                rank,
                has_opened,
                uradora_indicators,
                riichi_sticks,
                honba,
                honba_points,
                player_hands,
            } => {
                self.scores = scores;
                self.riichi_sticks = 0;

                let (win_hand, win_melds) =
                    if let Some(info) = player_hands.iter().find(|p| p.wind == winner) {
                        let hand = info.hand.clone();
                        let relative_idx = self.relative_player_index(winner);
                        let melds = if relative_idx == 0 {
                            self.melds.clone()
                        } else {
                            self.other_players[relative_idx - 1].melds.clone()
                        };
                        (hand, melds)
                    } else {
                        (Vec::new(), Vec::new())
                    };

                self.update_other_player_hands_on_win(&player_hands, winner);

                let lang = self.lang;
                let tr = Translator::new(lang);
                let winner_name = self.player_display_name(winner);
                let loser_name = loser.map(|l| self.player_display_name(l));
                let win_type = if loser.is_some() {
                    tr.get(Key::Ron)
                } else {
                    tr.get(Key::Tsumo)
                };
                let loser_text = if let Some(l) = loser {
                    tr.dealt_in_by(&self.player_display_name(l))
                } else {
                    String::new()
                };

                // Resolve the structured yaku/dora/rank into the
                // display language.
                let yaku: Vec<(String, u32)> = yaku_list
                    .iter()
                    .map(|(item, y_han)| (item.name(has_opened, lang).to_string(), *y_han))
                    .collect();
                let yakuman_multiplier = yaku_list
                    .iter()
                    .filter_map(|(item, y_han)| {
                        (matches!(item, ScoreItem::Yaku(_)) && *y_han >= 13).then_some(*y_han / 13)
                    })
                    .sum();
                let rank_name = rank.name_for_result(lang, yakuman_multiplier);

                let mut yaku_text = String::new();
                for ((name, y_han), (item, _)) in yaku.iter().zip(&yaku_list) {
                    if !yaku_text.is_empty() {
                        yaku_text.push_str("  ");
                    }
                    if matches!(item, ScoreItem::Yaku(_)) && *y_han >= 13 {
                        yaku_text.push_str(name);
                    } else {
                        yaku_text.push_str(&format!("{} {}", name, tr.han(*y_han)));
                    }
                }

                let rank_display = if rank == ScoreRank::Yakuman {
                    rank_name.clone()
                } else if rank_name.is_empty() {
                    tr.han_fu(han, fu)
                } else {
                    format!("{} {}", tr.han_fu(han, fu), rank_name)
                };

                let score_summary = if yakuman_multiplier > 0 {
                    format!("{yaku_text} {rank_display}")
                } else {
                    format!("{yaku_text}\n{rank_display}")
                };

                let riichi_sticks_text = if riichi_sticks == 0 {
                    String::new()
                } else {
                    format!("\n{}", tr.deposit_line(riichi_sticks))
                };

                let msg = format!(
                    "{}{}{}\n{} → {}",
                    tr.win_headline(&winner_name, win_type),
                    loser_text,
                    riichi_sticks_text,
                    score_summary,
                    tr.points(&score_points.to_string())
                );

                self.win_results.push(WinResult {
                    win_hand,
                    win_melds,
                    win_tile: Some(winning_tile),
                    win_is_tsumo: loser.is_none(),
                    uradora_indicators,
                    result_message: msg,
                    winner_name,
                    loser_name,
                    yaku,
                    han,
                    fu,
                    score_points,
                    rank_name,
                    rank,
                    yakuman_multiplier,
                    riichi_sticks,
                    honba,
                    honba_points,
                });

                // The first RoundWon initializes the phase and display.
                if self.phase != GamePhase::RoundResult {
                    self.win_result_index = 0;
                    self.apply_current_win_result();
                    self.phase = GamePhase::RoundResult;
                    self.is_my_turn = false;
                    self.turn_player = None;
                    self.available_calls.clear();
                    self.clear_riichi_selection();
                    self.self_kan_options.clear();
                }
            }

            ServerEvent::RoundNagashiMangan {
                winners,
                scores,
                riichi_sticks,
                player_hands,
            } => {
                self.scores = scores;
                // As with an ordinary win, the deposits reported by the
                // event are the amount collected by the winner, not what
                // remains on the table.
                self.riichi_sticks = 0;

                let winner_winds: Vec<Wind> = winners.iter().map(|winner| winner.wind).collect();
                self.update_other_player_hands_on_nagashi(&player_hands, &winner_winds);

                let tr = Translator::new(self.lang);
                let mut lines: Vec<String> = winners
                    .iter()
                    .map(|winner| {
                        let name = self.player_display_name(winner.wind);
                        format!("{}  {}", name, tr.points(&winner.score_points.to_string()))
                    })
                    .collect();
                if riichi_sticks > 0 {
                    lines.push(tr.deposit_line(riichi_sticks));
                }
                self.show_message_result(MessageResultKind::NagashiMangan, lines.join("\n"));
            }

            ServerEvent::RoundDraw {
                scores,
                reason,
                tenpai,
                riichi_sticks,
                player_hands,
                declarer,
            } => {
                self.scores = scores;
                self.riichi_sticks = riichi_sticks;
                self.update_other_player_hands_on_draw(&player_hands, &tenpai, declarer);
                let tr = Translator::new(self.lang);
                let mut msg = tr.draw_headline(reason);

                if !tenpai.is_empty() {
                    let tenpai_names: Vec<String> = tenpai
                        .iter()
                        .map(|wind| self.player_display_name(*wind))
                        .collect();
                    msg.push('\n');
                    msg.push_str(&tr.tenpai_list(&tenpai_names.join(", ")));
                }
                if riichi_sticks > 0 {
                    msg.push('\n');
                    msg.push_str(&tr.deposit_line(riichi_sticks));
                }

                self.show_message_result(MessageResultKind::Draw, msg);
            }
        }
    }

    /// Enters the result panel used by draws and tile-less wins.
    fn show_message_result(&mut self, kind: MessageResultKind, message: String) {
        self.win_results.clear();
        self.win_result_index = 0;
        self.win_hand.clear();
        self.win_tile = None;
        self.win_melds.clear();
        self.uradora_indicators.clear();
        self.result_message = Some(message);
        self.message_result_kind = kind;
        self.phase = GamePhase::RoundResult;
        self.is_my_turn = false;
        self.turn_player = None;
        self.available_calls.clear();
        self.can_tsumo = false;
        self.can_riichi = false;
        self.can_pei = false;
        self.clear_riichi_selection();
        self.self_kan_options.clear();
    }

    /// The win-result page being shown; None on a draw.
    pub fn current_win_result(&self) -> Option<&WinResult> {
        self.win_results.get(self.win_result_index)
    }

    /// Copies the page at win_result_index into the display fields.
    pub(super) fn apply_current_win_result(&mut self) {
        if let Some(wr) = self.win_results.get(self.win_result_index) {
            let wr = wr.clone();
            self.win_hand = wr.win_hand;
            self.win_melds = wr.win_melds;
            self.win_tile = wr.win_tile;
            self.win_is_tsumo = wr.win_is_tsumo;
            self.uradora_indicators = wr.uradora_indicators;
            self.result_message = Some(wr.result_message);
        }
    }

    /// Advances to the next win-result page.
    ///
    /// Returns true after updating the display, or false past the final
    /// page (the caller then requests the next hand).
    pub fn advance_win_result(&mut self) -> bool {
        let next = self.win_result_index + 1;
        if next < self.win_results.len() {
            self.win_result_index = next;
            self.apply_current_win_result();
            true
        } else {
            false
        }
    }

    /// Updates opponents' hands on a win, revealing the winner's.
    pub(super) fn update_other_player_hands_on_win(
        &mut self,
        player_hands: &[PlayerHandInfo],
        winner: Wind,
    ) {
        for info in player_hands {
            let relative_idx = self.relative_player_index(info.wind);
            if relative_idx == 0 {
                continue;
            }
            let other = &mut self.other_players[relative_idx - 1];
            // Update melds, keeping any known `from` info.
            if other.melds.is_empty() {
                other.melds = info
                    .melds
                    .iter()
                    .map(|m| Meld {
                        category: Self::call_type_to_meld_type(&m.call_type),
                        tiles: m.tiles.clone(),
                        from: MeldFrom::Unknown,
                        called_tile: None,
                    })
                    .collect();
            }
            if info.wind == winner {
                other.hand = info.hand.clone();
                other.revealed = true;
            }
        }
    }

    /// Updates opponents' hands on a draw, revealing tenpai players'
    /// and the nine-terminals declarer's.
    pub(super) fn update_other_player_hands_on_draw(
        &mut self,
        player_hands: &[PlayerHandInfo],
        tenpai: &[Wind],
        declarer: Option<Wind>,
    ) {
        for info in player_hands {
            let relative_idx = self.relative_player_index(info.wind);
            if relative_idx == 0 {
                continue;
            }
            let other = &mut self.other_players[relative_idx - 1];
            // Update melds, keeping any known `from` info.
            if other.melds.is_empty() {
                other.melds = info
                    .melds
                    .iter()
                    .map(|m| Meld {
                        category: Self::call_type_to_meld_type(&m.call_type),
                        tiles: m.tiles.clone(),
                        from: MeldFrom::Unknown,
                        called_tile: None,
                    })
                    .collect();
            }
            if tenpai.contains(&info.wind) || declarer == Some(info.wind) {
                other.hand = info.hand.clone();
                other.revealed = true;
            }
        }
    }

    /// Updates melds and reveals Nagashi Mangan winners' hands.
    pub(super) fn update_other_player_hands_on_nagashi(
        &mut self,
        player_hands: &[PlayerHandInfo],
        winners: &[Wind],
    ) {
        self.update_other_player_hands_on_draw(player_hands, winners, None);
    }
}
