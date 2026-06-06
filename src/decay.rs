//! Signal decay — exponential evaporation, spatial diffusion, and threshold pruning.
//!
//! Three mechanisms reduce and spread signals over time:
//! 1. **Evaporation**: each signal strength is multiplied by `(1 - evap_rate)`.
//! 2. **Diffusion**: a fraction of each cell's signal spreads to 4-connected neighbors.
//! 3. **Pruning**: signals below a threshold are removed entirely.

use serde::{Deserialize, Serialize};

use crate::environment::Environment;
use crate::signal::SignalType;

/// Configuration for signal decay processing.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SignalDecay {
    /// Fraction of signal lost per step (0.0–1.0).
    pub evaporation_rate: f64,
    /// Fraction of signal that diffuses to each neighbor per step (0.0–1.0).
    pub diffusion_rate: f64,
    /// Signals below this strength are pruned to zero.
    pub threshold: f64,
}

impl SignalDecay {
    /// Create a new decay configuration.
    pub fn new(evaporation_rate: f64, diffusion_rate: f64, threshold: f64) -> Self {
        Self {
            evaporation_rate,
            diffusion_rate,
            threshold,
        }
    }

    /// Apply one decay step to the entire environment.
    ///
    /// Order: diffuse → evaporate → prune.
    pub fn step(&self, env: &mut Environment) {
        self.diffuse(env);
        self.evaporate(env);
        self.prune(env);
    }

    /// Evaporate all signals: multiply by `(1 - evaporation_rate)`.
    pub fn evaporate(&self, env: &mut Environment) {
        let factor = 1.0 - self.evaporation_rate;
        for row in &mut env.grid {
            for cell in row {
                for v in cell.values_mut() {
                    *v *= factor;
                }
            }
        }
    }

    /// Diffuse signals to 4-connected neighbors.
    ///
    /// Each cell sends `diffusion_rate × strength` to each neighbor, and
    /// retains the remainder. Diffusion is computed from a snapshot so there's
    /// no order dependency.
    pub fn diffuse(&self, env: &mut Environment) {
        if self.diffusion_rate.abs() < f64::EPSILON {
            return;
        }
        // Snapshot current state
        let snapshot: Vec<Vec<std::collections::HashMap<SignalType, f64>>> = env.grid.clone();

        for (y, row) in snapshot.iter().enumerate() {
            for (x, cell) in row.iter().enumerate() {
                if cell.is_empty() {
                    continue;
                }
                let neighbors = env.neighbors4(x, y);
                for (&st, &strength) in cell {
                    if strength.abs() < f64::EPSILON {
                        continue;
                    }
                    let amount = self.diffusion_rate * strength;
                    let retain = strength - amount * neighbors.len() as f64;
                    // Update the source cell
                    env.grid[y][x].insert(st, retain.max(0.0));
                    // Distribute to neighbors
                    for &(nx, ny) in &neighbors {
                        env.grid[ny][nx]
                            .entry(st)
                            .and_modify(|v| *v += amount)
                            .or_insert(amount);
                    }
                }
            }
        }
    }

    /// Remove signals below the threshold.
    pub fn prune(&self, env: &mut Environment) {
        for row in &mut env.grid {
            for cell in row {
                cell.retain(|_, v| *v >= self.threshold);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fresh_env() -> Environment {
        Environment::new(10, 10)
    }

    #[test]
    fn evaporation_reduces_strength() {
        let mut env = fresh_env();
        env.set(5, 5, SignalType::Pheromone, 1.0);
        let decay = SignalDecay::new(0.1, 0.0, 0.0);
        decay.evaporate(&mut env);
        let val = env.read(5, 5, SignalType::Pheromone);
        assert!((val - 0.9).abs() < 1e-9);
    }

    #[test]
    fn repeated_evaporation() {
        let mut env = fresh_env();
        env.set(0, 0, SignalType::Pheromone, 1.0);
        let decay = SignalDecay::new(0.5, 0.0, 0.0);
        decay.evaporate(&mut env);
        decay.evaporate(&mut env);
        let val = env.read(0, 0, SignalType::Pheromone);
        assert!((val - 0.25).abs() < 1e-9);
    }

    #[test]
    fn diffusion_spreads_to_neighbors() {
        let mut env = fresh_env();
        env.set(5, 5, SignalType::Pheromone, 1.0);
        let decay = SignalDecay::new(0.0, 0.1, 0.0);
        decay.diffuse(&mut env);
        // Center retains 1.0 - 4 * 0.1 = 0.6
        let center = env.read(5, 5, SignalType::Pheromone);
        assert!((center - 0.6).abs() < 1e-9);
        // Each neighbor gets 0.1
        for &(nx, ny) in &env.neighbors4(5, 5) {
            let v = env.read(nx, ny, SignalType::Pheromone);
            assert!((v - 0.1).abs() < 1e-9, "neighbor ({},{})={}", nx, ny, v);
        }
    }

    #[test]
    fn no_diffusion_when_rate_zero() {
        let mut env = fresh_env();
        env.set(3, 3, SignalType::Marker, 1.0);
        let decay = SignalDecay::new(0.0, 0.0, 0.0);
        decay.diffuse(&mut env);
        assert!((env.read(3, 3, SignalType::Marker) - 1.0).abs() < 1e-9);
        assert_eq!(env.read(3, 4, SignalType::Marker), 0.0);
    }

    #[test]
    fn prune_removes_weak_signals() {
        let mut env = fresh_env();
        env.set(0, 0, SignalType::Pheromone, 0.05);
        env.set(1, 1, SignalType::Pheromone, 0.5);
        let decay = SignalDecay::new(0.0, 0.0, 0.1);
        decay.prune(&mut env);
        assert_eq!(env.read(0, 0, SignalType::Pheromone), 0.0);
        assert!((env.read(1, 1, SignalType::Pheromone) - 0.5).abs() < 1e-9);
    }

    #[test]
    fn full_step_evaporate_diffuse_prune() {
        let mut env = fresh_env();
        env.set(5, 5, SignalType::Pheromone, 1.0);
        let decay = SignalDecay::new(0.1, 0.1, 0.01);
        decay.step(&mut env);
        // After diffusion: center = 0.6, neighbors = 0.1
        // After evaporation (×0.9): center ≈ 0.54, neighbors ≈ 0.09
        // Both above threshold 0.01, so both survive
        let center = env.read(5, 5, SignalType::Pheromone);
        assert!(center > 0.4 && center < 0.7);
        let n = env.read(6, 5, SignalType::Pheromone);
        assert!(n > 0.05);
    }

    #[test]
    fn decay_repeatedly_converges_to_zero() {
        let mut env = fresh_env();
        env.set(0, 0, SignalType::Pheromone, 1.0);
        let decay = SignalDecay::new(0.3, 0.0, 0.01);
        for _ in 0..20 {
            decay.step(&mut env);
        }
        // Should have been pruned by now
        assert_eq!(env.read(0, 0, SignalType::Pheromone), 0.0);
    }

    #[test]
    fn diffuse_corner_cell() {
        let mut env = Environment::new(5, 5);
        env.set(0, 0, SignalType::Pheromone, 1.0);
        let decay = SignalDecay::new(0.0, 0.25, 0.0);
        decay.diffuse(&mut env);
        // Corner has 2 neighbors, retains 1.0 - 2*0.25 = 0.5
        let center = env.read(0, 0, SignalType::Pheromone);
        assert!((center - 0.5).abs() < 1e-9);
    }

    #[test]
    fn signal_decay_serde_roundtrip() {
        let d = SignalDecay::new(0.1, 0.2, 0.01);
        let json = serde_json::to_string(&d).unwrap();
        let back: SignalDecay = serde_json::from_str(&json).unwrap();
        assert_eq!(d, back);
    }
}
