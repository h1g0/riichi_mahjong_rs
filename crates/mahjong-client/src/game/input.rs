//! ローカル対局の入力処理（打牌・リーチ・カン・鳴き応答）

use super::*;

impl GameState {
    pub(super) fn clear_riichi_selection(&mut self) {
        self.riichi_selection_mode = false;
        self.riichi_selectable_tiles.clear();
        self.riichi_selectable_drawn = false;
        self.selected_tile = None;
        self.selected_drawn = false;
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
                // HandAnalyzer は3枚で解析するため、カンの場合は3枚に切り詰める
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

    /// 指定の牌を捨てた場合にフリテンになるかを判定する
    ///
    /// 捨てた後の手牌がテンパイで、待ち牌が自分の捨て牌に含まれていればフリテン。
    /// tile: Some(牌) = 手牌から捨てる, None = ツモ切り
    pub(super) fn would_discard_cause_furiten(&self, tile: Option<Tile>) -> bool {
        let mut hand_tiles = self.hand.clone();
        match tile {
            Some(target) => {
                let Some(idx) = hand_tiles.iter().position(|t| *t == target) else {
                    return false;
                };
                hand_tiles.remove(idx);
                if let Some(drawn) = self.drawn {
                    hand_tiles.push(drawn);
                    hand_tiles.sort();
                }
            }
            None => {
                // ツモ切り: drawnを使わない
            }
        }

        // 手牌13枚でテンパイか確認
        let hand = Hand::new_with_melds(hand_tiles, self.melds_for_analysis(), None);
        let analyzer = match HandAnalyzer::new(&hand) {
            Ok(a) => a,
            Err(_) => return false,
        };
        if !analyzer.shanten.is_ready() {
            return false;
        }

        // 待ち牌を求める
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

        // 待ち牌が自分の捨て牌に含まれていればフリテン
        let my_discards = &self.discards[0];
        for &wt in &waiting {
            if my_discards.iter().any(|d| d.tile.get() == wt) {
                return true;
            }
        }
        // 捨てようとしている牌自体も捨て牌に加わるので、それも含めて判定
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

    /// 北抜き可能かを更新する（三麻+北抜きあり時のみ）
    ///
    /// リーチ中はツモった牌が北の場合のみ可能。
    pub(super) fn refresh_can_pei(&mut self) {
        self.can_pei = false;
        if !self.is_three_player() || !self.nuki_dora {
            return;
        }
        // 生牌山が空（海底ツモ）では補充ツモができず北抜き不可。
        // サーバも却下するため、押しても無反応なボタンを出さない（#296）。
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

    /// 入力処理: オーバーレイのクリック結果と手牌クリックを処理してアクションを返す
    pub fn handle_input(
        &mut self,
        overlay_click: Option<crate::renderer::OverlayClick>,
    ) -> Option<ClientAction> {
        use crate::renderer::OverlayClick;

        if self.phase != GamePhase::Playing {
            return None;
        }

        // 未適用のサーバイベントが残っている間（宣言バナーの保留中など）は
        // 入力を受け付けない。画面は古い状態のままなので、それに基づく操作は
        // サーバの状態と食い違うため。
        if !self.pending_events.is_empty() {
            return None;
        }

        // リーチ中はツモ切り自動処理（マウス入力不要）
        if self.is_my_turn && self.is_riichi && self.drawn.is_some() && !self.can_tsumo {
            self.drawn.take();
            return Some(ClientAction::Discard { tile: None });
        }

        // オーバーレイのクリック判定（draw_game が返した結果を処理）
        if let Some(click) = overlay_click {
            if self.nine_terminals_pending {
                match click {
                    OverlayClick::NineTerminalsDeclare => {
                        self.nine_terminals_pending = false;
                        return Some(ClientAction::NineTerminals { declare: true });
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

            // 自分のターン：ツモ・リーチ・暗カン
            match click {
                OverlayClick::Action(action) => return Some(action),
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

        // オーバーレイがクリックされていない場合は手牌のクリックを処理
        if !self.is_my_turn || !is_mouse_button_pressed(MouseButton::Left) {
            return None;
        }

        // 九種九牌・チー・ポン・鳴きパネル表示中は手牌クリックを無視
        if self.nine_terminals_pending
            || self.chi_option_selecting
            || self.pon_option_selecting
            || !self.available_calls.is_empty()
        {
            return None;
        }

        if self.is_riichi {
            return None;
        }

        let (mx, my) = crate::renderer::mouse_position_design();

        // 手牌クリック（描画と同じ中央寄せ基準を使う）
        let hand_len = self.hand.len();
        let hand_start_x = crate::renderer::player_hand_start_x(hand_len);
        let hand_y = crate::renderer::HAND_Y;
        let tile_w = 48.0;
        let tile_h = 68.0;

        for i in 0..hand_len {
            let x = hand_start_x + i as f32 * tile_w;
            if mx >= x && mx <= x + tile_w && my >= hand_y && my <= hand_y + tile_h {
                if self.riichi_selection_mode && !self.riichi_selectable_tiles.contains(&i) {
                    return None;
                }

                // 喰い替え禁止牌は打牌できない。選択された見た目（少し上に表示）に
                // しつつ「喰い替えです！」警告を出し、打牌アクションは発行しない。
                if self.forbidden_discards.contains(&self.hand[i].get()) {
                    self.selected_tile = Some(i);
                    self.selected_drawn = false;
                    self.selected_would_cause_furiten = false;
                    self.selected_forbidden_swap = true;
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
                self.selected_forbidden_swap = false;
                self.selected_would_cause_furiten =
                    self.would_discard_cause_furiten(Some(self.hand[i]));
                return None;
            }
        }

        if self.drawn.is_some() {
            let drawn_x = hand_start_x + hand_len as f32 * tile_w + crate::renderer::DRAWN_GAP;
            if mx >= drawn_x && mx <= drawn_x + tile_w && my >= hand_y && my <= hand_y + tile_h {
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
                self.selected_forbidden_swap = false;
                self.selected_would_cause_furiten = self.would_discard_cause_furiten(None);
                return None;
            }
        }

        None
    }
}
