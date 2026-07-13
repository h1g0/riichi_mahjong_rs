//! Per-player state: hand, discards, score, riichi state, and so on.

use mahjong_core::hand::Hand;
use mahjong_core::hand_info::meld::{Meld, MeldFrom, MeldType};
use mahjong_core::tile::{Tile, TileType, Wind};
use serde::{Deserialize, Serialize};

use crate::scoring;

/// A player's state.
pub struct Player {
    /// Seat wind
    pub seat_wind: Wind,
    /// Hand
    pub hand: Hand,
    /// Discard pool
    pub discards: Vec<Discard>,
    /// Score
    pub score: i32,
    /// Has declared riichi
    pub is_riichi: bool,
    /// Declared riichi on the first discard (double riichi)
    pub is_double_riichi: bool,
    /// Ippatsu still possible
    pub is_ippatsu: bool,
    /// Still on the first draw, for Blessing of Heaven/Earth
    pub is_first_turn: bool,
    /// Whether a call interrupted the first go-around
    pub first_turn_interrupted: bool,
    /// Furiten from passing a ron after riichi (lasts the whole hand)
    pub is_riichi_furiten: bool,
    /// Temporary furiten from passing a ron (until the player's next draw)
    pub is_temporary_furiten: bool,
    /// Tile kinds forbidden on the next discard by the swap-calling rule;
    /// set right after a chii/pon and cleared by discarding or drawing
    forbidden_discards: Vec<TileType>,
    /// North tiles extracted as pei dora (three-player only; +1 han each)
    pub pei_tiles: Vec<Tile>,
}

/// One discard.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Discard {
    /// The discarded tile
    pub tile: Tile,
    /// Whether the drawn tile was discarded directly (tsumogiri)
    pub is_tsumogiri: bool,
    /// Whether this was the riichi declaration discard
    pub is_riichi_declaration: bool,
    /// Whether another player called it
    pub is_called: bool,
}

impl Player {
    pub fn new(seat_wind: Wind, tiles: Vec<Tile>, initial_score: i32) -> Self {
        let hand = Hand::new(tiles, None);
        Player {
            seat_wind,
            hand,
            discards: Vec::new(),
            score: initial_score,
            is_riichi: false,
            is_double_riichi: false,
            is_ippatsu: false,
            is_first_turn: true,
            first_turn_interrupted: false,
            is_riichi_furiten: false,
            is_temporary_furiten: false,
            forbidden_discards: Vec::new(),
            pei_tiles: Vec::new(),
        }
    }

    /// Whether the player may extract a North tile as pei dora.
    ///
    /// Under riichi the hand is locked, so only a freshly drawn North
    /// may be extracted.
    pub fn can_pei(&self) -> bool {
        if self.is_riichi {
            return self.hand.drawn().is_some_and(|t| t.get() == Tile::Z4);
        }
        self.hand.drawn().is_some_and(|t| t.get() == Tile::Z4)
            || self.hand.tiles().iter().any(|t| t.get() == Tile::Z4)
    }

    /// Extracts one North tile from the hand or the drawn tile.
    ///
    /// Returns true on success. A drawn North is extracted in preference
    /// to one from the hand.
    pub fn do_pei(&mut self) -> bool {
        if !self.can_pei() {
            return false;
        }

        if self.hand.drawn().is_some_and(|t| t.get() == Tile::Z4) {
            let tile = self.hand.drawn().unwrap();
            self.hand.set_drawn(None);
            self.pei_tiles.push(tile);
            return true;
        }

        let drawn = self.hand.drawn();
        let tiles = self.hand.tiles_mut();
        let Some(idx) = tiles.iter().position(|t| t.get() == Tile::Z4) else {
            return false;
        };
        let tile = tiles.remove(idx);
        if let Some(drawn_tile) = drawn {
            tiles.push(drawn_tile);
            tiles.sort();
        }
        self.hand.set_drawn(None);
        self.pei_tiles.push(tile);
        true
    }

    pub fn draw(&mut self, tile: Tile) {
        self.hand.set_drawn(Some(tile));
        // The swap-calling restriction only covers the discard right after
        // the call, so a draw clears it.
        self.forbidden_discards.clear();
    }

    /// Sets the tile kinds forbidden on the next discard (swap-calling rule).
    pub fn set_forbidden_discards(&mut self, tile_types: Vec<TileType>) {
        self.forbidden_discards = tile_types;
    }

    /// Whether the swap-calling rule forbids discarding this tile now.
    pub fn is_swap_call_forbidden(&self, tile: Tile) -> bool {
        self.forbidden_discards.contains(&tile.get())
    }

    /// Discards a tile: `Some` discards that tile from the hand,
    /// `None` discards the drawn tile.
    pub fn try_discard(&mut self, tile: Option<Tile>) -> Option<Tile> {
        if let Some(target) = tile
            && self.is_swap_call_forbidden(target)
        {
            return None;
        }

        let drawn = self.hand.drawn();

        let (discarded, is_tsumogiri) = match tile {
            Some(target) => {
                let tiles = self.hand.tiles_mut();
                let idx = tiles.iter().position(|t| *t == target)?;
                let discarded = tiles.remove(idx);

                if let Some(drawn_tile) = drawn {
                    tiles.push(drawn_tile);
                    tiles.sort();
                }
                self.hand.set_drawn(None);

                (discarded, false)
            }
            None => {
                let discarded = drawn?;
                self.hand.set_drawn(None);

                (discarded, true)
            }
        };

        self.discards.push(Discard {
            tile: discarded,
            is_tsumogiri,
            is_riichi_declaration: false,
            is_called: false,
        });

        self.is_ippatsu = false;
        self.is_first_turn = false;
        self.forbidden_discards.clear();

        Some(discarded)
    }

    /// Discards a tile, panicking when it is not held.
    ///
    /// Convenience API for internal use only; unvalidated client actions
    /// must go through `try_discard`.
    pub fn discard(&mut self, tile: Option<Tile>) -> Tile {
        match self.try_discard(tile) {
            Some(discarded) => discarded,
            None => panic!("捨てる牌が手牌またはツモ牌にありません"),
        }
    }

    /// Discards the drawn tile (auto-play helper).
    pub fn tsumogiri(&mut self) -> Tile {
        self.discard(None)
    }

    /// Whether the player is the dealer (East).
    pub fn is_dealer(&self) -> bool {
        self.seat_wind == Wind::East
    }

    /// Whether the hand is closed; concealed kans do not open the hand.
    pub fn is_menzen(&self) -> bool {
        self.hand.melds().iter().all(|o| o.from == MeldFrom::Myself)
    }

    pub fn declare_riichi(&mut self, is_double: bool) {
        self.is_riichi = true;
        self.is_ippatsu = true;
        if is_double {
            self.is_double_riichi = true;
        }
        self.score -= 1000;
    }

    // ----- Call availability -----

    /// Whether the player can pon this tile.
    pub fn can_pon(&self, tile: Tile) -> bool {
        let count = self
            .hand
            .tiles()
            .iter()
            .filter(|t| t.get() == tile.get())
            .count();
        count >= 2
    }

    /// Returns the hand-tile pairs usable for a pon.
    ///
    /// Usually a single option, but when the hand holds both a red five
    /// and a normal five, the with-red and without-red pairs are offered
    /// separately.
    pub fn pon_options(&self, tile: Tile) -> Vec<[Tile; 2]> {
        let tiles_of_type: Vec<Tile> = self
            .hand
            .tiles()
            .iter()
            .filter(|t| t.get() == tile.get())
            .cloned()
            .collect();

        if tiles_of_type.len() < 2 {
            return vec![];
        }

        // Deduplicate by whether the pair contains the red five.
        let mut has_with_red = false;
        let mut has_without_red = false;
        let mut options = vec![];

        for i in 0..tiles_of_type.len() {
            for j in (i + 1)..tiles_of_type.len() {
                let includes_red = tiles_of_type[i].is_red_dora() || tiles_of_type[j].is_red_dora();
                if includes_red && !has_with_red {
                    has_with_red = true;
                    options.push([tiles_of_type[i], tiles_of_type[j]]);
                } else if !includes_red && !has_without_red {
                    has_without_red = true;
                    options.push([tiles_of_type[i], tiles_of_type[j]]);
                }
            }
        }

        options
    }

    /// Returns the hand-tile pairs usable for a chii.
    ///
    /// Honours cannot be called with chii.
    pub fn chi_options(&self, tile: Tile) -> Vec<[Tile; 2]> {
        if tile.is_honour() {
            return vec![];
        }

        let tt = tile.get();
        let tiles = self.hand.tiles();
        let mut options = vec![];

        let suit_start = (tt / 9) * 9;
        let suit_end = suit_start + 9;

        // For a pattern (a, b), enumerate the actual tile instances so a
        // red five and a normal five yield distinct options.
        let mut add_pattern = |a: TileType, b: TileType| {
            let tiles_a: Vec<Tile> = tiles.iter().filter(|t| t.get() == a).cloned().collect();
            let tiles_b: Vec<Tile> = tiles.iter().filter(|t| t.get() == b).cloned().collect();
            if tiles_a.is_empty() || tiles_b.is_empty() {
                return;
            }
            // Deduplicate on the (is-red, is-red) pair.
            let mut seen = std::collections::HashSet::new();
            for ta in &tiles_a {
                for tb in &tiles_b {
                    let key = (ta.is_red_dora(), tb.is_red_dora());
                    if seen.insert(key) {
                        options.push([*ta, *tb]);
                    }
                }
            }
        };

        // [tt-2, tt-1] + tt, e.g. calling 3m while holding 1m2m.
        if tt >= suit_start + 2 {
            add_pattern(tt - 2, tt - 1);
        }

        // [tt-1, tt+1] + tt, e.g. calling 5m while holding 4m6m.
        if tt > suit_start && tt + 1 < suit_end {
            add_pattern(tt - 1, tt + 1);
        }

        // [tt+1, tt+2] + tt, e.g. calling 1m while holding 2m3m.
        if tt + 2 < suit_end {
            add_pattern(tt + 1, tt + 2);
        }

        options
    }

    /// Whether the player can call a quad on this tile.
    pub fn can_daiminkan(&self, tile: Tile) -> bool {
        let count = self
            .hand
            .tiles()
            .iter()
            .filter(|t| t.get() == tile.get())
            .count();
        count >= 3
    }

    /// Tile kinds the player can declare a concealed kan on.
    pub fn ankan_options(&self) -> Vec<TileType> {
        let mut counts = [0u8; Tile::LEN];
        for tile in self.hand.tiles() {
            counts[tile.get() as usize] += 1;
        }
        if let Some(drawn) = self.hand.drawn() {
            counts[drawn.get() as usize] += 1;
        }

        counts
            .iter()
            .enumerate()
            .filter_map(|(idx, &count)| (count == 4).then_some(idx as TileType))
            .collect()
    }

    /// Tile kinds the player can promote to a quad (kakan).
    pub fn kakan_options(&self) -> Vec<TileType> {
        let mut counts = [0u8; Tile::LEN];
        for tile in self.hand.tiles() {
            counts[tile.get() as usize] += 1;
        }
        if let Some(drawn) = self.hand.drawn() {
            counts[drawn.get() as usize] += 1;
        }

        self.hand
            .melds()
            .iter()
            .filter(|open| open.category == MeldType::Pon)
            .filter_map(|open| {
                let tile_type = open.tiles[0].get();
                (counts[tile_type as usize] >= 1).then_some(tile_type)
            })
            .collect()
    }

    /// The actual tile a kakan would add (red fives distinct).
    pub fn kakan_added_tile(&self, tile_type: TileType) -> Option<Tile> {
        if let Some(drawn) = self.hand.drawn()
            && drawn.get() == tile_type
        {
            return Some(drawn);
        }

        self.hand
            .tiles()
            .iter()
            .copied()
            .find(|tile| tile.get() == tile_type)
    }

    /// Whether the player is furiten (may win by tsumo only, not ron):
    /// 1. discard furiten: any waiting tile appears in their own discards
    /// 2. riichi furiten: passed a ron after riichi (lasts the whole hand)
    /// 3. temporary furiten: passed a ron (until their next draw)
    pub fn is_furiten(&self) -> bool {
        // Flag checks are O(1); the waiting-tile scan below is not.
        if self.is_riichi_furiten || self.is_temporary_furiten {
            return true;
        }
        let waiting = scoring::get_waiting_tiles(self);
        if waiting.is_empty() {
            return false;
        }
        for &wt in &waiting {
            if self.discards.iter().any(|d| d.tile.get() == wt) {
                return true;
            }
        }
        false
    }

    // ----- Call execution -----

    /// Executes a pon: removes the two hand tiles and melds them with
    /// the called tile.
    pub fn do_pon(&mut self, called_tile: Tile, hand_tiles: [Tile; 2], from: MeldFrom) {
        let mut indices: Vec<usize> = Vec::new();
        for &target in &hand_tiles {
            for (i, t) in self.hand.tiles().iter().enumerate() {
                if *t == target && !indices.contains(&i) {
                    indices.push(i);
                    break;
                }
            }
        }

        let t1 = self.hand.tiles()[indices[0]];
        let t2 = self.hand.tiles()[indices[1]];

        self.hand.remove_tiles_by_indices(&mut indices);

        self.hand.add_meld(Meld {
            tiles: vec![t1, t2, called_tile],
            category: MeldType::Pon,
            from,
            called_tile: Some(called_tile),
        });

        self.is_first_turn = false;
        self.is_ippatsu = false;
    }

    /// Executes a chii: removes the two hand tiles and melds them with
    /// the called tile.
    pub fn do_chi(&mut self, called_tile: Tile, hand_tiles: [Tile; 2]) {
        let mut indices: Vec<usize> = Vec::new();
        for &target in &hand_tiles {
            for (i, t) in self.hand.tiles().iter().enumerate() {
                if *t == target && !indices.contains(&i) {
                    indices.push(i);
                    break;
                }
            }
        }

        let t1 = self.hand.tiles()[indices[0]];
        let t2 = self.hand.tiles()[indices[1]];

        self.hand.remove_tiles_by_indices(&mut indices);

        let mut chi_tiles = [t1, t2, called_tile];
        chi_tiles.sort();

        self.hand.add_meld(Meld {
            tiles: chi_tiles.to_vec(),
            category: MeldType::Chi,
            from: MeldFrom::Previous, // Chii always comes from the left player.
            called_tile: Some(called_tile),
        });

        self.is_first_turn = false;
        self.is_ippatsu = false;
    }

    /// Executes a called quad (daiminkan).
    pub fn do_daiminkan(&mut self, called_tile: Tile, from: MeldFrom) {
        let tt = called_tile.get();
        let mut indices: Vec<usize> = Vec::new();
        for (i, t) in self.hand.tiles().iter().enumerate() {
            if t.get() == tt && indices.len() < 3 {
                indices.push(i);
            }
        }
        assert_eq!(indices.len(), 3, "大明カンに必要な3枚がありません");

        let t1 = self.hand.tiles()[indices[0]];
        let t2 = self.hand.tiles()[indices[1]];
        let t3 = self.hand.tiles()[indices[2]];

        self.hand.remove_tiles_by_indices(&mut indices);
        self.hand.add_meld(Meld {
            tiles: vec![t1, t2, t3],
            category: MeldType::Kan,
            from,
            called_tile: Some(called_tile),
        });

        self.is_first_turn = false;
        self.is_ippatsu = false;
    }

    /// Executes a concealed kan (ankan).
    pub fn do_ankan(&mut self, tile_type: TileType) {
        let mut indices: Vec<usize> = Vec::new();
        for (i, t) in self.hand.tiles().iter().enumerate() {
            if t.get() == tile_type {
                indices.push(i);
            }
        }

        let drawn = self.hand.drawn();
        let drawn_matches = drawn.map(|t| t.get() == tile_type).unwrap_or(false);
        assert_eq!(
            indices.len() + usize::from(drawn_matches),
            4,
            "暗カンに必要な4枚が揃っていません"
        );

        let mut kan_tiles: Vec<Tile> = indices.iter().map(|&idx| self.hand.tiles()[idx]).collect();

        // Remove the kan tiles first: pushing the drawn tile back and
        // sorting would shift the positions `indices` points at and
        // delete the wrong tiles.
        self.hand.remove_tiles_by_indices(&mut indices);

        if drawn_matches {
            kan_tiles.push(drawn.unwrap());
        } else if let Some(d) = drawn {
            // An unrelated drawn tile goes back into the hand so the
            // replacement draw does not overwrite it.
            self.hand.tiles_mut().push(d);
            self.hand.sort();
        }
        self.hand.set_drawn(None);

        let stored_tiles = Self::stored_kan_tiles(kan_tiles);

        self.hand.add_meld(Meld {
            tiles: stored_tiles,
            category: MeldType::Kan,
            from: MeldFrom::Myself,
            called_tile: None,
        });

        self.is_first_turn = false;
        self.is_ippatsu = false;
    }

    /// Executes a promoted quad (kakan).
    pub fn do_kakan(&mut self, tile_type: TileType) {
        let drawn_matches = self
            .hand
            .drawn()
            .map(|t| t.get() == tile_type)
            .unwrap_or(false);
        let added_tile = if drawn_matches {
            let tile = self.hand.drawn().expect("加カンに必要なツモ牌がありません");
            self.hand.set_drawn(None);
            tile
        } else {
            let idx = self
                .hand
                .tiles()
                .iter()
                .position(|t| t.get() == tile_type)
                .expect("加カンに必要な牌が手牌にありません");
            let tile = self.hand.tiles_mut().remove(idx);

            if let Some(drawn_tile) = self.hand.drawn() {
                self.hand.tiles_mut().push(drawn_tile);
                self.hand.sort();
                self.hand.set_drawn(None);
            }
            tile
        };

        let open = self
            .hand
            .melds_mut()
            .iter_mut()
            .find(|open| open.category == MeldType::Pon && open.tiles[0].get() == tile_type)
            .expect("加カン対象のポンがありません");
        open.category = MeldType::Kakan;
        open.called_tile = Some(added_tile);

        self.is_first_turn = false;
        self.is_ippatsu = false;
    }

    fn stored_kan_tiles(mut kan_tiles: Vec<Tile>) -> Vec<Tile> {
        let mut stored = Vec::with_capacity(3);
        if let Some(red_pos) = kan_tiles.iter().position(|tile| tile.is_red_dora()) {
            stored.push(kan_tiles.remove(red_pos));
        }
        stored.extend(kan_tiles.into_iter().take(3 - stored.len()));
        stored
    }

    /// Number of quads the player has declared.
    pub fn kan_count(&self) -> usize {
        self.hand
            .melds()
            .iter()
            .filter(|open| open.category.is_kan())
            .count()
    }

    /// Maps the caller/discarder seat difference to a `MeldFrom`.
    ///
    /// In three-player games there is no across player: diff 1 is the
    /// left player and diff 2 the right player.
    pub fn meld_from_relative(caller: usize, discarder: usize, player_count: usize) -> MeldFrom {
        let diff = (caller + player_count - discarder) % player_count;
        if diff == 1 {
            MeldFrom::Previous
        } else if diff == player_count - 1 {
            MeldFrom::Following
        } else if diff == 2 {
            MeldFrom::Opposite
        } else {
            unreachable!()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mahjong_core::tile::Tile;

    fn make_test_tiles() -> Vec<Tile> {
        // 1m2m3m 4p5p6p 7s8s9s 1z2z3z4z
        vec![
            Tile::new(Tile::M1),
            Tile::new(Tile::M2),
            Tile::new(Tile::M3),
            Tile::new(Tile::P4),
            Tile::new(Tile::P5),
            Tile::new(Tile::P6),
            Tile::new(Tile::S7),
            Tile::new(Tile::S8),
            Tile::new(Tile::S9),
            Tile::new(Tile::Z1),
            Tile::new(Tile::Z2),
            Tile::new(Tile::Z3),
            Tile::new(Tile::Z4),
        ]
    }

    #[test]
    fn test_player_new() {
        let player = Player::new(Wind::East, make_test_tiles(), 25000);
        assert_eq!(player.seat_wind, Wind::East);
        assert_eq!(player.score, 25000);
        assert_eq!(player.hand.tiles().len(), 13);
        assert!(player.discards.is_empty());
        assert!(!player.is_riichi);
        assert!(player.is_dealer());
    }

    #[test]
    fn test_draw_and_tsumogiri() {
        let mut player = Player::new(Wind::South, make_test_tiles(), 25000);
        let draw_tile = Tile::new(Tile::Z5);

        player.draw(draw_tile);
        assert_eq!(player.hand.drawn(), Some(draw_tile));

        let discarded = player.tsumogiri();
        assert_eq!(discarded, draw_tile);
        assert!(player.hand.drawn().is_none());
        assert_eq!(player.discards.len(), 1);
        assert!(player.discards[0].is_tsumogiri);
    }

    #[test]
    fn test_discard_from_hand() {
        let mut player = Player::new(Wind::West, make_test_tiles(), 25000);
        let draw_tile = Tile::new(Tile::Z5);

        player.draw(draw_tile);

        let discarded = player.discard(Some(Tile::new(Tile::M1)));
        assert_eq!(discarded.get(), Tile::M1);
        assert_eq!(player.discards.len(), 1);
        assert!(!player.discards[0].is_tsumogiri);

        // Still 13 tiles: the drawn tile joined the hand.
        assert_eq!(player.hand.tiles().len(), 13);
        assert!(player.hand.drawn().is_none());
    }

    #[test]
    fn test_riichi_declaration() {
        let mut player = Player::new(Wind::East, make_test_tiles(), 25000);

        player.declare_riichi(false);
        assert!(player.is_riichi);
        assert!(!player.is_double_riichi);
        assert!(player.is_ippatsu);
        assert_eq!(player.score, 24000); // minus the riichi deposit

        player.declare_riichi(true);
        assert!(player.is_double_riichi);
    }

    #[test]
    fn test_is_menzen() {
        let player = Player::new(Wind::North, make_test_tiles(), 25000);
        assert!(player.is_menzen());
    }

    #[test]
    fn test_not_dealer() {
        let player = Player::new(Wind::South, make_test_tiles(), 25000);
        assert!(!player.is_dealer());
    }

    #[test]
    fn test_can_pon() {
        let tiles = vec![
            Tile::new(Tile::M1),
            Tile::new(Tile::M1),
            Tile::new(Tile::M3),
            Tile::new(Tile::P4),
            Tile::new(Tile::P5),
            Tile::new(Tile::P6),
            Tile::new(Tile::S7),
            Tile::new(Tile::S8),
            Tile::new(Tile::S9),
            Tile::new(Tile::Z1),
            Tile::new(Tile::Z2),
            Tile::new(Tile::Z3),
            Tile::new(Tile::Z4),
        ];
        let player = Player::new(Wind::East, tiles, 25000);
        assert!(player.can_pon(Tile::new(Tile::M1)));
        assert!(!player.can_pon(Tile::new(Tile::M3)));
    }

    #[test]
    fn test_chi_options() {
        let tiles = vec![
            Tile::new(Tile::M2),
            Tile::new(Tile::M3),
            Tile::new(Tile::M5),
            Tile::new(Tile::P4),
            Tile::new(Tile::P5),
            Tile::new(Tile::P6),
            Tile::new(Tile::S7),
            Tile::new(Tile::S8),
            Tile::new(Tile::S9),
            Tile::new(Tile::Z1),
            Tile::new(Tile::Z2),
            Tile::new(Tile::Z3),
            Tile::new(Tile::Z4),
        ];
        let player = Player::new(Wind::East, tiles, 25000);

        // Calling 4m: [2m,3m] or [3m,5m].
        let options = player.chi_options(Tile::new(Tile::M4));
        assert_eq!(options.len(), 2);
        assert!(
            options
                .iter()
                .any(|o| o[0].get() == Tile::M2 && o[1].get() == Tile::M3)
        );
        assert!(
            options
                .iter()
                .any(|o| o[0].get() == Tile::M3 && o[1].get() == Tile::M5)
        );

        let options = player.chi_options(Tile::new(Tile::Z1));
        assert!(options.is_empty());

        // Calling 1m: only [2m,3m].
        let options = player.chi_options(Tile::new(Tile::M1));
        assert_eq!(options.len(), 1);
        assert_eq!(options[0][0].get(), Tile::M2);
        assert_eq!(options[0][1].get(), Tile::M3);
    }

    #[test]
    fn test_do_pon() {
        let tiles = vec![
            Tile::new(Tile::M1),
            Tile::new(Tile::M1),
            Tile::new(Tile::M3),
            Tile::new(Tile::P4),
            Tile::new(Tile::P5),
            Tile::new(Tile::P6),
            Tile::new(Tile::S7),
            Tile::new(Tile::S8),
            Tile::new(Tile::S9),
            Tile::new(Tile::Z1),
            Tile::new(Tile::Z2),
            Tile::new(Tile::Z3),
            Tile::new(Tile::Z4),
        ];
        let mut player = Player::new(Wind::South, tiles, 25000);
        let called = Tile::new(Tile::M1);

        player.do_pon(
            called,
            [Tile::new(Tile::M1), Tile::new(Tile::M1)],
            MeldFrom::Previous,
        );

        assert_eq!(player.hand.tiles().len(), 11);
        assert_eq!(player.hand.melds().len(), 1);
        assert_eq!(player.hand.melds()[0].category, MeldType::Pon);
        assert!(!player.is_menzen());
    }

    #[test]
    fn test_do_chi() {
        let tiles = vec![
            Tile::new(Tile::M2),
            Tile::new(Tile::M3),
            Tile::new(Tile::M5),
            Tile::new(Tile::P4),
            Tile::new(Tile::P5),
            Tile::new(Tile::P6),
            Tile::new(Tile::S7),
            Tile::new(Tile::S8),
            Tile::new(Tile::S9),
            Tile::new(Tile::Z1),
            Tile::new(Tile::Z2),
            Tile::new(Tile::Z3),
            Tile::new(Tile::Z4),
        ];
        let mut player = Player::new(Wind::South, tiles, 25000);
        let called = Tile::new(Tile::M4);

        player.do_chi(called, [Tile::new(Tile::M3), Tile::new(Tile::M5)]);

        assert_eq!(player.hand.tiles().len(), 11);
        assert_eq!(player.hand.melds().len(), 1);
        assert_eq!(player.hand.melds()[0].category, MeldType::Chi);
        assert!(!player.is_menzen());
    }

    #[test]
    fn test_can_daiminkan() {
        let tiles = vec![
            Tile::new(Tile::M1),
            Tile::new(Tile::M1),
            Tile::new(Tile::M1),
            Tile::new(Tile::M3),
            Tile::new(Tile::P4),
            Tile::new(Tile::P5),
            Tile::new(Tile::P6),
            Tile::new(Tile::S7),
            Tile::new(Tile::S8),
            Tile::new(Tile::S9),
            Tile::new(Tile::Z1),
            Tile::new(Tile::Z2),
            Tile::new(Tile::Z3),
        ];
        let player = Player::new(Wind::East, tiles, 25000);
        assert!(player.can_daiminkan(Tile::new(Tile::M1)));
        assert!(!player.can_daiminkan(Tile::new(Tile::M3)));
    }

    #[test]
    fn test_ankan_options() {
        let hand = Hand::from("111m234p567s789m1z 1m");
        let mut player = Player::new(Wind::East, hand.tiles().to_vec(), 25000);
        player.draw(hand.drawn().unwrap());

        assert_eq!(player.ankan_options(), vec![Tile::M1]);
    }

    #[test]
    fn test_do_daiminkan() {
        let hand = Hand::from("111m234p567s789m1z");
        let mut player = Player::new(Wind::South, hand.tiles().to_vec(), 25000);

        player.do_daiminkan(Tile::new(Tile::M1), MeldFrom::Previous);

        assert_eq!(player.hand.tiles().len(), 10);
        assert_eq!(player.hand.melds().len(), 1);
        assert_eq!(player.hand.melds()[0].category, MeldType::Kan);
        assert!(!player.is_menzen());
    }

    #[test]
    fn test_do_ankan() {
        let hand = Hand::from("111m234p567s789m1z 1m");
        let mut player = Player::new(Wind::South, hand.tiles().to_vec(), 25000);
        player.draw(hand.drawn().unwrap());

        player.do_ankan(Tile::M1);

        assert_eq!(player.hand.tiles().len(), 10);
        assert!(player.hand.drawn().is_none());
        assert_eq!(player.hand.melds().len(), 1);
        assert_eq!(player.hand.melds()[0].category, MeldType::Kan);
        assert!(player.is_menzen());
    }

    #[test]
    fn test_do_ankan_preserves_red_drawn_tile_in_meld() {
        let tiles = vec![
            Tile::new(Tile::M5),
            Tile::new(Tile::M5),
            Tile::new(Tile::M5),
            Tile::new(Tile::P2),
            Tile::new(Tile::P3),
            Tile::new(Tile::P4),
            Tile::new(Tile::S2),
            Tile::new(Tile::S3),
            Tile::new(Tile::S4),
            Tile::new(Tile::M2),
            Tile::new(Tile::M3),
            Tile::new(Tile::M4),
            Tile::new(Tile::Z1),
        ];
        let mut player = Player::new(Wind::South, tiles, 25000);
        player.draw(Tile::new_red(Tile::M5));

        player.do_ankan(Tile::M5);

        assert!(
            player.hand.melds()[0]
                .tiles
                .iter()
                .any(|tile| tile.is_red_dora()),
            "暗カンの赤ドラ牌が副露情報に残ること"
        );
    }

    /// Regression: a concealed kan that returns a drawn tile smaller than
    /// the kan tiles must not delete the wrong tiles via the sort-induced
    /// index shift.
    #[test]
    fn test_do_ankan_with_smaller_unrelated_drawn_tile() {
        let hand = Hand::from("234m567m234p9999s 1m");
        let mut player = Player::new(Wind::South, hand.tiles().to_vec(), 25000);
        player.draw(hand.drawn().unwrap());

        player.do_ankan(Tile::S9);

        assert_eq!(player.hand.tiles().len(), 10);
        assert!(player.hand.drawn().is_none());
        assert!(
            !player.hand.tiles().iter().any(|t| t.get() == Tile::S9),
            "9sは全てカンされて手牌に残らないこと"
        );
        assert!(
            player.hand.tiles().contains(&Tile::new(Tile::M1)),
            "ツモ牌の1mが手牌に戻ること"
        );
        assert!(
            player.hand.tiles().contains(&Tile::new(Tile::P4)),
            "カンと無関係な牌が誤って削除されないこと"
        );
        assert_eq!(player.hand.melds().len(), 1);
        assert_eq!(player.hand.melds()[0].category, MeldType::Kan);
        assert!(
            player.hand.melds()[0]
                .tiles
                .iter()
                .all(|t| t.get() == Tile::S9)
        );
    }

    #[test]
    fn test_do_kakan() {
        let mut player = Player::new(Wind::South, vec![], 25000);
        player.hand = Hand::from("234p567s789m1z 111m 1m");

        player.do_kakan(Tile::M1);

        assert_eq!(player.hand.tiles().len(), 10);
        assert!(player.hand.drawn().is_none());
        assert_eq!(player.hand.melds().len(), 1);
        assert_eq!(player.hand.melds()[0].category, MeldType::Kakan);
        assert!(!player.is_menzen());
    }

    #[test]
    fn test_do_kakan_preserves_unrelated_drawn_tile() {
        let mut player = Player::new(Wind::South, vec![], 25000);
        player.hand = Hand::from("127m234p567s1z 111m 9s");

        player.do_kakan(Tile::M1);

        assert!(player.hand.drawn().is_none());
        assert_eq!(player.hand.tiles().len(), 10);
        assert!(player.hand.tiles().contains(&Tile::new(Tile::S9)));
        assert_eq!(player.hand.melds().len(), 1);
        assert_eq!(player.hand.melds()[0].category, MeldType::Kakan);
    }

    #[test]
    fn test_do_kakan_tracks_added_red_tile() {
        let mut player = Player::new(Wind::South, vec![], 25000);
        player.hand = Hand::from("234p567s789m1z 555m");
        player.draw(Tile::new_red(Tile::M5));

        player.do_kakan(Tile::M5);

        assert_eq!(player.hand.melds()[0].category, MeldType::Kakan);
        assert_eq!(
            player.hand.melds()[0].called_tile,
            Some(Tile::new_red(Tile::M5))
        );
    }

    #[test]
    fn test_meld_from_relative() {
        assert_eq!(Player::meld_from_relative(1, 0, 4), MeldFrom::Previous);
        assert_eq!(Player::meld_from_relative(2, 0, 4), MeldFrom::Opposite);
        assert_eq!(Player::meld_from_relative(3, 0, 4), MeldFrom::Following);
    }

    #[test]
    fn test_can_pei_and_do_pei_from_hand() {
        let mut player = Player::new(Wind::East, make_test_tiles(), 35000);
        // make_test_tiles includes one North (4z).
        assert!(player.can_pei());

        player.draw(Tile::new(Tile::P1));
        assert!(player.do_pei());

        // The North leaves the hand and the drawn tile joins it,
        // keeping 13 tiles.
        assert_eq!(player.pei_tiles.len(), 1);
        assert_eq!(player.pei_tiles[0].get(), Tile::Z4);
        assert_eq!(player.hand.tiles().len(), 13);
        assert!(player.hand.drawn().is_none());
        assert!(!player.hand.tiles().iter().any(|t| t.get() == Tile::Z4));
    }

    #[test]
    fn test_do_pei_prefers_drawn_north() {
        let mut player = Player::new(Wind::East, make_test_tiles(), 35000);
        player.draw(Tile::new(Tile::Z4));
        assert!(player.do_pei());

        // The drawn North is extracted in preference, so the one in
        // the hand stays.
        assert_eq!(player.pei_tiles.len(), 1);
        assert!(player.hand.tiles().iter().any(|t| t.get() == Tile::Z4));
        assert!(player.hand.drawn().is_none());
    }

    #[test]
    fn test_can_pei_riichi_only_drawn_north() {
        let mut player = Player::new(Wind::East, make_test_tiles(), 35000);
        player.is_riichi = true;

        // Under riichi a North in the hand alone is not extractable.
        assert!(!player.can_pei());

        // A drawn North is.
        player.draw(Tile::new(Tile::Z4));
        assert!(player.can_pei());
        assert!(player.do_pei());
        assert_eq!(player.pei_tiles.len(), 1);
    }

    #[test]
    fn test_can_pei_without_north() {
        let tiles: Vec<Tile> = make_test_tiles()
            .into_iter()
            .map(|t| {
                if t.get() == Tile::Z4 {
                    Tile::new(Tile::Z5)
                } else {
                    t
                }
            })
            .collect();
        let mut player = Player::new(Wind::East, tiles, 35000);
        assert!(!player.can_pei());
        assert!(!player.do_pei());
    }

    #[test]
    fn test_meld_from_relative_three_player() {
        assert_eq!(Player::meld_from_relative(1, 0, 3), MeldFrom::Previous);
        assert_eq!(Player::meld_from_relative(2, 0, 3), MeldFrom::Following);
        assert_eq!(Player::meld_from_relative(0, 2, 3), MeldFrom::Previous);
        assert_eq!(Player::meld_from_relative(0, 1, 3), MeldFrom::Following);
    }

    #[test]
    fn test_is_furiten_riichi_furiten() {
        let mut player = Player::new(Wind::East, make_test_tiles(), 25000);
        player.is_riichi_furiten = true;
        assert!(player.is_furiten());
    }

    #[test]
    fn test_is_furiten_temporary_furiten() {
        let mut player = Player::new(Wind::East, make_test_tiles(), 25000);
        player.is_temporary_furiten = true;
        assert!(player.is_furiten());
    }

    #[test]
    fn test_is_furiten_none() {
        let player = Player::new(Wind::East, make_test_tiles(), 25000);
        assert!(!player.is_furiten());
    }

    #[test]
    fn test_forbidden_discard_is_rejected() {
        let mut player = Player::new(Wind::East, make_test_tiles(), 25000);
        player.set_forbidden_discards(vec![Tile::M1]);

        assert!(player.is_swap_call_forbidden(Tile::new(Tile::M1)));
        // The forbidden discard is rejected and the hand is untouched.
        assert!(player.try_discard(Some(Tile::new(Tile::M1))).is_none());
        assert_eq!(player.hand.tiles().len(), 13);
        assert!(player.discards.is_empty());
    }

    #[test]
    fn test_non_forbidden_discard_still_allowed_then_clears() {
        let mut player = Player::new(Wind::East, make_test_tiles(), 25000);
        player.set_forbidden_discards(vec![Tile::M1]);

        let discarded = player.try_discard(Some(Tile::new(Tile::M2)));
        assert_eq!(discarded.map(|t| t.get()), Some(Tile::M2));
        // Completing a discard lifts the restriction; 1m becomes legal.
        assert!(!player.is_swap_call_forbidden(Tile::new(Tile::M1)));
        assert_eq!(player.discard(Some(Tile::new(Tile::M1))).get(), Tile::M1);
    }

    #[test]
    fn test_draw_clears_forbidden_discards() {
        let mut player = Player::new(Wind::East, make_test_tiles(), 25000);
        player.set_forbidden_discards(vec![Tile::M1]);

        // Drawing lifts the restriction.
        player.draw(Tile::new(Tile::Z5));
        assert!(!player.is_swap_call_forbidden(Tile::new(Tile::M1)));
        assert_eq!(player.discard(Some(Tile::new(Tile::M1))).get(), Tile::M1);
    }
}
