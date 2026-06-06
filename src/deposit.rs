//! Deposit strategies — how agents place signals into the environment.
//!
//! Different deposit patterns produce different emergent behaviors:
//! - **Point** deposit: signal at a single cell (waypoint marking)
//! - **Gradient** deposit: signal spread over a radius with diminishing strength
//! - **Broadcast** deposit: uniform signal over a rectangular area

use serde::{Deserialize, Serialize};

use crate::environment::Environment;
use crate::signal::{Signal, SignalType};

/// Strategy for depositing signals.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "strategy", rename_all = "snake_case")]
pub enum DepositStrategy {
    /// Place signal at exactly one cell.
    Point,
    /// Place signal over a radius with strength falling off linearly.
    Gradient {
        /// Radius in cells (0 = same as point).
        radius: usize,
    },
    /// Place uniform signal over a rectangular area.
    Broadcast {
        /// Half-width of the broadcast area.
        half_width: usize,
        /// Half-height of the broadcast area.
        half_height: usize,
    },
}

impl DepositStrategy {
    /// Deposit a signal into the environment using this strategy.
    ///
    /// The `base_strength` is the peak strength. For gradient deposits it
    /// diminishes with distance; for broadcast it's uniform.
    #[allow(clippy::too_many_arguments)]
    pub fn deposit(
        &self,
        env: &mut Environment,
        signal_type: SignalType,
        base_strength: f64,
        x: usize,
        y: usize,
        timestamp: u64,
        decay_rate: f64,
    ) {
        match self {
            DepositStrategy::Point => {
                if env.in_bounds(x, y) {
                    let s = Signal::new(signal_type, base_strength, x, y, timestamp, decay_rate);
                    env.deposit(&s);
                }
            }
            DepositStrategy::Gradient { radius } => {
                let r = *radius as i32;
                let xi = x as i32;
                let yi = y as i32;
                for dy in -r..=r {
                    for dx in -r..=r {
                        let dist = ((dx.unsigned_abs() + dy.unsigned_abs()) as f64).sqrt();
                        if dist <= *radius as f64 {
                            let nx = (xi + dx) as usize;
                            let ny = (yi + dy) as usize;
                            if env.in_bounds(nx, ny) && (xi + dx) >= 0 && (yi + dy) >= 0 {
                                let strength = if *radius == 0 {
                                    base_strength
                                } else {
                                    base_strength * (1.0 - dist / (*radius as f64 + 1.0))
                                };
                                if strength > 0.0 {
                                    let s = Signal::new(
                                        signal_type,
                                        strength,
                                        nx,
                                        ny,
                                        timestamp,
                                        decay_rate,
                                    );
                                    env.deposit(&s);
                                }
                            }
                        }
                    }
                }
            }
            DepositStrategy::Broadcast {
                half_width,
                half_height,
            } => {
                let x_start = x.saturating_sub(*half_width);
                let y_start = y.saturating_sub(*half_height);
                for cy in y_start..=y + half_height {
                    for cx in x_start..=x + half_width {
                        if env.in_bounds(cx, cy) {
                            let s = Signal::new(
                                signal_type,
                                base_strength,
                                cx,
                                cy,
                                timestamp,
                                decay_rate,
                            );
                            env.deposit(&s);
                        }
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fresh_env() -> Environment {
        Environment::new(20, 20)
    }

    #[test]
    fn point_deposit_single_cell() {
        let mut env = fresh_env();
        DepositStrategy::Point.deposit(&mut env, SignalType::Pheromone, 1.0, 5, 5, 0, 0.1);
        assert!((env.read(5, 5, SignalType::Pheromone) - 1.0).abs() < 1e-9);
        // No neighbors affected
        assert_eq!(env.read(5, 4, SignalType::Pheromone), 0.0);
    }

    #[test]
    fn point_deposit_out_of_bounds() {
        let mut env = fresh_env();
        DepositStrategy::Point.deposit(&mut env, SignalType::Pheromone, 1.0, 50, 50, 0, 0.1);
        assert_eq!(env.total_signal(), 0.0);
    }

    #[test]
    fn gradient_deposit_spreads() {
        let mut env = fresh_env();
        DepositStrategy::Gradient { radius: 2 }.deposit(
            &mut env,
            SignalType::Pheromone,
            1.0,
            10,
            10,
            0,
            0.1,
        );
        // Center should be strongest
        let center = env.read(10, 10, SignalType::Pheromone);
        assert!(center > 0.0);
        // Some neighbors should have signal too
        let neighbor = env.read(11, 10, SignalType::Pheromone);
        assert!(neighbor > 0.0);
        assert!(neighbor < center);
    }

    #[test]
    fn gradient_radius_zero_same_as_point() {
        let mut env = fresh_env();
        DepositStrategy::Gradient { radius: 0 }.deposit(
            &mut env,
            SignalType::Marker,
            1.0,
            5,
            5,
            0,
            0.0,
        );
        assert!((env.read(5, 5, SignalType::Marker) - 1.0).abs() < 1e-9);
    }

    #[test]
    fn broadcast_deposit_uniform() {
        let mut env = fresh_env();
        DepositStrategy::Broadcast {
            half_width: 1,
            half_height: 1,
        }
        .deposit(&mut env, SignalType::Barrier, 2.0, 10, 10, 0, 0.1);
        // 3x3 area all get 2.0
        for dy in 0..=2 {
            for dx in 0..=2 {
                assert!(
                    (env.read(9 + dx, 9 + dy, SignalType::Barrier) - 2.0).abs() < 1e-9,
                    "cell ({},{}) should be 2.0",
                    9 + dx,
                    9 + dy
                );
            }
        }
    }

    #[test]
    fn broadcast_clips_at_boundaries() {
        let mut env = fresh_env();
        DepositStrategy::Broadcast {
            half_width: 5,
            half_height: 5,
        }
        .deposit(&mut env, SignalType::Pheromone, 1.0, 0, 0, 0, 0.1);
        // Should not panic; only in-bounds cells get signal
        assert!(env.total_signal() > 0.0);
    }

    #[test]
    fn strategy_serde_roundtrip() {
        let strategies = vec![
            DepositStrategy::Point,
            DepositStrategy::Gradient { radius: 3 },
            DepositStrategy::Broadcast {
                half_width: 2,
                half_height: 4,
            },
        ];
        for s in &strategies {
            let json = serde_json::to_string(s).unwrap();
            let back: DepositStrategy = serde_json::from_str(&json).unwrap();
            assert_eq!(*s, back);
        }
    }
}
