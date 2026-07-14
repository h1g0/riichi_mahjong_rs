//! CPU AI players.
//!
//! CPUs talk to the server through the same gameplay protocol as human
//! players (`ServerEvent` / `ClientAction`). Immutable table rules are
//! supplied when a CPU is created; CPUs never reach into live server state.

pub mod client;
pub mod defense;
pub mod evaluator;
pub mod heuristics;
pub mod personalities;
pub mod state;
