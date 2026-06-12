//! ゲームアダプター
//!
//! クライアントUIとサーバロジックの間の抽象境界。
//! ローカル対戦（サーバ内蔵）とオンライン対戦（ネットワーク経由）を
//! 同じインターフェースで扱えるようにする。

mod local;

pub use local::LocalAdapter;

use mahjong_server::protocol::{ClientAction, ServerEvent};

/// クライアントUIから見たゲームサーバへのインターフェース
///
/// メインループはこのトレイト経由でアクション送信とイベント取得を行い、
/// 接続先がローカルかリモートかを意識しない。
pub trait GameAdapter {
    /// プレイヤーのアクションを送信する
    fn send_action(&mut self, action: ClientAction);

    /// 自分宛てのイベントを取得する
    fn poll_events(&mut self) -> Vec<ServerEvent>;

    /// ゲームを1ティック進める
    fn tick(&mut self);

    /// 局結果画面を確認し、次の局への進行を要求する
    fn request_next_round(&mut self);

    /// ゲームが終了しているか
    fn is_game_over(&self) -> bool;
}
