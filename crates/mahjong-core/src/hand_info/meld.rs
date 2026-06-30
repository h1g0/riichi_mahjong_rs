use serde::{Deserialize, Serialize};

use crate::tile::*;

/// 副露の種類
#[derive(Debug, Clone, Copy, Eq, PartialEq, Serialize, Deserialize)]
pub enum MeldType {
    /// チー
    Chi,
    /// ポン
    Pon,
    /// カン（暗カン・大明カン）
    Kan,
    /// 加カン（ポンに1枚追加）
    Kakan,
}

impl MeldType {
    /// カン系（Kan または Kakan）かどうかを返す
    pub fn is_kan(&self) -> bool {
        matches!(self, MeldType::Kan | MeldType::Kakan)
    }
}

/// 誰から副露したか
#[derive(Debug, Clone, Copy, Eq, PartialEq, Serialize, Deserialize)]
pub enum MeldFrom {
    /// 上家（チー・ポン・明カン）
    Previous,
    /// 自家（暗カンしたときのみ）
    Myself,
    /// 下家（ポン・明カン）
    Following,
    /// 対面（ポン・明カン）
    Opposite,
    /// 不明
    Unknown,
}

/// 副露状態を表す構造体
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Meld {
    /// 副露で公開された牌
    pub tiles: Vec<Tile>,
    /// 副露の種類
    pub category: MeldType,
    /// 誰から副露したか
    pub from: MeldFrom,
    /// 鳴いた牌（捨て牌から取った牌。暗カンの場合は None）
    #[serde(default)]
    pub called_tile: Option<Tile>,
}

impl Meld {
    /// カンの4枚目の牌を返す
    ///
    /// 解析用に `tiles` には3枚のみ保持するため、表示・ドラ計算用の4枚目は
    /// 鳴いた牌（あれば）か、保持中の赤ドラでない牌から補う。
    /// 赤ドラは4枚中1枚しか存在しないので、`tiles` に赤ドラが含まれる場合に
    /// 4枚目を赤ドラとして複製してはならない。
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

    /// 副露の牌を表示用に展開して返す（カンは4枚目を補完する）
    pub fn expanded_tiles(&self) -> Vec<Tile> {
        let mut tiles = self.tiles.clone();
        if self.category.is_kan() && tiles.len() == 3 {
            tiles.push(self.kan_fourth_tile());
        }
        tiles
    }

    /// 喰い替え（swap-calling）で、この副露の直後に打牌が禁止される牌種を返す。
    ///
    /// - ポン: 鳴いた牌と同種（現物喰い替え）。
    /// - チー: 鳴いた牌と同種（現物喰い替え）に加え、鳴いた牌が順子の端にある場合は
    ///   反対側の外側の牌（スジ喰い替え）。
    ///   例: 4-5 を持ち 3 をチー（順子 3-4-5） → 6 を禁止。
    ///   5-6 を持ち 7 をチー（順子 5-6-7） → 4 を禁止。
    ///   鳴いた牌が順子の中央（嵌張）の場合はスジ喰い替えは発生しない。
    /// - カン系・暗カン: 喰い替えは発生しないため空を返す。
    pub fn forbidden_swap_tiles(&self) -> Vec<TileType> {
        let Some(called) = self.called_tile else {
            return Vec::new();
        };
        let called_tt = called.get();

        match self.category {
            MeldType::Pon => vec![called_tt],
            MeldType::Chi => {
                // 現物喰い替え（鳴いた牌と同種）は常に禁止
                let mut forbidden = vec![called_tt];

                // self.tiles はソート済みの順子 [low, low+1, low+2]
                let low = self.tiles[0].get();
                let high = self.tiles[2].get();
                let suit_start = (called_tt / 9) * 9;
                let suit_end = suit_start + 9;

                if called_tt == low && high + 1 < suit_end {
                    // 鳴いた牌が下端: 上端の1つ上を禁止（例: 3 をチーして 4-5 使用 → 6）
                    forbidden.push(high + 1);
                } else if called_tt == high && low > suit_start {
                    // 鳴いた牌が上端: 下端の1つ下を禁止（例: 7 をチーして 5-6 使用 → 4）
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
        // 4-5 を持ち 3 をチー（順子 3-4-5）→ 3（現物）と 6（スジ）を禁止
        let meld = chi([Tile::M3, Tile::M4, Tile::M5], Tile::M3);
        let forbidden = meld.forbidden_swap_tiles();
        assert!(forbidden.contains(&Tile::M3));
        assert!(forbidden.contains(&Tile::M6));
        assert_eq!(forbidden.len(), 2);
    }

    #[test]
    fn chi_high_end_forbids_genbutsu_and_lower_suji() {
        // 5-6 を持ち 7 をチー（順子 5-6-7）→ 7（現物）と 4（スジ）を禁止
        let meld = chi([Tile::M5, Tile::M6, Tile::M7], Tile::M7);
        let forbidden = meld.forbidden_swap_tiles();
        assert!(forbidden.contains(&Tile::M7));
        assert!(forbidden.contains(&Tile::M4));
        assert_eq!(forbidden.len(), 2);
    }

    #[test]
    fn chi_middle_forbids_only_genbutsu() {
        // 4-6 を持ち 5 をチー（嵌張 4-5-6）→ 5（現物）のみ禁止、スジ喰い替えなし
        let meld = chi([Tile::M4, Tile::M5, Tile::M6], Tile::M5);
        assert_eq!(meld.forbidden_swap_tiles(), vec![Tile::M5]);
    }

    #[test]
    fn chi_suji_does_not_cross_suit_boundary() {
        // 8-9p を持ち 7p をチー（順子 7-8-9p）→ スジ側(10p)は存在しないので 7p のみ
        let meld = chi([Tile::P7, Tile::P8, Tile::P9], Tile::P7);
        assert_eq!(meld.forbidden_swap_tiles(), vec![Tile::P7]);

        // 1-2s を持ち 3s をチー（順子 1-2-3s）→ スジ側(0s)は存在しないので 3s のみ
        let meld = chi([Tile::S1, Tile::S2, Tile::S3], Tile::S3);
        assert_eq!(meld.forbidden_swap_tiles(), vec![Tile::S3]);
    }

    #[test]
    fn red_five_called_tile_normalizes_to_tile_type() {
        // 赤5を含むチーでも、禁止は牌種で判定する
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
