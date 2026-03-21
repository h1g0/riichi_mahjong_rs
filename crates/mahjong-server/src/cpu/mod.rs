//! CPU AI プレイヤーモジュール
//!
//! CPUはプレイヤーと同じプロトコル（ServerEvent / ClientAction）で
//! サーバとやり取りする。サーバ内部に直接アクセスしない。

pub mod client;
pub mod defense;
pub mod evaluator;
pub mod personalities;
pub mod state;
