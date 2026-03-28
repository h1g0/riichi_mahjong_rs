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
