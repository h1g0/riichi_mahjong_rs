//! サーバ・クライアント間プロトコル
//!
//! 将来的なオンライン対戦を見据えたメッセージ定義。
//! LocalAdapter ではこれらのメッセージを直接やり取りする。

use mahjong_core::tile::{Tile, Wind};
use serde::{Deserialize, Serialize};

/// サーバからクライアントへのイベント
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ServerEvent {
    /// ゲーム開始
    GameStarted {
        /// 自分の座席の風
        seat_wind: Wind,
        /// 自分の手牌
        hand: Vec<Tile>,
        /// 各プレイヤーの初期点数
        scores: [i32; 4],
        /// 場風
        prevailing_wind: Wind,
        /// ドラ表示牌
        dora_indicators: Vec<Tile>,
    },

    /// ツモ（自分がツモった）
    TileDrawn {
        /// ツモった牌
        tile: Tile,
        /// 山の残り枚数
        remaining_tiles: usize,
    },

    /// 他プレイヤーがツモった（牌は非公開）
    OtherPlayerDrew {
        /// ツモったプレイヤーの風
        player: Wind,
        /// 山の残り枚数
        remaining_tiles: usize,
    },

    /// 牌が捨てられた
    TileDiscarded {
        /// 捨てたプレイヤーの風
        player: Wind,
        /// 捨てた牌
        tile: Tile,
        /// ツモ切りか
        is_tsumogiri: bool,
    },

    /// 局終了（和了）
    RoundWon {
        /// 和了プレイヤーの風
        winner: Wind,
        /// 放銃プレイヤーの風（ツモの場合はNone）
        loser: Option<Wind>,
        /// 和了牌
        winning_tile: Tile,
        /// 点数移動後の各プレイヤーの点数
        scores: [i32; 4],
    },

    /// 局終了（流局）
    RoundDraw {
        /// 点数移動後の各プレイヤーの点数
        scores: [i32; 4],
    },
}

/// クライアントからサーバへのアクション
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ClientAction {
    /// 牌を捨てる
    Discard {
        /// 手牌のインデックス（Noneならツモ切り）
        tile_index: Option<usize>,
    },

    /// ツモ和了を宣言する
    Tsumo,

    /// ロン和了を宣言する
    Ron,

    /// リーチを宣言する
    Riichi {
        /// 捨てる牌のインデックス
        tile_index: usize,
    },

    /// チーを宣言する
    Chi {
        /// 使用する手牌のインデックス2枚
        tile_indices: [usize; 2],
    },

    /// ポンを宣言する
    Pon {
        /// 使用する手牌のインデックス2枚
        tile_indices: [usize; 2],
    },

    /// カンを宣言する（暗カン/明カン/加カン）
    Kan {
        /// カンする牌の種類
        tile_index: usize,
    },

    /// パス（鳴きやロンをしない）
    Pass,
}
