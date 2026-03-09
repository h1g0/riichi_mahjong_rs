use serde::{Deserialize, Serialize};

use crate::tile::*;

/// 副露の種類
#[derive(Debug, Clone, Copy, Eq, PartialEq, Serialize, Deserialize)]
pub enum OpenType {
    /// チー
    Chi,
    /// ポン
    Pon,
    /// カン
    Kan,
}

/// 誰から副露したか
#[derive(Debug, Clone, Copy, Eq, PartialEq, Serialize, Deserialize)]
pub enum OpenFrom {
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
pub struct OpenTiles {
    /// 3枚の牌が入る。カンした時も3枚（4枚目は自明）
    pub tiles: [Tile; 3],
    /// 副露の種類
    pub category: OpenType,
    /// 誰から副露したか
    pub from: OpenFrom,
}
