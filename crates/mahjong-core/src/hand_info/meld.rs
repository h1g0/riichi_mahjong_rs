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
}
