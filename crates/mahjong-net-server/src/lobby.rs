//! ロビー（ルームレジストリ）
//!
//! ルームコードからルームアクターへの送信チャネルを引けるレジストリ。
//! ロックは作成・参照・削除の間だけ保持する（ゲーム状態は持たない）。

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use mahjong_server::table::GameSettings;
use rand::RngExt;
use tokio::sync::mpsc;

use crate::peers::Peers;
use crate::room::{RoomConfig, RoomMsg, run_room};

/// ルームコードの文字種（紛らわしい 0/O/1/I を除いた32文字）
const CODE_ALPHABET: &[u8] = b"ABCDEFGHJKLMNPQRSTUVWXYZ23456789";

/// ルームコードの長さ
const CODE_LEN: usize = 6;

/// ルームコードを生成する（約30ビットのエントロピー）
fn generate_code() -> String {
    let mut rng = rand::rng();
    (0..CODE_LEN)
        .map(|_| CODE_ALPHABET[rng.random_range(0..CODE_ALPHABET.len())] as char)
        .collect()
}

/// ルームコードを正規化する（前後の空白を除き大文字にする）
pub fn normalize_code(code: &str) -> String {
    code.trim().to_ascii_uppercase()
}

/// 正規化済みのルームコードとして正しい形式か判定する
pub fn is_valid_code(code: &str) -> bool {
    code.len() == CODE_LEN && code.bytes().all(|b| CODE_ALPHABET.contains(&b))
}

/// ロビー: ルームコード → ルームアクターのレジストリ
#[derive(Clone)]
pub struct Lobby {
    rooms: Arc<Mutex<HashMap<String, mpsc::Sender<RoomMsg>>>>,
    config: RoomConfig,
}

impl Lobby {
    /// 新しいロビーを作成する
    pub fn new(config: RoomConfig) -> Self {
        Lobby {
            rooms: Arc::new(Mutex::new(HashMap::new())),
            config,
        }
    }

    /// ルームを作成し、アクタータスクを起動する
    ///
    /// 生成したルームコードと、ルームへの送信チャネルを返す。
    /// コードはローカルのレジストリに加えて、ピア（他マシン）のルームとも
    /// 衝突しないことを確認して確定する（衝突すると参加者が誤ったルームへ
    /// 転送されうるため）。
    pub async fn create_room(
        &self,
        settings: GameSettings,
        peers: &Peers,
    ) -> (String, mpsc::Sender<RoomMsg>) {
        let (tx, rx) = mpsc::channel(64);

        let code = loop {
            // ローカルで未使用の候補を選び、ピアにも照会する
            let candidate = peers
                .pick_unused_code(|| {
                    let rooms = self.rooms.lock().unwrap();
                    loop {
                        let candidate = generate_code();
                        if !rooms.contains_key(&candidate) {
                            break candidate;
                        }
                    }
                })
                .await;
            // ピア照会（await）中にローカルで同じコードが使われた可能性が
            // あるため、挿入と同じロック内で再確認する
            let mut rooms = self.rooms.lock().unwrap();
            if !rooms.contains_key(&candidate) {
                rooms.insert(candidate.clone(), tx.clone());
                break candidate;
            }
        };

        tracing::info!(code, "room created");
        tokio::spawn(run_room(
            code.clone(),
            settings,
            self.clone(),
            rx,
            self.config,
        ));

        (code, tx)
    }

    /// ルームコードからルームを引く（大文字小文字は区別しない）
    pub fn get(&self, code: &str) -> Option<mpsc::Sender<RoomMsg>> {
        let normalized = normalize_code(code);
        self.rooms.lock().unwrap().get(&normalized).cloned()
    }

    /// ルームをレジストリから削除する（ルームアクターが終了時に呼ぶ）
    pub fn remove(&self, code: &str) {
        self.rooms.lock().unwrap().remove(code);
        tracing::info!(code, "room removed");
    }

    /// 現在のルーム数
    pub fn room_count(&self) -> usize {
        self.rooms.lock().unwrap().len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_code_format() {
        for _ in 0..100 {
            let code = generate_code();
            assert_eq!(code.len(), CODE_LEN);
            assert!(
                code.bytes().all(|b| CODE_ALPHABET.contains(&b)),
                "コードに不正な文字が含まれる: {code}"
            );
            // 紛らわしい文字が含まれない
            assert!(!code.contains(['0', 'O', '1', 'I']));
        }
    }

    #[test]
    fn test_normalize_and_validate_code() {
        assert_eq!(normalize_code(" abc234 "), "ABC234");
        assert!(is_valid_code("ABC234"));
        // 長さ違い・不正文字（0/O/1/I や小文字）は拒否
        assert!(!is_valid_code("ABC23"));
        assert!(!is_valid_code("ABC2345"));
        assert!(!is_valid_code("ABC0O1"));
        assert!(!is_valid_code("abc234"));
    }

    #[tokio::test]
    async fn test_create_and_lookup_room() {
        let lobby = Lobby::new(RoomConfig::default());
        let (code, _tx) = lobby
            .create_room(GameSettings::default(), &Peers::none())
            .await;

        assert_eq!(lobby.room_count(), 1);
        assert!(lobby.get(&code).is_some());
        // 小文字や空白付きでも引ける
        assert!(
            lobby
                .get(&format!(" {} ", code.to_ascii_lowercase()))
                .is_some()
        );
        assert!(lobby.get("ZZZZZZ").is_none());

        lobby.remove(&code);
        assert_eq!(lobby.room_count(), 0);
    }
}
