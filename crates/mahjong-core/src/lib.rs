/// Board (table state shared by all players)
pub mod board;
/// Player's hand (tehai / 手牌)
pub mod hand;
/// Hand metadata (open/closed state, melds, etc.)
pub mod hand_info;
/// Minipoints (fu) and score calculation
pub mod scoring;
/// Rule settings
pub mod settings;
/// Tiles
pub mod tile;
/// Yaku (winning hand patterns)
pub mod winning_hand;
