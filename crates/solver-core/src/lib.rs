use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap, HashSet, VecDeque};
use std::fmt;
use std::path::Path;

/// Number of rows on the Chomping Glass board.
pub const ROWS: usize = 5;
/// Number of columns on the Chomping Glass board.
pub const COLS: usize = 8;

/// The coordinates of the poison square (zero-indexed).
pub const POISON: Move = Move {
    row: (ROWS - 1) as u8,
    col: (COLS - 1) as u8,
};

/// Representation of a solver move in zero-indexed board coordinates.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash, Serialize, Deserialize)]
pub struct Move {
    pub row: u8,
    pub col: u8,
}

impl Move {
    pub const fn new(row: u8, col: u8) -> Self {
        Self { row, col }
    }

    pub const fn to_tuple(self) -> (u8, u8) {
        (self.row, self.col)
    }

    pub fn to_one_indexed(self) -> (u8, u8) {
        (self.row + 1, self.col + 1)
    }
}

/// Board state encoded as column heights (Ferrers shape).
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub struct BoardState {
    heights: [i8; COLS],
}

impl BoardState {
    /// Fresh board.
    pub fn new() -> Self {
        Self {
            heights: [-1; COLS],
        }
    }

    /// Construct from explicit heights (mostly useful for tests).
    pub fn from_heights(heights: [i8; COLS]) -> Self {
        debug_assert!(heights
            .iter()
            .all(|&h| (-1..=(ROWS as i8 - 1)).contains(&h)));
        Self { heights }
    }

    pub fn heights(&self) -> &[i8; COLS] {
        &self.heights
    }

    /// Return a new state after applying `mv`.
    /// Eats the candy at (row, col) and all candies above it and to the left.
    pub fn apply_move(&self, mv: Move) -> Self {
        let mut next = *self;
        let target_row = mv.row as i8;
        // Eat all columns from 0 up to and including mv.col (left and including chosen candy)
        for col in 0..=mv.col as usize {
            if target_row > next.heights[col] {
                next.heights[col] = target_row;
            }
        }
        next
    }

    /// Generate every legal candy move from this position.
    pub fn legal_moves(&self) -> Vec<Move> {
        let mut moves = Vec::new();
        for col in 0..COLS {
            let top_eaten = self.heights[col];
            for row in (top_eaten + 1)..(ROWS as i8) {
                let mv = Move::new(row as u8, col as u8);
                if mv == POISON {
                    continue;
                }
                moves.push(mv);
            }
        }
        moves
    }

    pub fn is_terminal(&self) -> bool {
        self.legal_moves().is_empty()
    }
}

impl Default for BoardState {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for BoardState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for row in 0..ROWS {
            for col in 0..COLS {
                let eaten = self.heights[col] >= row as i8;
                let symbol = if (row as u8, col as u8) == POISON.to_tuple() {
                    'X'
                } else if eaten {
                    '.'
                } else {
                    'o'
                };
                write!(f, "  {}", symbol)?;
            }
            writeln!(f)?;
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Evaluation {
    pub winning: bool,
    pub winning_moves: Vec<Move>,
}

/// Memoizing solver for the 5×8 board.
#[derive(Default)]
pub struct Solver {
    cache: HashMap<BoardState, Evaluation>,
}

impl Solver {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn evaluate(&mut self, state: BoardState) -> Evaluation {
        if let Some(entry) = self.cache.get(&state) {
            return entry.clone();
        }

        let moves = state.legal_moves();
        if moves.is_empty() {
            let eval = Evaluation {
                winning: false,
                winning_moves: Vec::new(),
            };
            self.cache.insert(state, eval.clone());
            return eval;
        }

        let mut winning_moves = Vec::new();
        for mv in moves {
            let next_state = state.apply_move(mv);
            if !self.evaluate(next_state).winning {
                winning_moves.push(mv);
            }
        }

        let eval = Evaluation {
            winning: !winning_moves.is_empty(),
            winning_moves,
        };
        self.cache.insert(state, eval.clone());
        eval
    }
}

/// Enumerate every reachable board state via BFS.
pub fn enumerate_states() -> Vec<BoardState> {
    let start = BoardState::new();
    let mut seen = HashSet::new();
    let mut queue = VecDeque::new();
    seen.insert(start);
    queue.push_back(start);

    while let Some(state) = queue.pop_front() {
        for mv in state.legal_moves() {
            let next = state.apply_move(mv);
            if seen.insert(next) {
                queue.push_back(next);
            }
        }
    }

    seen.into_iter().collect()
}

/// Export the complete policy table to JSON on disk.
pub fn export_policy_json<P: AsRef<Path>>(path: P) -> anyhow::Result<()> {
    let mut solver = Solver::new();
    let mut table = BTreeMap::new();
    for state in enumerate_states() {
        let eval = solver.evaluate(state);
        table.insert(format!("{:?}", state.heights), eval);
    }
    let writer = std::fs::File::create(path)?;
    serde_json::to_writer_pretty(writer, &table)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unique_winning_opening() {
        let mut solver = Solver::new();
        let start = BoardState::new();
        let eval = solver.evaluate(start);
        assert!(eval.winning);
        let moves: Vec<(u8, u8)> = eval.winning_moves.iter().map(|m| m.to_tuple()).collect();
        assert_eq!(moves, vec![(0, 1)]);
    }

    #[test]
    fn second_move_book_matches_notes() {
        let mut solver = Solver::new();
        let start = BoardState::new().apply_move(Move::new(0, 1));

        let ai_responses = [
            (Move::new(1, 0), vec![Move::new(0, 4)]),
            (Move::new(0, 2), vec![Move::new(3, 1)]),
            (Move::new(2, 0), vec![Move::new(1, 2), Move::new(0, 3)]),
            (Move::new(3, 0), vec![Move::new(2, 7)]),
            (Move::new(4, 0), vec![Move::new(3, 5)]),
        ];

        for (ai_move, expected) in ai_responses {
            let state = start.apply_move(ai_move);
            let eval = solver.evaluate(state);
            assert!(eval.winning);
            let mut got: Vec<(u8, u8)> =
                eval.winning_moves.iter().map(|mv| mv.to_tuple()).collect();
            got.sort();
            let mut expected_sorted: Vec<(u8, u8)> =
                expected.into_iter().map(|mv| mv.to_tuple()).collect();
            expected_sorted.sort();
            assert_eq!(got, expected_sorted);
        }
    }
}
