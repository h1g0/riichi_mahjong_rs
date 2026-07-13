use serde::{Deserialize, Serialize};

use crate::tile::*;

/// Kind of meld call.
#[derive(Debug, Clone, Copy, Eq, PartialEq, Serialize, Deserialize)]
pub enum MeldType {
    /// Chii (チー)
    Chi,
    /// Pon (ポン)
    Pon,
    /// Kan, both concealed (ankan / 暗槓) and called (daiminkan / 大明槓)
    Kan,
    /// Promoted quad (kakan / 加槓): a tile added to a melded pon
    Kakan,
}

impl MeldType {
    /// Whether this is a quad (`Kan` or `Kakan`).
    pub fn is_kan(&self) -> bool {
        matches!(self, MeldType::Kan | MeldType::Kakan)
    }
}

/// Which player the meld was called from.
#[derive(Debug, Clone, Copy, Eq, PartialEq, Serialize, Deserialize)]
pub enum MeldFrom {
    /// Left player (kamicha / 上家): chii, pon, or called kan
    Previous,
    /// Self: concealed kan only
    Myself,
    /// Right player (shimocha / 下家): pon or called kan
    Following,
    /// Across player (toimen / 対面): pon or called kan
    Opposite,
    Unknown,
}

/// A melded (open) group.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Meld {
    /// Tiles revealed by the call
    pub tiles: Vec<Tile>,
    /// Kind of call
    pub category: MeldType,
    /// Which player the tile was called from
    pub from: MeldFrom,
    /// The called discard; `None` for a concealed kan
    #[serde(default)]
    pub called_tile: Option<Tile>,
}

impl Meld {
    /// Returns the fourth tile of a quad.
    ///
    /// `tiles` keeps only three tiles for hand analysis, so the fourth tile
    /// (needed for display and dora counting) is recovered from the called
    /// tile, or from a held tile that is not a red five. Only one red five
    /// exists per kind, so when `tiles` contains one the fourth tile must
    /// not be duplicated as another red five.
    pub fn kan_fourth_tile(&self) -> Tile {
        if let Some(tile) = self.called_tile {
            return tile;
        }

        self.tiles
            .iter()
            .copied()
            .find(|tile| !tile.is_red_dora())
            .unwrap_or(self.tiles[0])
    }

    /// Returns the meld's tiles for display, restoring the fourth tile of a quad.
    pub fn expanded_tiles(&self) -> Vec<Tile> {
        let mut tiles = self.tiles.clone();
        if self.category.is_kan() && tiles.len() == 3 {
            tiles.push(self.kan_fourth_tile());
        }
        tiles
    }

    /// Returns the tile kinds that the swap-calling (kuikae / 喰い替え) rule
    /// forbids discarding immediately after this call.
    ///
    /// - Pon: the same kind as the called tile.
    /// - Chii: the same kind as the called tile, plus — when the called tile
    ///   is at either end of the sequence — the tile just outside the other
    ///   end (suji kuikae). E.g. calling 3 to use 4-5 (3-4-5) forbids 6;
    ///   calling 7 to use 5-6 (5-6-7) forbids 4. No suji restriction applies
    ///   when the called tile is in the middle of the sequence.
    /// - Kan (any kind): returns empty because swap-calling cannot occur.
    pub fn forbidden_swap_tiles(&self) -> Vec<TileType> {
        let Some(called) = self.called_tile else {
            return Vec::new();
        };
        let called_tt = called.get();

        match self.category {
            MeldType::Pon => vec![called_tt],
            MeldType::Chi => {
                let mut forbidden = vec![called_tt];

                // self.tiles is a sorted sequence [low, low+1, low+2].
                // The suji tile is only forbidden when it stays inside the suit.
                let low = self.tiles[0].get();
                let high = self.tiles[2].get();
                let suit_start = (called_tt / 9) * 9;
                let suit_end = suit_start + 9;

                if called_tt == low && high + 1 < suit_end {
                    forbidden.push(high + 1);
                } else if called_tt == high && low > suit_start {
                    forbidden.push(low - 1);
                }

                forbidden
            }
            MeldType::Kan | MeldType::Kakan => Vec::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn chi(tiles: [TileType; 3], called: TileType) -> Meld {
        Meld {
            tiles: tiles.iter().map(|&t| Tile::new(t)).collect(),
            category: MeldType::Chi,
            from: MeldFrom::Previous,
            called_tile: Some(Tile::new(called)),
        }
    }

    #[test]
    fn pon_forbids_only_the_called_tile() {
        let meld = Meld {
            tiles: vec![Tile::new(Tile::S1); 3],
            category: MeldType::Pon,
            from: MeldFrom::Opposite,
            called_tile: Some(Tile::new(Tile::S1)),
        };
        assert_eq!(meld.forbidden_swap_tiles(), vec![Tile::S1]);
    }

    #[test]
    fn chi_low_end_forbids_genbutsu_and_upper_suji() {
        let meld = chi([Tile::M3, Tile::M4, Tile::M5], Tile::M3);
        let forbidden = meld.forbidden_swap_tiles();
        assert!(forbidden.contains(&Tile::M3));
        assert!(forbidden.contains(&Tile::M6));
        assert_eq!(forbidden.len(), 2);
    }

    #[test]
    fn chi_high_end_forbids_genbutsu_and_lower_suji() {
        let meld = chi([Tile::M5, Tile::M6, Tile::M7], Tile::M7);
        let forbidden = meld.forbidden_swap_tiles();
        assert!(forbidden.contains(&Tile::M7));
        assert!(forbidden.contains(&Tile::M4));
        assert_eq!(forbidden.len(), 2);
    }

    #[test]
    fn chi_middle_forbids_only_genbutsu() {
        let meld = chi([Tile::M4, Tile::M5, Tile::M6], Tile::M5);
        assert_eq!(meld.forbidden_swap_tiles(), vec![Tile::M5]);
    }

    #[test]
    fn chi_suji_does_not_cross_suit_boundary() {
        // The suji tile would be a nonexistent "10p" / "0s",
        // so only the called tile is forbidden.
        let meld = chi([Tile::P7, Tile::P8, Tile::P9], Tile::P7);
        assert_eq!(meld.forbidden_swap_tiles(), vec![Tile::P7]);

        let meld = chi([Tile::S1, Tile::S2, Tile::S3], Tile::S3);
        assert_eq!(meld.forbidden_swap_tiles(), vec![Tile::S3]);
    }

    #[test]
    fn red_five_called_tile_normalizes_to_tile_type() {
        // A red five in the meld must not affect the forbidden kinds,
        // which are decided by tile kind only.
        let meld = Meld {
            tiles: vec![
                Tile::new(Tile::M3),
                Tile::new(Tile::M4),
                Tile::new_red(Tile::M5),
            ],
            category: MeldType::Chi,
            from: MeldFrom::Previous,
            called_tile: Some(Tile::new(Tile::M3)),
        };
        let forbidden = meld.forbidden_swap_tiles();
        assert!(forbidden.contains(&Tile::M3));
        assert!(forbidden.contains(&Tile::M6));
    }

    #[test]
    fn kan_has_no_swap_restriction() {
        let meld = Meld {
            tiles: vec![Tile::new(Tile::M1); 3],
            category: MeldType::Kan,
            from: MeldFrom::Myself,
            called_tile: None,
        };
        assert!(meld.forbidden_swap_tiles().is_empty());
    }
}
