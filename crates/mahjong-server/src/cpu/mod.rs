//! CPU AI players.
//!
//! CPUs talk to the server through the same protocol as human players
//! (ServerEvent / ClientAction); they never reach into server internals.

pub mod client;
pub mod defense;
pub mod evaluator;
pub mod heuristics;
pub mod personalities;
pub mod state;
