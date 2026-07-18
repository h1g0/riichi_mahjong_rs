//! Input handling for local play: discards, riichi, kan, call responses.

use super::*;

impl GameState {
    /// Applies the optimistic UI state for an action that consumes our turn.
    ///
    /// The server remains authoritative, but hiding the now-stale controls
    /// prevents a second click from submitting another action before the
    /// first response arrives over a remote connection.
    fn submit_turn_action(&mut self, action: ClientAction) -> Option<ClientAction> {
        self.is_my_turn = false;
        self.can_tsumo = false;
        self.can_riichi = false;
        self.self_kan_options.clear();
        self.can_pei = false;
        Some(action)
    }

    /// Revalidates an own-turn overlay click against the latest state.
    ///
    /// Rendering happens before network events are applied in a frame,
    /// so the click can refer to a button that a just-arrived event made
    /// stale.
    fn can_submit_turn_action(&self, action: &ClientAction) -> bool {
        match action {
            ClientAction::Tsumo => self.is_my_turn && self.can_tsumo,
            ClientAction::Kan { tile_index } => {
                self.is_my_turn
                    && self
                        .self_kan_options
                        .iter()
                        .any(|tile| tile.get() as usize == *tile_index)
            }
            ClientAction::Pei => self.is_my_turn && self.can_pei,
            _ => false,
        }
    }

    pub(super) fn clear_riichi_selection(&mut self) {
        self.riichi_selection_mode = false;
        self.riichi_selectable_tiles.clear();
        self.riichi_selectable_drawn = false;
        self.clear_tile_selection();
    }

    /// Clears the currently selected hand tile and its related warnings.
    pub(super) fn clear_tile_selection(&mut self) {
        self.selected_tile = None;
        self.selected_drawn = false;
        self.selected_forbidden_swap = false;
        self.selected_would_cause_furiten = false;
    }

    pub(super) fn can_discard_for_riichi(&self, tile: Option<Tile>) -> bool {
        if self.drawn.is_none() {
            return false;
        }

        let mut hand =
            Hand::new_with_melds(self.hand.clone(), self.melds_for_analysis(), self.drawn);
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
            Ok(analyzer) => analyzer.shanten.is_ready(),
            Err(_) => false,
        }
    }

    pub(super) fn melds_for_analysis(&self) -> Vec<Meld> {
        self.melds
            .iter()
            .map(|meld| {
                let mut m = meld.clone();
                // HandAnalyzer expects three-tile melds; truncate quads.
                if m.category.is_kan() && m.tiles.len() > 3 {
                    m.tiles.truncate(3);
                }
                m
            })
            .collect()
    }

    pub(super) fn enter_riichi_selection(&mut self) {
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

    /// Whether discarding the tile would leave us furiten: the remaining
    /// hand is tenpai and one of its waits appears in our own discards.
    /// `Some(tile)` discards from the hand, `None` the drawn tile.
    pub(super) fn would_discard_cause_furiten(&self, tile: Option<Tile>) -> bool {
        let mut hand_tiles = self.hand.clone();
        if let Some(target) = tile {
            let Some(idx) = hand_tiles.iter().position(|t| *t == target) else {
                return false;
            };
            hand_tiles.remove(idx);
            if let Some(drawn) = self.drawn {
                hand_tiles.push(drawn);
                hand_tiles.sort();
            }
        }

        let hand = Hand::new_with_melds(hand_tiles, self.melds_for_analysis(), None);
        let analyzer = match HandAnalyzer::new(&hand) {
            Ok(a) => a,
            Err(_) => return false,
        };
        if !analyzer.shanten.is_ready() {
            return false;
        }

        let mut waiting: Vec<TileType> = Vec::new();
        for tile_type in 0..Tile::LEN as u32 {
            let mut test_hand = hand.clone();
            test_hand.set_drawn(Some(Tile::new(tile_type)));
            if let Ok(a) = HandAnalyzer::new(&test_hand)
                && a.shanten.has_won()
            {
                waiting.push(tile_type);
            }
        }

        if waiting.is_empty() {
            return false;
        }

        let my_discards = &self.discards[0];
        for &wt in &waiting {
            if my_discards.iter().any(|d| d.tile.get() == wt) {
                return true;
            }
        }
        // The tile being discarded joins the pool too, so include it.
        let discard_tile_type = match tile {
            Some(t) => t.get(),
            None => match self.drawn {
                Some(d) => d.get(),
                None => return false,
            },
        };
        for &wt in &waiting {
            if wt == discard_tile_type {
                return true;
            }
        }
        false
    }

    /// Updates whether pei is possible (three-player with pei dora only).
    ///
    /// Under riichi only a drawn North can be extracted.
    pub(super) fn refresh_can_pei(&mut self) {
        self.can_pei = false;
        if !self.is_three_player() || !self.nuki_dora {
            return;
        }
        // With the live wall empty no replacement draw exists and the
        // server rejects pei; never show a dead button (#296).
        if self.remaining_tiles == 0 {
            return;
        }
        let drawn_is_north = self.drawn.is_some_and(|t| t.get() == Tile::Z4);
        if self.is_riichi {
            self.can_pei = drawn_is_north;
        } else {
            self.can_pei = drawn_is_north || self.hand.iter().any(|t| t.get() == Tile::Z4);
        }
    }

    pub(super) fn refresh_self_kan_options(&mut self) {
        self.self_kan_options.clear();
        if self.drawn.is_none() || self.is_riichi {
            return;
        }

        let mut counts = [0u8; Tile::LEN];
        for tile in &self.hand {
            counts[tile.get() as usize] += 1;
        }
        if let Some(drawn) = self.drawn {
            counts[drawn.get() as usize] += 1;
        }

        for (idx, count) in counts.iter().enumerate() {
            if *count == 4 {
                self.self_kan_options.push(Tile::new(idx as u32));
                continue;
            }

            let has_pon = self.melds.iter().any(|meld| {
                meld.category == MeldType::Pon
                    && meld.tiles.first().map(|tile| tile.get()) == Some(idx as u32)
            });
            if has_pon && *count >= 1 {
                self.self_kan_options.push(Tile::new(idx as u32));
            }
        }
    }

    pub(super) fn apply_local_discard_from_hand(&mut self, idx: usize) -> Tile {
        let discarded_tile = self.hand[idx];
        let pre_hand_len = self.hand.len();
        self.selected_tile = None;
        self.selected_drawn = false;

        // Sorting tiles together with their origins preserves the
        // physical instance each final tile should animate from. Matching
        // by tile value afterwards would be ambiguous for duplicates.
        let mut tiles_with_origins: Vec<(Tile, SelfTileOrigin)> = self
            .hand
            .iter()
            .copied()
            .enumerate()
            .filter(|(hand_idx, _)| *hand_idx != idx)
            .map(|(hand_idx, tile)| (tile, SelfTileOrigin::Hand(hand_idx)))
            .collect();
        if let Some(drawn_tile) = self.drawn.take() {
            tiles_with_origins.push((drawn_tile, SelfTileOrigin::Drawn));
        }
        tiles_with_origins.sort_by_key(|(tile, _)| *tile);

        self.hand = tiles_with_origins.iter().map(|(tile, _)| *tile).collect();
        self.self_tedashi_anim = Some(SelfTedashiAnim {
            origins: tiles_with_origins
                .into_iter()
                .map(|(_, origin)| origin)
                .collect(),
            pre_hand_len,
            started_at: self.clock,
        });
        discarded_tile
    }

    /// Handles a click on one of our concealed hand tiles.
    ///
    /// Selection is always available while playing. Only our turn, and
    /// only before riichi, permits a second click to submit a discard.
    pub(super) fn handle_hand_tile_click(&mut self, idx: usize) -> Option<ClientAction> {
        let &tile = self.hand.get(idx)?;
        let can_discard = self.is_my_turn && !self.is_riichi;

        if can_discard && self.riichi_selection_mode && !self.riichi_selectable_tiles.contains(&idx)
        {
            return None;
        }

        // Keep forbidden tiles selected so the raised tile and warning
        // explain the rejected discard. Outside our turn no discard is
        // attempted, so showing that warning would be misleading.
        if can_discard && self.forbidden_discards.contains(&tile.get()) {
            self.selected_tile = Some(idx);
            self.selected_drawn = false;
            self.selected_would_cause_furiten = false;
            self.selected_forbidden_swap = true;
            return None;
        }

        if can_discard && self.selected_tile == Some(idx) {
            let discarded_tile = self.apply_local_discard_from_hand(idx);
            if self.riichi_selection_mode {
                self.clear_riichi_selection();
                return self.submit_turn_action(ClientAction::Riichi {
                    tile: Some(discarded_tile),
                });
            }
            return self.submit_turn_action(ClientAction::Discard {
                tile: Some(discarded_tile),
            });
        }

        self.selected_tile = Some(idx);
        self.selected_drawn = false;
        self.selected_forbidden_swap = false;
        self.selected_would_cause_furiten =
            can_discard && self.would_discard_cause_furiten(Some(tile));
        None
    }

    /// Handles a click on our drawn tile, with the same informational
    /// selection behavior outside our turn as concealed hand tiles.
    pub(super) fn handle_drawn_tile_click(&mut self) -> Option<ClientAction> {
        self.drawn?;
        let can_discard = self.is_my_turn && !self.is_riichi;

        if can_discard && self.riichi_selection_mode && !self.riichi_selectable_drawn {
            return None;
        }

        if can_discard && self.selected_drawn {
            self.selected_drawn = false;
            self.drawn.take();
            self.self_tedashi_anim = None;
            if self.riichi_selection_mode {
                self.clear_riichi_selection();
                return self.submit_turn_action(ClientAction::Riichi { tile: None });
            }
            return self.submit_turn_action(ClientAction::Discard { tile: None });
        }

        self.selected_drawn = true;
        self.selected_tile = None;
        self.selected_forbidden_swap = false;
        self.selected_would_cause_furiten = can_discard && self.would_discard_cause_furiten(None);
        None
    }

    /// Input handling: turns overlay clicks and hand clicks into actions.
    pub fn handle_input(
        &mut self,
        overlay_click: Option<crate::renderer::OverlayClick>,
        now: f64,
    ) -> Option<ClientAction> {
        use crate::renderer::OverlayClick;

        if self.phase != GamePhase::Playing {
            return None;
        }
        self.clock = now;

        // Refuse input while unapplied server events remain (e.g. a
        // pending declaration banner): the screen still shows stale state,
        // so actions based on it would contradict the server.
        if !self.pending_events.is_empty() {
            return None;
        }

        // Under riichi the discard is automatic. The drawn tile is shown
        // for a moment before being discarded so the player can see it
        // (#291); while tsumo or pei is available the discard is held so
        // the player can choose.
        if self.is_my_turn
            && self.is_riichi
            && self.drawn.is_some()
            && !self.can_tsumo
            && !self.can_pei
        {
            let deadline = *self
                .riichi_auto_discard_at
                .get_or_insert(now + RIICHI_AUTO_DISCARD_SECS);
            if now < deadline {
                return None;
            }
            self.riichi_auto_discard_at = None;
            self.drawn.take();
            self.self_tedashi_anim = None;
            return self.submit_turn_action(ClientAction::Discard { tile: None });
        }

        // Overlay clicks, as reported by draw_game.
        if let Some(click) = overlay_click {
            self.clear_tile_selection();

            if self.nine_terminals_pending {
                match click {
                    OverlayClick::NineTerminalsDeclare => {
                        self.nine_terminals_pending = false;
                        return self
                            .submit_turn_action(ClientAction::NineTerminals { declare: true });
                    }
                    OverlayClick::NineTerminalsPass => {
                        self.nine_terminals_pending = false;
                        return Some(ClientAction::NineTerminals { declare: false });
                    }
                    _ => {}
                }
                return None;
            }

            if self.chi_option_selecting {
                match click {
                    OverlayClick::Action(action) => {
                        self.chi_option_selecting = false;
                        self.chi_pending_options.clear();
                        self.available_calls.clear();
                        self.call_target_tile = None;
                        return Some(action);
                    }
                    OverlayClick::CancelMeldSelection => {
                        self.chi_option_selecting = false;
                        self.chi_pending_options.clear();
                    }
                    _ => {}
                }
                return None;
            }

            if self.pon_option_selecting {
                match click {
                    OverlayClick::Action(action) => {
                        self.pon_option_selecting = false;
                        self.pon_pending_options.clear();
                        self.available_calls.clear();
                        self.call_target_tile = None;
                        return Some(action);
                    }
                    OverlayClick::CancelMeldSelection => {
                        self.pon_option_selecting = false;
                        self.pon_pending_options.clear();
                    }
                    _ => {}
                }
                return None;
            }

            if !self.available_calls.is_empty() {
                match click {
                    OverlayClick::Action(action) => {
                        self.available_calls.clear();
                        self.call_target_tile = None;
                        return Some(action);
                    }
                    OverlayClick::ShowChiSelection { options } => {
                        self.chi_pending_options = options;
                        self.chi_option_selecting = true;
                    }
                    OverlayClick::ShowPonSelection { options } => {
                        self.pon_pending_options = options;
                        self.pon_option_selecting = true;
                    }
                    _ => {}
                }
                return None;
            }

            match click {
                OverlayClick::Action(action) => {
                    if self.can_submit_turn_action(&action) {
                        return self.submit_turn_action(action);
                    }
                    return None;
                }
                OverlayClick::PassSelfCall => {
                    // Declining the pei offer under riichi falls back to
                    // the usual automatic tsumogiri.
                    if self.is_riichi && self.drawn.is_some() {
                        self.can_pei = false;
                        self.drawn.take();
                        self.self_tedashi_anim = None;
                        return self.submit_turn_action(ClientAction::Discard { tile: None });
                    }
                    return None;
                }
                OverlayClick::ToggleRiichi => {
                    if self.riichi_selection_mode {
                        self.clear_riichi_selection();
                    } else {
                        self.enter_riichi_selection();
                    }
                    return None;
                }
                _ => {}
            }
        }

        // Do not let inspection selection interfere with tsumo/pei or
        // automatic tsumogiri during our own riichi turn.
        if self.is_my_turn && self.is_riichi {
            return None;
        }

        if !is_mouse_button_pressed(MouseButton::Left) {
            return None;
        }

        // Do not let a hand click become a discard while the own-turn
        // nine terminals decision is unresolved. Call-response panels
        // occur outside our turn, so inspection remains safe there.
        if self.nine_terminals_pending {
            self.clear_tile_selection();
            return None;
        }

        let (mx, my) = crate::renderer::mouse_position_design();

        // Hand clicks use the same centering as the renderer.
        let hand_len = self.hand.len();
        let hand_start_x = crate::renderer::player_hand_start_x(hand_len);
        let hand_y = crate::renderer::HAND_Y;
        let tile_w = 48.0;
        let tile_h = 68.0;
        let selected_raise = 14.0;

        for i in 0..hand_len {
            let x = crate::renderer::player_hand_tile_x(self, i, now);
            let y = if self.selected_tile == Some(i) {
                hand_y - selected_raise
            } else {
                hand_y
            };
            if mx >= x && mx <= x + tile_w && my >= y && my <= y + tile_h {
                return self.handle_hand_tile_click(i);
            }
        }

        if self.drawn.is_some() {
            let drawn_x = hand_start_x + hand_len as f32 * tile_w + crate::renderer::DRAWN_GAP;
            let drawn_y = if self.selected_drawn {
                hand_y - selected_raise
            } else {
                hand_y
            };
            if mx >= drawn_x && mx <= drawn_x + tile_w && my >= drawn_y && my <= drawn_y + tile_h {
                return self.handle_drawn_tile_click();
            }
        }

        self.clear_tile_selection();
        None
    }
}
