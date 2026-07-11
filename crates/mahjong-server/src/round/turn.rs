//! ターン進行（ツモ・打牌・カン・北抜き）

use mahjong_core::tile::{Tile, TileType};

use crate::protocol::{CallType, DrawReason, ServerEvent};

use super::{Round, TurnPhase};

impl Round {
    /// ツモフェーズを実行する
    /// 山から1枚引いて現在のプレイヤーに配る
    pub fn do_draw(&mut self) -> bool {
        if self.phase != TurnPhase::Draw {
            return false;
        }

        // 同巡フリテンを解除（自分のツモ番が来たので）
        self.players[self.current_player].is_temporary_furiten = false;

        // 牌山が空なら流局
        if self.wall.is_empty() {
            self.do_exhaustive_draw();
            return true;
        }

        let Some(tile) = self.wall.draw() else {
            self.do_exhaustive_draw();
            return true;
        };
        self.players[self.current_player].draw(tile);
        self.last_draw_was_dead_wall = false;
        self.phase = TurnPhase::WaitForDiscard;

        self.push_draw_events(self.current_player, tile, "draw");

        // 九種九牌チェック: 初回ツモかつ条件を満たす場合に選択を促す
        if self.settings.nine_terminals_draw && self.check_nine_terminals() {
            self.phase = TurnPhase::WaitForNineTerminals;
            self.events
                .push((self.current_player, ServerEvent::NineTerminalsAvailable));
        }

        true
    }

    /// 打牌を実行する
    ///
    /// - `tile`: 捨てる牌（Noneならツモ切り）
    ///
    /// 打牌後、他プレイヤーの鳴き候補をチェックし、
    /// 鳴き候補があれば WaitForCalls フェーズに移行する。
    pub fn do_discard(&mut self, tile: Option<Tile>) -> bool {
        if self.phase != TurnPhase::WaitForDiscard {
            return false;
        }

        // 手出しなら打牌前のソート済み手牌内での位置を控える（他家の手牌演出用）
        let hand_index = self.discard_hand_index(self.current_player, tile);
        let Some(discarded) = self.players[self.current_player].try_discard(tile) else {
            return false;
        };

        // 一発フラグは try_discard() 内で解除済み。
        // リーチ宣言牌の打牌は do_riichi() が別途処理し、そこでフラグを復元する。
        self.announce_discard_and_check_calls(
            discarded,
            self.current_player,
            tile.is_none(),
            hand_index,
        );

        true
    }

    /// 暗カン/加カンを実行する
    pub fn do_kan(&mut self, tile_type: TileType) -> bool {
        if self.phase != TurnPhase::WaitForDiscard {
            return false;
        }

        let player_idx = self.current_player;
        if self.players[player_idx].is_riichi {
            return false;
        }

        // 場全体で4回カン済みなら追加のカン不可
        if self.total_kan_count() >= 4 {
            return false;
        }

        if self.players[player_idx]
            .ankan_options()
            .contains(&tile_type)
        {
            self.players[player_idx].do_ankan(tile_type);
        } else if self.players[player_idx]
            .kakan_options()
            .contains(&tile_type)
        {
            self.check_kakan_ron_and_resolve(player_idx, tile_type);
            return true;
        } else {
            return false;
        }
        // ankan 確定時のみこの行以降が実行される（kakan/不可の場合は early return 済み）
        self.invalidate_first_turn_flags();

        let caller_wind = self.players[player_idx].seat_wind;
        let open = self.players[player_idx].hand.melds().last().unwrap();
        let tiles = open.expanded_tiles();
        let called_tile = Tile::new(tile_type);

        for i in 0..self.player_count {
            self.events.push((
                i,
                ServerEvent::PlayerCalled {
                    player: caller_wind,
                    call_type: CallType::Ankan,
                    called_tile,
                    tiles: tiles.clone(),
                },
            ));
        }

        self.events.push((
            player_idx,
            ServerEvent::HandUpdated {
                hand: self.players[player_idx].hand.tiles().to_vec(),
            },
        ));

        self.reveal_new_dora_indicator();
        self.draw_after_kan(player_idx);
        true
    }

    /// 北抜きを実行する（三麻の抜きドラ）
    ///
    /// - 手番アクション（鳴きではない）のため、一発・第一巡フラグは中断しない
    /// - 新しいドラ表示牌は公開されない（カンとは異なる）
    /// - 補充は生牌山の末尾から行う（王牌は補充しない既存の簡略化に合わせる。
    ///   `remaining()`/海底の計算は自動で整合する）
    /// - 補充ツモでの和了は嶺上開花にならない
    pub fn do_pei(&mut self) -> bool {
        if !(self.settings.three_player && self.settings.nuki_dora) {
            return false;
        }
        if self.phase != TurnPhase::WaitForDiscard {
            return false;
        }
        // 補充する牌がない（生牌山が空）なら北抜き不可
        if self.wall.is_empty() {
            return false;
        }

        let player_idx = self.current_player;
        if !self.players[player_idx].do_pei() {
            return false;
        }

        // 全プレイヤーに北抜きを通知（各家の枚数は風のインデックス順）
        let declarer_wind = self.players[player_idx].seat_wind;
        let mut pei_counts = [0u8; 4];
        for p in self.players.iter().take(self.player_count) {
            pei_counts[p.seat_wind.to_index()] = p.pei_tiles.len() as u8;
        }
        for i in 0..self.player_count {
            self.events.push((
                i,
                ServerEvent::PeiDeclared {
                    player: declarer_wind,
                    pei_counts,
                },
            ));
        }

        // 手牌から抜いた場合に備えて本人へ手牌同期
        self.events.push((
            player_idx,
            ServerEvent::HandUpdated {
                hand: self.players[player_idx].hand.tiles().to_vec(),
            },
        ));

        // 生牌山の末尾から補充ツモ（is_empty チェック済みのため必ず成功する）
        let Some(tile) = self.wall.draw_replacement_from_tail() else {
            return false;
        };
        self.players[player_idx].draw(tile);
        self.last_draw_was_dead_wall = false;
        self.push_draw_events(player_idx, tile, "pei_draw");
        true
    }

    /// 手出し打牌が手牌（ツモ牌を除く・ソート済み）の何枚目かを返す
    ///
    /// `Player::try_discard` と同じ検索（完全一致の先頭位置）を打牌前に行う。
    /// ツモ切り（`tile` が None）や手牌に存在しない牌では None。
    pub(super) fn discard_hand_index(
        &self,
        player_idx: usize,
        tile: Option<Tile>,
    ) -> Option<usize> {
        let target = tile?;
        self.players[player_idx]
            .hand
            .tiles()
            .iter()
            .position(|t| *t == target)
    }

    /// 指定プレイヤーの最後の捨て牌を「鳴かれた」としてマークする
    pub(super) fn mark_last_discard_as_called(&mut self, discarder: usize) {
        if let Some(last_discard) = self.players[discarder].discards.last_mut() {
            last_discard.is_called = true;
        }
    }

    /// 鳴き・カンなどにより全プレイヤーの一発フラグと
    /// 第1巡フラグ（四風連打の判定用）を無効化する
    pub(super) fn invalidate_first_turn_flags(&mut self) {
        for player in &mut self.players {
            player.is_ippatsu = false;
            player.first_turn_interrupted = true;
        }
    }

    pub(super) fn reveal_new_dora_indicator(&mut self) {
        self.wall.add_dora_indicator();
        let dora_indicators = self.wall.dora_indicators();
        for i in 0..self.player_count {
            self.events.push((
                i,
                ServerEvent::DoraIndicatorsUpdated {
                    dora_indicators: dora_indicators.clone(),
                },
            ));
        }
    }

    pub(super) fn draw_after_kan(&mut self, player_idx: usize) {
        // 四槓散了チェック: 4回目のカン直後に判定（設定がありの場合のみ）
        if self.settings.four_kans_draw && self.check_four_kans_draw() {
            self.declare_special_draw(DrawReason::FourKans, None);
            return;
        }

        // 同巡フリテンを解除（嶺上ツモも自分のツモ番）
        self.players[player_idx].is_temporary_furiten = false;

        let Some(tile) = self.wall.draw_rinshan() else {
            self.do_exhaustive_draw();
            return;
        };

        self.current_player = player_idx;
        self.phase = TurnPhase::WaitForDiscard;
        self.last_draw_was_dead_wall = true;
        self.players[player_idx].draw(tile);

        self.push_draw_events(player_idx, tile, "kan_draw");
    }

    /// 却下した打牌・リーチ宣言の送り主に正しい手牌を送り返して再同期させる
    ///
    /// クライアントは打牌をローカルの手牌へ楽観的に適用してから送信するため、
    /// サーバが黙って却下するとクライアントの手牌が食い違ったままになり、
    /// 以降その牌の打牌が却下され続けて進行が止まって見える（#294）。
    /// `HandUpdated` で手牌を戻し、手番でツモ牌があれば `TileDrawn` も
    /// 再送して打牌をやり直させる（本人のみ。`OtherPlayerDrew` は送らない）。
    pub(crate) fn resync_hand(&mut self, player_idx: usize) {
        if player_idx >= self.player_count {
            return;
        }

        self.events.push((
            player_idx,
            ServerEvent::HandUpdated {
                hand: self.players[player_idx].hand.tiles().to_vec(),
            },
        ));

        if self.phase == TurnPhase::WaitForDiscard
            && self.current_player == player_idx
            && let Some(drawn) = self.players[player_idx].hand.drawn()
        {
            let can_tsumo = self.can_tsumo();
            let can_riichi = self.can_player_riichi(player_idx);
            let is_furiten = self.players[player_idx].is_furiten();
            self.events.push((
                player_idx,
                ServerEvent::TileDrawn {
                    tile: drawn,
                    remaining_tiles: self.wall.remaining(),
                    can_tsumo,
                    can_riichi,
                    is_furiten,
                },
            ));
        }
    }

    /// ツモ直後の通知イベントを積む
    ///
    /// 本人には牌と可能アクションを含む `TileDrawn`、
    /// 他プレイヤーには `OtherPlayerDrew` を送る。
    fn push_draw_events(&mut self, player_idx: usize, tile: Tile, diag_label: &str) {
        let remaining = self.wall.remaining();
        let can_tsumo = self.can_tsumo();
        let can_riichi = self.can_player_riichi(player_idx);
        #[cfg(debug_assertions)]
        self.log_draw_diagnostics(player_idx, diag_label, can_tsumo, can_riichi);
        #[cfg(not(debug_assertions))]
        let _ = diag_label;

        let is_furiten = self.players[player_idx].is_furiten();
        self.events.push((
            player_idx,
            ServerEvent::TileDrawn {
                tile,
                remaining_tiles: remaining,
                can_tsumo,
                can_riichi,
                is_furiten,
            },
        ));

        let current_wind = self.players[player_idx].seat_wind;
        for i in 0..self.player_count {
            if i != player_idx {
                self.events.push((
                    i,
                    ServerEvent::OtherPlayerDrew {
                        player: current_wind,
                        remaining_tiles: remaining,
                    },
                ));
            }
        }
    }
}
