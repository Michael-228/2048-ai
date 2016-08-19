// #![warn(missing_docs)]

//! This crate provides an implementation of an 2048 AI.
//!
//! The `board` module contains 2048 game logic.
//!
//! The `agent` module contains an AI player.
//!
//! The `heuristic` module contains various heuristics that the AI player can use to evaluate
//! board positions and try to maximize. It also contains the `Heuristic` trait that can be used
//! to implemnt your own heuristic.
//!
//! The `search_tree` module exposes a dynamically generated tree of possible board states.
//!
//! The `SearchResult` and `SearchStatistics` types are containers for the results of the AI
//! player's evaluation of a position and some interesting statistics.

extern crate rand;
extern crate time;
extern crate itertools;
extern crate lazycell;

pub use searcher::{SearchResult, SearchStatistics};

pub mod board;
pub mod agent;
pub mod heuristic;
pub mod search_tree;

mod searcher;
