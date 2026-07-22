//! Lobby (room registry): maps room codes to the room actors' send
//! channels. The lock is held only across create/lookup/remove; no game
//! state lives here.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use mahjong_server::table::GameSettings;
use rand::RngExt;
use tokio::sync::mpsc;

use crate::peers::Peers;
use crate::room::{RoomConfig, RoomMsg, run_room};

/// Room-code alphabet: 32 characters, excluding the confusable 0/O/1/I.
const CODE_ALPHABET: &[u8] = b"ABCDEFGHJKLMNPQRSTUVWXYZ23456789";

/// Room-code length.
const CODE_LEN: usize = 6;

/// Generates a room code (~30 bits of entropy).
fn generate_code() -> String {
    let mut rng = rand::rng();
    (0..CODE_LEN)
        .map(|_| CODE_ALPHABET[rng.random_range(0..CODE_ALPHABET.len())] as char)
        .collect()
}

/// Normalizes a room code: trim and uppercase.
pub fn normalize_code(code: &str) -> String {
    code.trim().to_ascii_uppercase()
}

/// Whether a normalized room code is well-formed.
pub fn is_valid_code(code: &str) -> bool {
    code.len() == CODE_LEN && code.bytes().all(|b| CODE_ALPHABET.contains(&b))
}

/// The lobby: room code to room actor registry.
#[derive(Clone)]
pub struct Lobby {
    rooms: Arc<Mutex<HashMap<String, mpsc::Sender<RoomMsg>>>>,
    config: RoomConfig,
}

impl Lobby {
    pub fn new(config: RoomConfig) -> Self {
        Lobby {
            rooms: Arc::new(Mutex::new(HashMap::new())),
            config,
        }
    }

    /// Creates a room and spawns its actor task.
    ///
    /// Returns the generated code and the room's send channel. The code
    /// is checked against both the local registry and the peers' rooms:
    /// a collision could forward joiners to the wrong room.
    pub async fn create_room(
        &self,
        settings: GameSettings,
        peers: &Peers,
    ) -> (String, mpsc::Sender<RoomMsg>) {
        let (tx, rx) = mpsc::channel(64);

        let code = loop {
            // Pick a locally unused candidate, then ask the peers.
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
            // The code may have been taken locally while the peer query
            // awaited; recheck under the same lock as the insert.
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

    /// Looks up a room by code, case-insensitively.
    pub fn get(&self, code: &str) -> Option<mpsc::Sender<RoomMsg>> {
        let normalized = normalize_code(code);
        self.rooms.lock().unwrap().get(&normalized).cloned()
    }

    /// Removes a room; called by the actor as it exits.
    pub fn remove(&self, code: &str) {
        self.rooms.lock().unwrap().remove(code);
        tracing::info!(code, "room removed");
    }

    /// Current room count.
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
                "room code contains an invalid character: {code}"
            );
            // No confusable characters.
            assert!(!code.contains(['0', 'O', '1', 'I']));
        }
    }

    #[test]
    fn test_normalize_and_validate_code() {
        assert_eq!(normalize_code(" abc234 "), "ABC234");
        assert!(is_valid_code("ABC234"));
        // Wrong lengths and invalid characters (0/O/1/I, lowercase)
        // are rejected.
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
        // Lookup tolerates lowercase and surrounding whitespace.
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
