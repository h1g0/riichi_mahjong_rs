//! ゲームアダプター
//!
//! クライアントUIとサーバロジックの間の抽象境界。
//! ローカル対戦（サーバ内蔵）とオンライン対戦（ネットワーク経由）を
//! 同じインターフェースで扱えるようにする。

mod local;
mod remote;

pub use local::LocalAdapter;
pub use remote::{ConnStatus, RemoteAdapter, RoomView, error_code_message};

use mahjong_core::settings::Lang;
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

    /// 接続状態などの表示用テキスト（問題がなければ None）
    fn status_text(&self, _lang: Lang) -> Option<String> {
        None
    }

    /// 手番の制限時間の残り秒数（制限がなければ None）
    fn turn_remaining_secs(&self) -> Option<u32> {
        None
    }
}
