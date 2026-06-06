//! Trail formation — emergent path formation from repeated signal deposits.
//!
//! When multiple agents deposit pheromone along similar paths and signals
//! decay over time, trails emerge: well-traveled paths accumulate stronger
//! signals while unused paths evaporate. This module provides:
//!
//! - **TrailFormation**: a tracker that records deposit events and evaluates
//!   trail quality over time.
//! - **Trail quality metric**: measures how concentrated and connected signals
//!   are along a path.
//! - **Path following**: given a trail, suggests the next step along it.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::decay::SignalDecay;
use crate::environment::Environment;
use crate::signal::SignalType;

/// A waypoint in a trail.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Waypoint {
    pub x: usize,
    pub y: usize,
    pub strength: f64,
}

/// Metrics about trail quality.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TrailQuality {
    /// Total signal strength along the trail.
    pub total_strength: f64,
    /// Number of occupied cells.
    pub cell_count: usize,
    /// Average strength per occupied cell.
    pub avg_strength: f64,
    /// Fraction of trail cells that are connected (4-adjacent to another trail cell).
    pub connectivity: f64,
}

/// Configuration and state for trail formation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrailFormation {
    /// The signal type that forms trails.
    pub signal_type: SignalType,
    /// Minimum strength to be considered part of a trail.
    pub trail_threshold: f64,
    /// History of deposit positions (for analysis).
    pub deposit_history: Vec<Waypoint>,
}

impl TrailFormation {
    /// Create a new trail formation tracker.
    pub fn new(signal_type: SignalType, trail_threshold: f64) -> Self {
        Self {
            signal_type,
            trail_threshold,
            deposit_history: Vec::new(),
        }
    }

    /// Record a deposit at a position.
    pub fn record_deposit(&mut self, x: usize, y: usize, strength: f64) {
        self.deposit_history.push(Waypoint { x, y, strength });
    }

    /// Compute trail quality metrics from the current environment state.
    pub fn quality(&self, env: &Environment) -> TrailQuality {
        let mut trail_cells: Vec<(usize, usize, f64)> = Vec::new();
        for (x, y, cell) in env.occupied_cells() {
            if let Some(&s) = cell.get(&self.signal_type) {
                if s >= self.trail_threshold {
                    trail_cells.push((x, y, s));
                }
            }
        }

        let cell_count = trail_cells.len();
        let total_strength: f64 = trail_cells.iter().map(|(_, _, s)| s).sum();
        let avg_strength = if cell_count > 0 {
            total_strength / cell_count as f64
        } else {
            0.0
        };

        // Compute connectivity: fraction of trail cells adjacent to another trail cell
        let trail_set: HashMap<(usize, usize), ()> =
            trail_cells.iter().map(|&(x, y, _)| ((x, y), ())).collect();
        let connected_count = trail_cells
            .iter()
            .filter(|&&(x, y, _)| {
                let neighbors = env.neighbors4(x, y);
                neighbors
                    .iter()
                    .any(|&(nx, ny)| trail_set.contains_key(&(nx, ny)))
            })
            .count();

        let connectivity = if cell_count > 0 {
            connected_count as f64 / cell_count as f64
        } else {
            0.0
        };

        TrailQuality {
            total_strength,
            cell_count,
            avg_strength,
            connectivity,
        }
    }

    /// Given a position, follow the trail by moving to the strongest
    /// adjacent cell that is part of the trail.
    ///
    /// Returns `None` if no trail neighbor exists.
    pub fn follow(&self, env: &Environment, x: usize, y: usize) -> Option<(usize, usize)> {
        let neighbors = env.neighbors4(x, y);
        let mut best: Option<(usize, usize, f64)> = None;

        for &(nx, ny) in &neighbors {
            let val = env.read(nx, ny, self.signal_type);
            if val >= self.trail_threshold && (best.is_none() || val > best.unwrap().2) {
                best = Some((nx, ny, val));
            }
        }

        best.map(|(nx, ny, _)| (nx, ny))
    }

    /// Simulate a simple ant-like trail formation scenario.
    ///
    /// `agents` is a list of (x, y) start positions. Each agent deposits
    /// pheromone along a path toward `target`. Returns the environment after
    /// `steps` iterations.
    pub fn simulate_ants(
        &mut self,
        env: &mut Environment,
        agents: &[(usize, usize)],
        target: (usize, usize),
        steps: u64,
        deposit_strength: f64,
        decay: &SignalDecay,
    ) {
        let mut positions: Vec<(usize, usize)> = agents.to_vec();

        for _step in 0..steps {
            // Each agent deposits and moves
            for pos in &mut positions {
                let (ax, ay) = *pos;

                // Deposit pheromone at current position
                env.set(
                    ax,
                    ay,
                    self.signal_type,
                    env.read(ax, ay, self.signal_type) + deposit_strength,
                );
                self.record_deposit(ax, ay, deposit_strength);

                // Move toward target (simple greedy)
                if ax < target.0 {
                    pos.0 += 1;
                } else if ax > target.0 {
                    pos.0 -= 1;
                }
                if ay < target.1 {
                    pos.1 += 1;
                } else if ay > target.1 {
                    pos.1 -= 1;
                }
            }

            // Decay step
            decay.step(env);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn trail_tracker() -> TrailFormation {
        TrailFormation::new(SignalType::Pheromone, 0.1)
    }

    #[test]
    fn record_deposit() {
        let mut t = trail_tracker();
        t.record_deposit(5, 5, 1.0);
        t.record_deposit(6, 5, 0.8);
        assert_eq!(t.deposit_history.len(), 2);
        assert_eq!(t.deposit_history[0].x, 5);
    }

    #[test]
    fn quality_empty_environment() {
        let t = trail_tracker();
        let env = Environment::new(10, 10);
        let q = t.quality(&env);
        assert_eq!(q.cell_count, 0);
        assert_eq!(q.total_strength, 0.0);
        assert_eq!(q.connectivity, 0.0);
    }

    #[test]
    fn quality_with_trail() {
        let t = trail_tracker();
        let mut env = Environment::new(10, 10);
        // Linear trail
        for x in 0..5 {
            env.set(x, 5, SignalType::Pheromone, 1.0);
        }
        let q = t.quality(&env);
        assert_eq!(q.cell_count, 5);
        assert!((q.total_strength - 5.0).abs() < 1e-9);
        assert!((q.avg_strength - 1.0).abs() < 1e-9);
        // All connected except possibly endpoints
        assert!(q.connectivity >= 0.6);
    }

    #[test]
    fn quality_below_threshold_excluded() {
        let t = trail_tracker();
        let mut env = Environment::new(10, 10);
        env.set(0, 0, SignalType::Pheromone, 0.05); // below threshold 0.1
        let q = t.quality(&env);
        assert_eq!(q.cell_count, 0);
    }

    #[test]
    fn follow_trail() {
        let t = trail_tracker();
        let mut env = Environment::new(10, 10);
        env.set(5, 5, SignalType::Pheromone, 1.0);
        env.set(6, 5, SignalType::Pheromone, 2.0); // stronger to the right
        let next = t.follow(&env, 5, 5);
        assert_eq!(next, Some((6, 5)));
    }

    #[test]
    fn follow_no_trail() {
        let t = trail_tracker();
        let env = Environment::new(10, 10);
        assert!(t.follow(&env, 5, 5).is_none());
    }

    #[test]
    fn emergent_trail_formation() {
        let mut t = TrailFormation::new(SignalType::Pheromone, 0.01);
        let mut env = Environment::new(20, 20);
        let decay = SignalDecay::new(0.05, 0.1, 0.005);

        // 5 agents start at (0,10), heading to (19,10)
        let agents: Vec<(usize, usize)> = (0..5).map(|_| (0, 10)).collect();
        t.simulate_ants(&mut env, &agents, (19, 10), 30, 1.0, &decay);

        // After simulation, there should be a pheromone trail along y=10
        let q = t.quality(&env);
        assert!(
            q.cell_count > 3,
            "expected emergent trail, got {} cells",
            q.cell_count
        );
        assert!(
            q.total_strength > 0.5,
            "expected trail strength > 0.5, got {}",
            q.total_strength
        );
    }

    #[test]
    fn trail_strength_increases_with_more_agents() {
        let mut env1 = Environment::new(20, 20);
        let mut env2 = Environment::new(20, 20);
        let decay = SignalDecay::new(0.05, 0.05, 0.005);

        let mut t1 = TrailFormation::new(SignalType::Pheromone, 0.01);
        let agents1: Vec<(usize, usize)> = (0..1).map(|_| (0, 10)).collect();
        t1.simulate_ants(&mut env1, &agents1, (15, 10), 20, 1.0, &decay);

        let mut t2 = TrailFormation::new(SignalType::Pheromone, 0.01);
        let agents2: Vec<(usize, usize)> = (0..10).map(|_| (0, 10)).collect();
        t2.simulate_ants(&mut env2, &agents2, (15, 10), 20, 1.0, &decay);

        let q1 = t1.quality(&env1);
        let q2 = t2.quality(&env2);
        assert!(
            q2.total_strength > q1.total_strength,
            "more agents should produce stronger trail"
        );
    }

    #[test]
    fn connectivity_of_linear_trail() {
        let t = trail_tracker();
        let mut env = Environment::new(10, 1);
        // Single row trail — all cells connected
        for x in 0..10 {
            env.set(x, 0, SignalType::Pheromone, 1.0);
        }
        let q = t.quality(&env);
        assert!(
            (q.connectivity - 1.0).abs() < 1e-9,
            "fully connected trail should be 1.0"
        );
    }

    #[test]
    fn trail_formation_serde_roundtrip() {
        let mut t = trail_tracker();
        t.record_deposit(1, 2, 3.0);
        let json = serde_json::to_string(&t).unwrap();
        let back: TrailFormation = serde_json::from_str(&json).unwrap();
        assert_eq!(back.deposit_history.len(), 1);
    }
}
