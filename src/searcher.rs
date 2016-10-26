//! This is the meat of the library. This module implements an `ExpectiMax` search
//! (`https://en.wikipedia.org/wiki/Expectiminimax_tree` - we don't need a MIN node, since
//! the Computer player is not trying to win). So we're just trying to find the best moves
//! after which, with perfect play, we expect the heuristic value to be the highest, on average.
//!
//! Of course, it's impossible to calculate the whole tree in most board positions, so we stop
//! going deeper into the search tree as soon as we either reach a node whose probabiltiy is lower
//! than some value, or as soon as we reach a certain depth, whichever happens first.
//!
//! As soon as that happens, or we reach a terminal (Game Over) node, we run the provided heuristic
//! against the current board state and pass it up.
//!
//! Player nodes are MAX nodes that seek to select the best continuation, and so discard all other
//! evaluations before passing the evaluation up.
//!
//! Computer nodes are AVG nodes that return the weighted average of its child states.

use board::{Board, Move};
use heuristic::Heuristic;
use itertools::Itertools;
use search_tree::{ComputerNode, PlayerNode, SearchTree};
use std::collections::HashMap;
use std::f32;
use time::{self, Duration};

const PROBABILITY_OF2: f32 = 0.9;
const PROBABILITY_OF4: f32 = 0.1;

/// Not sure why I created a trait. I used to experiment a lot with different search methods,
/// but I don't think I'll find a better algorithm than `ExpectiMax` now.
pub trait Searcher {
    fn search(&self, search_tree: &SearchTree) -> SearchResult;
}

/// The main consumer of computational resources of the program.
pub struct ExpectiMaxer<H: Heuristic> {
    min_probability: f32,
    max_search_depth: u8,
    heuristic: H,
}

/// Return a numnber of interesting statistics together with a recommendation for the best move.
pub struct SearchResult {
    /// Some useful statistics
    pub search_statistics: SearchStatistics,
    /// The game state for which analysis was conducted.
    pub root_board: Board,
    /// A map of evaluations. Can be empty if the player has no more moves, that is,
    /// in a game over state.
    pub move_evaluations: HashMap<Move, f32>,
    /// The best move, if one exists. Can be `None` if the player has no available
    /// moves, that is, in a game over state.
    pub best_move: Option<(Move, f32)>,
}

/// These are the interesting statistics. May add some more later.
pub struct SearchStatistics {
    /// The time it took for the search to complete.
    pub search_duration: Duration,
    /// The number of search tree nodes visited.
    pub nodes_traversed: usize,
    /// The number of nodes for which the game state was evaluated with a heuristic.
    pub terminal_traversed: usize,
    /// Known unique search tree nodes that represent the Player's turn.
    pub known_player_nodes: usize,
    /// Known unique search tree nodes that represent the Computer's turn.
    pub known_computer_nodes: usize,
    /// New unique game states that the Player can encounter that were found
    /// during this search.
    pub new_player_nodes: usize,
    /// New unique game states that the Computer can encounter that were found
    /// during this search.
    pub new_computer_nodes: usize,
}

// Unfortunately, can't derive a default trait, since Duration apparently doesn't implemnt it
// (why?)
impl Default for SearchStatistics {
    fn default() -> Self {
        SearchStatistics {
            search_duration: Duration::zero(),
            nodes_traversed: 0,
            terminal_traversed: 0,
            known_player_nodes: 0,
            known_computer_nodes: 0,
            new_player_nodes: 0,
            new_computer_nodes: 0,
        }
    }
}

// Helper methods to compute some derivative values.
impl SearchStatistics {
    fn known_nodes(&self) -> usize {
        self.known_player_nodes + self.known_computer_nodes
    }
    fn new_nodes(&self) -> usize {
        self.new_player_nodes + self.new_computer_nodes
    }
    fn nodes_per_second(&self) -> u64 {
        (self.nodes_traversed as f32 *
         (1_000_000_000f32 / self.search_duration.num_nanoseconds().unwrap() as f32)) as u64
    }
    fn new_nodes_per_second(&self) -> u64 {
        (self.new_nodes() as f32 *
         (1_000_000_000f32 / self.search_duration.num_nanoseconds().unwrap() as f32)) as u64
    }
}

// I really should be using a formatter...
impl ToString for SearchStatistics {
    fn to_string(&self) -> String {
        let mut result = String::new();
        result.push_str("Statistics:\n");
        result.push_str(&format!("Search duration:       {}\n", self.search_duration));
        result.push_str(&format!("Known nodes:           {}\n", self.known_nodes()));
        result.push_str(&format!("Nodes traversed:       {}\n", self.nodes_traversed));
        result.push_str(&format!("New nodes:             {}\n", self.new_nodes()));
        result.push_str(&format!("Terminal nodes:        {}\n", self.terminal_traversed));
        result.push_str(&format!("Nodes per second:      {}\n", self.nodes_per_second()));
        result.push_str(&format!("New nodes per second:  {}\n", self.new_nodes_per_second()));

        result
    }
}

impl<H: Heuristic> Searcher for ExpectiMaxer<H> {
    /// Do the search.
    fn search(&self, search_tree: &SearchTree) -> SearchResult {
        let mut search_statistics = SearchStatistics::default();

        // gather some data before starting the search
        let start = time::now_utc();
        let known_player_nodes_start = search_tree.known_player_node_count();
        let known_computer_nodes_start = search_tree.known_computer_node_count();

        // actual search
        let hashmap = self.init(search_tree, &mut search_statistics);

        // gather some data after finishing the search
        let finish = time::now_utc();
        let elapsed = finish - start;
        let known_player_nodes_finish = search_tree.known_player_node_count();
        let known_computer_nodes_finish = search_tree.known_computer_node_count();

        // compute some deltas
        search_statistics.search_duration = elapsed;
        search_statistics.new_computer_nodes = known_computer_nodes_finish -
                                               known_computer_nodes_start;
        search_statistics.new_player_nodes = known_player_nodes_finish - known_player_nodes_start;
        search_statistics.known_computer_nodes = known_computer_nodes_finish;
        search_statistics.known_player_nodes = known_player_nodes_finish;

        // find the best evaluation and move
        let best_move = hashmap.iter()
            .sorted_by(|&a, &b| b.1.partial_cmp(a.1).unwrap())
            .into_iter()
            .map(|(&mv, &eval)| (mv, eval))
            .nth(0);

        SearchResult {
            root_board: *search_tree.root().board(),
            move_evaluations: hashmap,
            search_statistics: search_statistics,
            best_move: best_move,
        }
    }
}

impl<H: Heuristic> ExpectiMaxer<H> {
    /// Creates a new `ExpectiMaxer`. Require the heuristic to use, the limit probability
    /// lower than which we'll won't search, and the maximum search depth.
    pub fn new(min_probability: f32, max_search_depth: u8, heuristic: H) -> Self {
        assert!(max_search_depth != 0);
        ExpectiMaxer {
            min_probability: min_probability,
            max_search_depth: max_search_depth,
            heuristic: heuristic,
        }
    }

    fn init(&self,
            search_tree: &SearchTree,
            mut search_statistics: &mut SearchStatistics)
            -> HashMap<Move, f32> {
        let children = search_tree.root().children_by_move();

        if children.is_empty() {
            return HashMap::new();
        }

        children.iter()
            .map(|(m, n)| {
                let eval =
                    self.computer_node_eval(n, self.max_search_depth, 1f32, &mut search_statistics);
                (m, eval)
            })
            .collect()
    }

    fn player_node_eval(&self,
                        node: &PlayerNode,
                        depth: u8,
                        probability: f32,
                        mut search_statistics: &mut SearchStatistics)
                        -> f32 {
        search_statistics.nodes_traversed += 1;

        let children = node.children_by_move();

        if children.is_empty() || depth == 0 || probability < self.min_probability {
            search_statistics.terminal_traversed += 1;

            let heur = match node.heuristic.get() {
                Some(heur) => heur,
                None => {
                    let heur = self.heuristic.eval(node);
                    node.heuristic.set(Some(heur));
                    heur
                }
            };

            return heur;
        }

        children.values()
            .map(|n| self.computer_node_eval(n, depth, probability, &mut search_statistics))
            .fold(f32::NAN, f32::max)
    }

    fn computer_node_eval(&self,
                          node: &ComputerNode,
                          depth: u8,
                          probability: f32,
                          mut search_statistics: &mut SearchStatistics)
                          -> f32 {
        search_statistics.nodes_traversed += 1;
        let children = node.children();
        let count = children.variants();

        let avg_with2 = children
            .with2()
            .map(|n| {
                self.player_node_eval(n,
                                      depth - 1,
                                      probability * PROBABILITY_OF2 / (count as f32),
                                      &mut search_statistics)
            })
            .sum::<f32>() / (count as f32);

        let avg_with4 = children
            .with4()
            .map(|n| {
                self.player_node_eval(n,
                                      depth - 1,
                                      probability * PROBABILITY_OF4 / (count as f32),
                                      &mut search_statistics)
            })
            .sum::<f32>() / (count as f32);

        avg_with2 * PROBABILITY_OF2 + avg_with4 * PROBABILITY_OF4
    }
}

#[cfg(test)]
mod tests {
    use board::Board;
    use heuristic::composite::CompositeHeuristic;
    use search_tree::SearchTree;
    use super::*;

    #[test]
    fn can_search_result() {
        let board = Board::default().add_random_tile();
        let search_tree = SearchTree::new(board);
        let heuristic = CompositeHeuristic::default();
        let searcher = ExpectiMaxer::new(0.01, 3, heuristic);

        let result = searcher.search(&search_tree);

        assert_eq!(result.root_board, board);
        assert!(result.move_evaluations.len() >= 2);
        assert!(result.move_evaluations.len() <= 4);
    }
}
