//! 流局処理（荒牌流局・途中流局）

use mahjong_core::tile::Wind;

use crate::protocol::{DrawReason, ServerEvent};
use crate::scoring;

use super::{Round, RoundResult, TurnPhase};

impl Round {
    /// 荒牌流局を処理する（ノーテン罰符を含む）
    pub(super) fn do_exhaustive_draw(&mut self) {
        // テンパイ判定
        let mut tenpai_players = Vec::new();
        let mut noten_players = Vec::new();

        for i in 0..self.player_count {
            if scoring::is_ready(&self.players[i]) {
                tenpai_players.push(i);
            } else {
                noten_players.push(i);
            }
        }

        // ノーテン罰符の計算（テンパイ者と非テンパイ者がいる場合のみ）
        if !tenpai_players.is_empty() && !noten_players.is_empty() {
            let total_penalty = 3000i32;
            let tenpai_count = tenpai_players.len() as i32;
            let noten_count = noten_players.len() as i32;

            let gain_each = total_penalty / tenpai_count;
            let loss_each = total_penalty / noten_count;

            for &i in &tenpai_players {
                self.players[i].score += gain_each;
            }
            for &i in &noten_players {
                self.players[i].score -= loss_each;
            }
        }

        let scores = self.get_scores();
        let tenpai_winds: Vec<Wind> = tenpai_players
            .iter()
            .map(|&i| self.players[i].seat_wind)
            .collect();

        let dealer_tenpai = tenpai_players.contains(&self.dealer);
        let player_hands = self.build_player_hands();

        self.phase = TurnPhase::RoundOver;
        self.result = Some(RoundResult::ExhaustiveDraw { dealer_tenpai });

        for i in 0..self.player_count {
            self.events.push((
                i,
                ServerEvent::RoundDraw {
                    scores,
                    reason: DrawReason::Exhaustive,
                    tenpai: tenpai_winds.clone(),
                    riichi_sticks: self.riichi_sticks,
                    player_hands: player_hands.clone(),
                    declarer: None,
                },
            ));
        }
    }

    /// 特殊流局をチェックする（四風連打、四家立直）
    pub(super) fn check_special_draws(&mut self) {
        // 四風連打チェック: 全員が1枚ずつ捨てて、全て同じ風牌
        if self.settings.four_winds_draw && self.check_four_winds_draw() {
            self.declare_special_draw(DrawReason::FourWinds, None);
            return;
        }

        // 四家立直チェック: 全員がリーチ宣言済み
        if self.settings.four_riichi_draw && self.check_four_riichi_draw() {
            self.declare_special_draw(DrawReason::FourRiichi, None);
        }
    }

    /// 四風連打を判定する
    ///
    /// 条件: 各プレイヤーがちょうど1枚ずつ捨てており、
    /// 全て同じ風牌で、鳴きが発生していない
    pub(super) fn check_four_winds_draw(&self) -> bool {
        // 全プレイヤー（三麻は3人）がちょうど1枚捨てていること
        let players = &self.players[..self.player_count];
        for player in players {
            if player.discards.len() != 1 {
                return false;
            }
            // 鳴かれていたら不成立
            if player.discards[0].is_called {
                return false;
            }
        }

        // 全て同じ風牌であること
        let first_tile = players[0].discards[0].tile;
        if !first_tile.is_wind() {
            return false;
        }

        players
            .iter()
            .all(|p| p.discards[0].tile.get() == first_tile.get())
    }

    /// 四家立直を判定する
    ///
    /// 条件: 全プレイヤー（三麻は3人）がリーチ宣言済み
    pub(super) fn check_four_riichi_draw(&self) -> bool {
        self.players[..self.player_count]
            .iter()
            .all(|p| p.is_riichi)
    }

    /// 九種九牌の宣言条件を判定する
    ///
    /// 条件: 現在のプレイヤーが一度も捨牌しておらず、
    /// 手牌＋ツモ牌に9種類以上のヤオ九牌（老頭牌・字牌）がある
    pub(super) fn check_nine_terminals(&self) -> bool {
        let player = &self.players[self.current_player];
        // 初回ツモのみ（捨牌済みなら宣言不可）
        if !player.discards.is_empty() {
            return false;
        }
        let mut tile_types = std::collections::HashSet::new();
        for tile in player.hand.tiles() {
            if tile.is_1_9_honour() {
                tile_types.insert(tile.get());
            }
        }
        if let Some(tile) = player.hand.drawn()
            && tile.is_1_9_honour()
        {
            tile_types.insert(tile.get());
        }
        tile_types.len() >= 9
    }

    /// 九種九牌の宣言を処理する
    ///
    /// - `declare=true`: 流局を宣言する
    /// - `declare=false`: 続行する（通常の打牌フェーズへ移行）
    pub fn do_nine_terminals(&mut self, player_idx: usize, declare: bool) -> bool {
        if self.phase != TurnPhase::WaitForNineTerminals {
            return false;
        }
        if self.current_player != player_idx {
            return false;
        }
        if declare {
            let declarer_wind = self.players[player_idx].seat_wind;
            self.declare_special_draw(DrawReason::NineTerminals, Some(declarer_wind));
        } else {
            self.phase = TurnPhase::WaitForDiscard;

            // 続行を選んだプレイヤーに TileDrawn を再送して打牌を促す。
            // 最初の TileDrawn への応答（打牌）は WaitForNineTerminals
            // フェーズで拒否されているため、再送しないとクライアントが
            // 打牌の機会を得られず局が進行しなくなる。
            if let Some(drawn) = self.players[player_idx].hand.drawn() {
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
        true
    }

    /// 場全体のカン回数を返す
    pub(super) fn total_kan_count(&self) -> usize {
        self.players.iter().map(|p| p.kan_count()).sum()
    }

    /// 四槓散了を判定する
    ///
    /// 条件: 場全体で4回カンが成立し、かつ2人以上がカンしている
    /// （1人が4回カンした場合は四槓子の可能性があるため続行）
    pub(super) fn check_four_kans_draw(&self) -> bool {
        if self.total_kan_count() < 4 {
            return false;
        }
        let players_with_kan = self.players.iter().filter(|p| p.kan_count() > 0).count();
        players_with_kan >= 2
    }

    /// 特殊流局を宣言する
    pub(super) fn declare_special_draw(&mut self, reason: DrawReason, declarer: Option<Wind>) {
        let scores = self.get_scores();
        let player_hands = self.build_player_hands();
        self.phase = TurnPhase::RoundOver;
        self.result = Some(RoundResult::SpecialDraw);

        for i in 0..self.player_count {
            self.events.push((
                i,
                ServerEvent::RoundDraw {
                    scores,
                    reason: reason.clone(),
                    tenpai: Vec::new(),
                    riichi_sticks: self.riichi_sticks,
                    player_hands: player_hands.clone(),
                    declarer,
                },
            ));
        }
    }
}
