//! 2D environment grid for signal deposit and retrieval.
//!
//! The `Environment` is a width × height grid. Each cell stores a `HashMap`
//! keyed by `SignalType` (serialized as a simple string) mapping to the
//! aggregated signal strength at that cell. This keeps serde simple — no
//! complex or tuple-struct keys.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::signal::{Signal, SignalType};

/// A 2D grid environment where agents deposit and read signals.
///
/// Internally the grid is `Vec<Vec<HashMap<SignalType, f64>>>` — one map per
/// cell. This avoids complex key types in serde while still allowing O(1)
/// lookup by signal type.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Environment {
    /// Grid width (number of columns).
    pub width: usize,
    /// Grid height (number of rows).
    pub height: usize,
    /// The signal grid: `grid[y][x]` returns the HashMap for cell (x, y).
    pub grid: Vec<Vec<HashMap<SignalType, f64>>>,
}

impl Environment {
    /// Create a new environment of the given dimensions, all cells empty.
    pub fn new(width: usize, height: usize) -> Self {
        let row = vec![HashMap::new(); width];
        let grid = vec![row; height];
        Self {
            width,
            height,
            grid,
        }
    }

    /// Deposit a signal into the environment, aggregating strength.
    ///
    /// If a signal of the same type already exists at the cell, strengths
    /// are summed.
    pub fn deposit(&mut self, signal: &Signal) {
        if signal.x < self.width && signal.y < self.height {
            let cell = &mut self.grid[signal.y][signal.x];
            cell.entry(signal.signal_type)
                .and_modify(|v| *v += signal.strength)
                .or_insert(signal.strength);
        }
    }

    /// Read the strength of a signal type at a given cell.
    pub fn read(&self, x: usize, y: usize, signal_type: SignalType) -> f64 {
        if x < self.width && y < self.height {
            *self.grid[y][x].get(&signal_type).unwrap_or(&0.0)
        } else {
            0.0
        }
    }

    /// Get all signal strengths at a cell as a cloned HashMap.
    pub fn read_cell(&self, x: usize, y: usize) -> HashMap<SignalType, f64> {
        if x < self.width && y < self.height {
            self.grid[y][x].clone()
        } else {
            HashMap::new()
        }
    }

    /// Set the strength of a signal type at a cell directly.
    pub fn set(&mut self, x: usize, y: usize, signal_type: SignalType, strength: f64) {
        if x < self.width && y < self.height {
            if strength.abs() < f64::EPSILON {
                self.grid[y][x].remove(&signal_type);
            } else {
                self.grid[y][x].insert(signal_type, strength);
            }
        }
    }

    /// Clear all signals from every cell.
    pub fn clear(&mut self) {
        for row in &mut self.grid {
            for cell in row {
                cell.clear();
            }
        }
    }

    /// Return the total signal strength summed over all cells and types.
    pub fn total_signal(&self) -> f64 {
        let mut total = 0.0;
        for row in &self.grid {
            for cell in row {
                for &v in cell.values() {
                    total += v;
                }
            }
        }
        total
    }

    /// Iterate over all non-empty cells: yields `(x, y, &HashMap)`.
    pub fn occupied_cells(&self) -> Vec<(usize, usize, &HashMap<SignalType, f64>)> {
        let mut result = Vec::new();
        for (y, row) in self.grid.iter().enumerate() {
            for (x, cell) in row.iter().enumerate() {
                if !cell.is_empty() {
                    result.push((x, y, cell));
                }
            }
        }
        result
    }

    /// Returns true if the coordinates are within bounds.
    pub fn in_bounds(&self, x: usize, y: usize) -> bool {
        x < self.width && y < self.height
    }

    /// Return the 4-connected neighbors of (x, y) that are in bounds.
    pub fn neighbors4(&self, x: usize, y: usize) -> Vec<(usize, usize)> {
        let mut result = Vec::new();
        if x > 0 {
            result.push((x - 1, y));
        }
        if x + 1 < self.width {
            result.push((x + 1, y));
        }
        if y > 0 {
            result.push((x, y - 1));
        }
        if y + 1 < self.height {
            result.push((x, y + 1));
        }
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_environment_is_empty() {
        let env = Environment::new(10, 10);
        assert_eq!(env.width, 10);
        assert_eq!(env.height, 10);
        assert_eq!(env.total_signal(), 0.0);
    }

    #[test]
    fn deposit_and_read() {
        let mut env = Environment::new(10, 10);
        let s = Signal::pheromone(2.5, 3, 4, 0, 0.1);
        env.deposit(&s);
        let val = env.read(3, 4, SignalType::Pheromone);
        assert!((val - 2.5).abs() < 1e-9);
    }

    #[test]
    fn deposit_aggregates() {
        let mut env = Environment::new(5, 5);
        let s1 = Signal::pheromone(1.0, 0, 0, 0, 0.1);
        let s2 = Signal::pheromone(2.0, 0, 0, 0, 0.1);
        env.deposit(&s1);
        env.deposit(&s2);
        assert!((env.read(0, 0, SignalType::Pheromone) - 3.0).abs() < 1e-9);
    }

    #[test]
    fn deposit_out_of_bounds_ignored() {
        let mut env = Environment::new(5, 5);
        let s = Signal::pheromone(1.0, 10, 10, 0, 0.1);
        env.deposit(&s);
        assert_eq!(env.total_signal(), 0.0);
    }

    #[test]
    fn read_out_of_bounds_zero() {
        let env = Environment::new(5, 5);
        assert_eq!(env.read(99, 99, SignalType::Pheromone), 0.0);
    }

    #[test]
    fn set_and_clear() {
        let mut env = Environment::new(3, 3);
        env.set(1, 1, SignalType::Marker, 5.0);
        assert!((env.read(1, 1, SignalType::Marker) - 5.0).abs() < 1e-9);
        env.set(1, 1, SignalType::Marker, 0.0); // zero removes
        assert_eq!(env.read(1, 1, SignalType::Marker), 0.0);
    }

    #[test]
    fn clear_empties_all() {
        let mut env = Environment::new(3, 3);
        env.set(0, 0, SignalType::Pheromone, 1.0);
        env.set(1, 1, SignalType::Barrier, 2.0);
        env.clear();
        assert_eq!(env.total_signal(), 0.0);
    }

    #[test]
    fn occupied_cells() {
        let mut env = Environment::new(3, 3);
        env.set(0, 0, SignalType::Pheromone, 1.0);
        env.set(2, 2, SignalType::Marker, 1.0);
        let occ = env.occupied_cells();
        assert_eq!(occ.len(), 2);
    }

    #[test]
    fn neighbors4() {
        let env = Environment::new(5, 5);
        let n = env.neighbors4(2, 2);
        assert_eq!(n.len(), 4);
        let corner = env.neighbors4(0, 0);
        assert_eq!(corner.len(), 2);
    }

    #[test]
    fn in_bounds() {
        let env = Environment::new(5, 5);
        assert!(env.in_bounds(0, 0));
        assert!(env.in_bounds(4, 4));
        assert!(!env.in_bounds(5, 0));
    }

    #[test]
    fn environment_serde_roundtrip() {
        let mut env = Environment::new(3, 3);
        env.set(1, 1, SignalType::Pheromone, 2.0);
        let json = serde_json::to_string(&env).unwrap();
        let back: Environment = serde_json::from_str(&json).unwrap();
        assert_eq!(back.width, 3);
        assert!((back.read(1, 1, SignalType::Pheromone) - 2.0).abs() < 1e-9);
    }

    #[test]
    fn total_signal_sums_all() {
        let mut env = Environment::new(2, 2);
        env.set(0, 0, SignalType::Pheromone, 1.0);
        env.set(0, 0, SignalType::Barrier, 2.0);
        env.set(1, 1, SignalType::Marker, 3.0);
        assert!((env.total_signal() - 6.0).abs() < 1e-9);
    }
}
