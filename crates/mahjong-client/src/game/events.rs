//! サーバイベントの処理と結果画面の状態更新

use super::*;

impl GameState {
    /// サーバイベントを処理する
    pub fn handle_event(&mut self, event: ServerEvent) {
        match event {
            ServerEvent::GameStarted {
                seat_wind,
                hand,
                scores,
                round_wind,
                dora_indicators,
                round_number,
                total_rounds: _,
                honba,
                riichi_sticks,
                three_player,
                nuki_dora,
            } => {
                self.player_count = if three_player { 3 } else { 4 };
                self.nuki_dora = nuki_dora;
                self.pei_counts = [0; 4];
                self.can_pei = false;
                self.seat_wind = Some(seat_wind);
                // 起家の座席を逆算する: 現在の親の座席（自分の風から求まる）を
                // 局番号ぶん巻き戻す。連荘では局番号が進まないため常に一致する。
                let n = self.player_count;
                let dealer_seat = (self.my_seat + n - seat_wind.to_index()) % n;
                self.initial_dealer_seat = (dealer_seat + n - round_number % n) % n;
                self.hand = hand;
                self.hand.sort();
                self.drawn = None;
                self.scores = scores;
                self.round_wind = Some(round_wind);
                self.dora_indicators = dora_indicators;
                self.uradora_indicators = Vec::new();
                self.discards = [Vec::new(), Vec::new(), Vec::new(), Vec::new()];
                self.pending_riichi_player = None;
                self.result_message = None;
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
                self.is_riichi = false;
                self.clear_riichi_selection();
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
                self.last_discarder = None;
                self.call_banners = [None; 4];
            }

            ServerEvent::TileDrawn {
                tile,
                remaining_tiles,
                can_tsumo,
                can_riichi,
                is_furiten,
            } => {
                self.drawn = Some(tile);
                // 新しいツモに対して自動ツモ切りの待ち時間を取り直す
                self.riichi_auto_discard_at = None;
                self.remaining_tiles = remaining_tiles;
                self.is_my_turn = true;
                self.can_tsumo = can_tsumo;
                self.can_riichi = can_riichi;
                self.is_furiten = is_furiten;
                self.selected_would_cause_furiten = false;
                self.clear_riichi_selection();
                self.available_calls.clear();
                self.call_target_tile = None;
                self.refresh_self_kan_options();
                self.refresh_can_pei();
                // ツモ後は喰い替え制限が解除される
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
                let relative_idx = self.relative_player_index(player);
                if relative_idx > 0 {
                    self.other_players[relative_idx - 1].concealed_count += 1;
                }
            }

            ServerEvent::TileDiscarded {
                player,
                tile,
                is_tsumogiri,
            } => {
                self.last_discarder = Some(player);
                // 新しい打牌が出たら、過去の鳴き打診で残った call_discarder を捨てる。
                // （パスして鳴かなかった場合に古い値が残り、次の鳴き元判定を誤らせていた）
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

                // 他プレイヤーが捨てた場合、隠し手牌の枚数を更新
                if relative_idx > 0 {
                    let other_idx = relative_idx - 1;
                    self.other_players[other_idx].concealed_count = self.other_players[other_idx]
                        .concealed_count
                        .saturating_sub(1);
                }

                // 自分が捨てた場合
                if Some(player) == self.seat_wind {
                    self.is_my_turn = false;
                    self.drawn = None;
                    self.selected_tile = None;
                    self.selected_drawn = false;
                    self.clear_riichi_selection();
                    self.self_kan_options.clear();
                    self.can_pei = false;
                    // 打牌が完了したので喰い替え制限を解除する
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
                // 鳴き選択肢をクリア
                self.available_calls.clear();
                self.call_target_tile = None;
                self.refresh_self_kan_options();

                // CallType → MeldType 変換
                let category = Self::call_type_to_meld_type(&call_type);

                // 鳴き元の判定
                let meld_from = match call_type {
                    CallType::Ankan => MeldFrom::Myself,
                    CallType::Kakan => MeldFrom::Myself,
                    _ => {
                        if let Some(discarder) = self.call_discarder.or(self.last_discarder) {
                            Self::compute_meld_direction(player, discarder)
                        } else {
                            MeldFrom::Previous
                        }
                    }
                };

                // 鳴かれた牌（ポン・チー・大明槓）を河で薄く表示するためマークする。
                // 取られた牌は鳴いた側の手番直前に捨てられた、放銃元の河の最後の該当牌。
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

                // 他プレイヤーが鳴いた場合、副露情報を記録
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
                                // from はポン時のままにする
                                other.concealed_count = other.concealed_count.saturating_sub(1);
                            } else {
                                other.melds.push(Meld {
                                    category,
                                    tiles: tiles.clone(),
                                    from: meld_from,
                                    called_tile: Some(called_tile),
                                });
                                other.concealed_count = other.concealed_count.saturating_sub(1);
                            }
                        }
                        CallType::Ankan => {
                            other.melds.push(Meld {
                                category,
                                tiles: tiles.clone(),
                                from: MeldFrom::Myself,
                                called_tile: None,
                            });
                            other.concealed_count = other.concealed_count.saturating_sub(3);
                        }
                        CallType::Pon | CallType::Chi => {
                            other.melds.push(Meld {
                                category,
                                tiles: tiles.clone(),
                                from: meld_from,
                                called_tile: Some(called_tile),
                            });
                            other.concealed_count = other.concealed_count.saturating_sub(2);
                        }
                        CallType::Daiminkan => {
                            other.melds.push(Meld {
                                category,
                                tiles: tiles.clone(),
                                from: meld_from,
                                called_tile: Some(called_tile),
                            });
                            other.concealed_count = other.concealed_count.saturating_sub(3);
                        }
                    }
                }

                // 自分が鳴いた場合、副露情報を保存し打牌待ちへ
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
                            // チー・ポン直後の打牌では喰い替え牌を捨てられない。
                            // （大明槓は嶺上ツモになるため対象外）
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
                // 他家の北抜きは手牌が1枚減って見える（補充ツモで戻る）
                let relative_idx = self.relative_player_index(player);
                if relative_idx > 0 {
                    let other = &mut self.other_players[relative_idx - 1];
                    other.concealed_count = other.concealed_count.saturating_sub(1);
                }
            }

            ServerEvent::PlayerRiichi {
                player,
                scores,
                riichi_sticks,
            } => {
                self.scores = scores;
                self.riichi_sticks = riichi_sticks;

                // 次の打牌をリーチ宣言牌としてマーク
                self.pending_riichi_player = Some(player);

                // 自分がリーチした場合
                if Some(player) == self.seat_wind {
                    self.is_riichi = true;
                    self.can_riichi = false;
                    self.clear_riichi_selection();
                }
            }

            ServerEvent::HandUpdated { hand } => {
                self.hand = hand;
                self.hand.sort();
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
                player_hands,
            } => {
                self.scores = scores;
                self.riichi_sticks = 0;

                // 手牌情報を取得
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

                // 構造化された役・ドラ・等級を表示言語へ解決する。
                let yaku: Vec<(String, u32)> = yaku_list
                    .iter()
                    .map(|(item, y_han)| (item.name(has_opened, lang).to_string(), *y_han))
                    .collect();
                let rank_name = rank.name(lang).to_string();

                let mut yaku_text = String::new();
                for (name, y_han) in &yaku {
                    if !yaku_text.is_empty() {
                        yaku_text.push_str("  ");
                    }
                    yaku_text.push_str(&format!("{} {}", name, tr.han(*y_han)));
                }

                let rank_display = if rank_name.is_empty() {
                    tr.han_fu(han, fu)
                } else {
                    format!("{} {}", tr.han_fu(han, fu), rank_name)
                };

                let riichi_sticks_text = if riichi_sticks == 0 {
                    String::new()
                } else {
                    format!("\n{}", tr.deposit_line(riichi_sticks))
                };

                let msg = format!(
                    "{}{}{}\n{}\n{} → {}",
                    tr.win_headline(&winner_name, win_type),
                    loser_text,
                    riichi_sticks_text,
                    yaku_text,
                    rank_display,
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
                    riichi_sticks,
                });

                // 最初のRoundWonでフェーズ遷移・表示を初期化
                if self.phase != GamePhase::RoundResult {
                    self.win_result_index = 0;
                    self.apply_current_win_result();
                    self.phase = GamePhase::RoundResult;
                    self.is_my_turn = false;
                    self.available_calls.clear();
                    self.clear_riichi_selection();
                    self.self_kan_options.clear();
                }
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
                    let tenpai_names: Vec<String> =
                        tenpai.iter().map(|w| self.wind_to_name(*w)).collect();
                    msg.push('\n');
                    msg.push_str(&tr.tenpai_list(&tenpai_names.join(", ")));
                }
                if riichi_sticks > 0 {
                    msg.push('\n');
                    msg.push_str(&tr.deposit_line(riichi_sticks));
                }

                self.win_hand.clear();
                self.win_tile = None;
                self.win_melds.clear();
                self.uradora_indicators.clear();
                self.result_message = Some(msg);
                self.phase = GamePhase::RoundResult;
                self.is_my_turn = false;
                self.available_calls.clear();
                self.clear_riichi_selection();
                self.self_kan_options.clear();
            }
        }
    }

    /// 現在表示中の和了結果ページを返す（流局時は None）。
    pub fn current_win_result(&self) -> Option<&WinResult> {
        self.win_results.get(self.win_result_index)
    }

    /// 現在の win_result_index が指すページを GameState の表示用フィールドに反映する
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

    /// 次の和了結果ページへ進む
    ///
    /// 次のページがある場合: 表示を更新して true を返す
    /// 最後のページだった場合: false を返す（呼び出し元が next_round() を呼ぶ）
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

    /// 和了時に他プレイヤーの手牌を更新する（和了者の手牌を公開）
    pub(super) fn update_other_player_hands_on_win(
        &mut self,
        player_hands: &[PlayerHandInfo],
        winner: Wind,
    ) {
        for info in player_hands {
            let relative_idx = self.relative_player_index(info.wind);
            if relative_idx == 0 {
                continue; // 自分はスキップ
            }
            let other = &mut self.other_players[relative_idx - 1];
            // 副露を更新（既存の from 情報を保持）
            if other.melds.is_empty() {
                other.melds = info
                    .melds
                    .iter()
                    .map(|m| Meld {
                        category: Self::call_type_to_meld_type(&m.call_type),
                        tiles: m.tiles.clone(),
                        from: MeldFrom::Unknown, // フォールバック
                        called_tile: None,
                    })
                    .collect();
            }
            // 和了者の手牌を公開
            if info.wind == winner {
                other.hand = info.hand.clone();
                other.revealed = true;
            }
        }
    }

    /// 流局時に他プレイヤーの手牌を更新する（テンパイ者・九種九牌宣言者の手牌を公開）
    pub(super) fn update_other_player_hands_on_draw(
        &mut self,
        player_hands: &[PlayerHandInfo],
        tenpai: &[Wind],
        declarer: Option<Wind>,
    ) {
        for info in player_hands {
            let relative_idx = self.relative_player_index(info.wind);
            if relative_idx == 0 {
                continue; // 自分はスキップ
            }
            let other = &mut self.other_players[relative_idx - 1];
            // 副露を更新（既存の from 情報を保持）
            if other.melds.is_empty() {
                other.melds = info
                    .melds
                    .iter()
                    .map(|m| Meld {
                        category: Self::call_type_to_meld_type(&m.call_type),
                        tiles: m.tiles.clone(),
                        from: MeldFrom::Unknown, // フォールバック
                        called_tile: None,
                    })
                    .collect();
            }
            // テンパイ者または九種九牌宣言者の手牌を公開
            if tenpai.contains(&info.wind) || declarer == Some(info.wind) {
                other.hand = info.hand.clone();
                other.revealed = true;
            }
        }
    }
}
