//! ロビー（ルームレジストリ）
//!
//! ルームコードからルームアクターへの送信チャネルを引けるレジストリ。
//! ロックは作成・参照・削除の間だけ保持する（ゲーム状態は持たない）。

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use mahjong_server::table::GameSettings;
use rand::RngExt;
use tokio::sync::mpsc;

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
    pub fn create_room(&self, settings: GameSettings) -> (String, mpsc::Sender<RoomMsg>) {
        let (tx, rx) = mpsc::channel(64);

        let code = {
            let mut rooms = self.rooms.lock().unwrap();
            // 衝突したら引き直す
            let code = loop {
                let candidate = generate_code();
                if !rooms.contains_key(&candidate) {
                    break candidate;
                }
            };
            rooms.insert(code.clone(), tx.clone());
            code
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
        let normalized = code.trim().to_ascii_uppercase();
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

    #[tokio::test]
    async fn test_create_and_lookup_room() {
        let lobby = Lobby::new(RoomConfig::default());
        let (code, _tx) = lobby.create_room(GameSettings::default());

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
