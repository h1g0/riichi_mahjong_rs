//! サーバ・クライアント間プロトコル
//!
//! 将来的なオンライン対戦を見据えたメッセージ定義。
//! LocalAdapter ではこれらのメッセージを直接やり取りする。

use mahjong_core::tile::{Tile, TileType, Wind};
use serde::{Deserialize, Serialize};

/// 流局の理由
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DrawReason {
    /// 荒牌流局（牌山切れ）
    Exhaustive,
    /// 四風連打
    FourWinds,
    /// 四家立直
    FourRiichi,
    /// 九種九牌
    NineTerminals,
}

/// 鳴きの種類
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum CallType {
    /// ロン
    Ron,
    /// ポン
    Pon,
    /// チー
    Chi,
    /// 大明カン
    Daiminkan,
}

/// 利用可能な鳴きアクション（CallAvailableイベント内で使用）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AvailableCall {
    /// ロン和了可能
    Ron,
    /// ポン可能
    Pon,
    /// チー可能（使える手牌の組み合わせのリスト: 各要素は [TileType; 2]）
    Chi { options: Vec<[TileType; 2]> },
}

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
        /// 局番号（0-based: 東1局=0, 東2局=1, ...）
        round_number: usize,
        /// 本場数
        honba: usize,
    },

    /// ツモ（自分がツモった）
    TileDrawn {
        /// ツモった牌
        tile: Tile,
        /// 山の残り枚数
        remaining_tiles: usize,
        /// ツモ和了可能か
        can_tsumo: bool,
        /// リーチ宣言可能か
        can_riichi: bool,
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

    /// 鳴き可能通知（自分に鳴きの選択肢がある）
    CallAvailable {
        /// 捨てられた牌
        tile: Tile,
        /// 捨てたプレイヤーの風
        discarder: Wind,
        /// 利用可能な鳴きアクション
        calls: Vec<AvailableCall>,
    },

    /// プレイヤーが鳴きを行った
    PlayerCalled {
        /// 鳴いたプレイヤーの風
        player: Wind,
        /// 鳴きの種類
        call_type: CallType,
        /// 鳴いた牌（捨て牌から取った牌）
        called_tile: Tile,
        /// 副露で公開された牌
        tiles: Vec<Tile>,
    },

    /// リーチ宣言
    PlayerRiichi {
        /// リーチしたプレイヤーの風
        player: Wind,
    },

    /// 手牌更新（鳴き後やリーチ後に自分の手牌を同期する）
    HandUpdated {
        /// 更新後の手牌
        hand: Vec<Tile>,
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
        /// 成立した役の一覧（役名, 翻数）
        yaku_list: Vec<(String, u32)>,
        /// 翻数
        han: u32,
        /// 符
        fu: u32,
        /// 和了者が得た点数
        score_points: i32,
        /// 点数等級名（満貫、跳満など。通常は空文字列）
        rank_name: String,
        /// 裏ドラ表示牌（リーチ和了時のみ公開）
        uradora_indicators: Vec<Tile>,
    },

    /// 局終了（流局）
    RoundDraw {
        /// 点数移動後の各プレイヤーの点数
        scores: [i32; 4],
        /// 流局の理由
        reason: DrawReason,
        /// テンパイしているプレイヤーの風（荒牌流局の場合のみ）
        tenpai: Vec<Wind>,
    },
}

/// クライアントからサーバへのアクション
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ClientAction {
    /// 牌を捨てる
    Discard {
        /// 捨てる牌（Noneならツモ切り）
        tile: Option<Tile>,
    },

    /// ツモ和了を宣言する
    Tsumo,

    /// ロン和了を宣言する
    Ron,

    /// リーチを宣言する
    Riichi {
        /// 捨てる牌（Noneならツモ切りリーチ）
        tile: Option<Tile>,
    },

    /// チーを宣言する
    Chi {
        /// 使用する手牌の牌種2つ（TileType値）
        tiles: [TileType; 2],
    },

    /// ポンを宣言する
    Pon,

    /// カンを宣言する（暗カン/明カン/加カン）
    Kan {
        /// カンする牌の種類
        tile_index: usize,
    },

    /// パス（鳴きやロンをしない）
    Pass,
}

