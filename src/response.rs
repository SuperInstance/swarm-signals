//! Response functions — how agents react to detected signals.
//!
//! Agents sample the environment and decide how to move or act based on
//! signal patterns. The `ResponseFunction` enum captures common policies:
//!
//! - **FollowStrongest**: move toward the strongest signal of a given type
//! - **Avoid**: move away from cells with strong signal
//! - **Aggregate**: move to the center of mass of signal strength
//! - **ThresholdTrigger**: act only when signal exceeds a threshold

use serde::{Deserialize, Serialize};

use crate::environment::Environment;
use crate::signal::SignalType;

/// A direction an agent might take, expressed as a (dx, dy) offset.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Direction {
    pub dx: i32,
    pub dy: i32,
}

impl Direction {
    pub fn new(dx: i32, dy: i32) -> Self {
        Self { dx, dy }
    }

    /// Apply this direction to a position, returning None if out of bounds.
    pub fn apply(&self, x: usize, y: usize, width: usize, height: usize) -> Option<(usize, usize)> {
        let nx = x as i32 + self.dx;
        let ny = y as i32 + self.dy;
        if nx >= 0 && ny >= 0 && (nx as usize) < width && (ny as usize) < height {
            Some((nx as usize, ny as usize))
        } else {
            None
        }
    }
}

/// How an agent responds to signals in its local neighborhood.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "response", rename_all = "snake_case")]
pub enum ResponseFunction {
    /// Move toward the neighboring cell with the strongest signal.
    FollowStrongest,
    /// Move away from the neighboring cell with the strongest signal.
    Avoid,
    /// Move toward the weighted centroid of signals in the neighborhood.
    Aggregate {
        /// Radius to consider for aggregation.
        radius: usize,
    },
    /// Only respond when the local signal exceeds a threshold.
    ThresholdTrigger {
        /// The threshold to trigger a response.
        threshold: f64,
        /// Direction to move when triggered.
        direction: Direction,
    },
}

impl ResponseFunction {
    /// Evaluate the response function, returning a suggested move direction.
    ///
    /// Returns `None` if no response is warranted (e.g., no signals nearby,
    /// or threshold not met).
    pub fn evaluate(
        &self,
        env: &Environment,
        x: usize,
        y: usize,
        signal_type: SignalType,
    ) -> Option<Direction> {
        match self {
            ResponseFunction::FollowStrongest => follow_strongest(env, x, y, signal_type, false),
            ResponseFunction::Avoid => follow_strongest(env, x, y, signal_type, false)
                .map(|d| Direction::new(-d.dx, -d.dy)),
            ResponseFunction::Aggregate { radius } => {
                aggregate_toward(env, x, y, signal_type, *radius)
            }
            ResponseFunction::ThresholdTrigger {
                threshold,
                direction,
            } => {
                let val = env.read(x, y, signal_type);
                if val >= *threshold {
                    Some(*direction)
                } else {
                    None
                }
            }
        }
    }
}

/// Find the direction toward (invert=false) or away from (invert=true) the
/// strongest neighboring cell.
fn follow_strongest(
    env: &Environment,
    x: usize,
    y: usize,
    signal_type: SignalType,
    invert: bool,
) -> Option<Direction> {
    let neighbors = env.neighbors4(x, y);
    let mut best_dir: Option<Direction> = None;
    let mut best_val: f64 = f64::MIN;

    for &(nx, ny) in &neighbors {
        let dx = nx as i32 - x as i32;
        let dy = ny as i32 - y as i32;
        let val = env.read(nx, ny, signal_type);
        if val > best_val {
            best_val = val;
            best_dir = Some(Direction::new(dx, dy));
        }
    }

    if best_val.abs() < f64::EPSILON {
        None
    } else if invert {
        best_dir.map(|d| Direction::new(-d.dx, -d.dy))
    } else {
        best_dir
    }
}

/// Move toward the center of mass of signal within a radius.
fn aggregate_toward(
    env: &Environment,
    x: usize,
    y: usize,
    signal_type: SignalType,
    radius: usize,
) -> Option<Direction> {
    let r = radius as i32;
    let xi = x as i32;
    let yi = y as i32;

    let mut total_weight = 0.0;
    let mut weighted_x = 0.0;
    let mut weighted_y = 0.0;

    for dy in -r..=r {
        for dx in -r..=r {
            let nx = xi + dx;
            let ny = yi + dy;
            if nx >= 0 && ny >= 0 {
                let ux = nx as usize;
                let uy = ny as usize;
                if env.in_bounds(ux, uy) {
                    let val = env.read(ux, uy, signal_type);
                    if val > 0.0 {
                        total_weight += val;
                        weighted_x += val * nx as f64;
                        weighted_y += val * ny as f64;
                    }
                }
            }
        }
    }

    if total_weight.abs() < f64::EPSILON {
        return None;
    }

    let cx = weighted_x / total_weight;
    let cy = weighted_y / total_weight;

    let diff_x = cx - xi as f64;
    let diff_y = cy - yi as f64;

    let dir_x = if diff_x.abs() < f64::EPSILON {
        0
    } else {
        diff_x.signum() as i32
    };
    let dir_y = if diff_y.abs() < f64::EPSILON {
        0
    } else {
        diff_y.signum() as i32
    };

    if dir_x == 0 && dir_y == 0 {
        None
    } else {
        Some(Direction::new(dir_x, dir_y))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn env_with_gradient() -> Environment {
        let mut env = Environment::new(10, 10);
        // Gradient: stronger to the right
        env.set(7, 5, SignalType::Pheromone, 3.0);
        env.set(6, 5, SignalType::Pheromone, 2.0);
        env.set(5, 5, SignalType::Pheromone, 1.0);
        env
    }

    #[test]
    fn follow_strongest_picks_right() {
        let env = env_with_gradient();
        let resp = ResponseFunction::FollowStrongest;
        let dir = resp.evaluate(&env, 5, 5, SignalType::Pheromone).unwrap();
        assert_eq!(dir.dx, 1);
        assert_eq!(dir.dy, 0);
    }

    #[test]
    fn follow_strongest_none_when_no_signal() {
        let env = Environment::new(5, 5);
        let resp = ResponseFunction::FollowStrongest;
        assert!(resp.evaluate(&env, 2, 2, SignalType::Pheromone).is_none());
    }

    #[test]
    fn avoid_moves_away() {
        let env = env_with_gradient();
        let resp = ResponseFunction::Avoid;
        let dir = resp.evaluate(&env, 5, 5, SignalType::Pheromone).unwrap();
        assert_eq!(dir.dx, -1); // away from strongest (right)
    }

    #[test]
    fn aggregate_toward_center() {
        let env = env_with_gradient();
        let resp = ResponseFunction::Aggregate { radius: 5 };
        let dir = resp.evaluate(&env, 3, 5, SignalType::Pheromone).unwrap();
        assert_eq!(dir.dx, 1); // toward weighted centroid
    }

    #[test]
    fn aggregate_none_when_at_center() {
        let mut env = Environment::new(10, 10);
        // Place signal symmetrically so centroid IS at (5,5)
        env.set(4, 5, SignalType::Pheromone, 1.0);
        env.set(6, 5, SignalType::Pheromone, 1.0);
        env.set(5, 4, SignalType::Pheromone, 1.0);
        env.set(5, 6, SignalType::Pheromone, 1.0);
        let resp = ResponseFunction::Aggregate { radius: 3 };
        let result = resp.evaluate(&env, 5, 5, SignalType::Pheromone);
        assert!(result.is_none());
    }

    #[test]
    fn threshold_trigger_fires() {
        let mut env = Environment::new(5, 5);
        env.set(2, 2, SignalType::Pheromone, 1.5);
        let resp = ResponseFunction::ThresholdTrigger {
            threshold: 1.0,
            direction: Direction::new(1, 0),
        };
        let dir = resp.evaluate(&env, 2, 2, SignalType::Pheromone).unwrap();
        assert_eq!(dir.dx, 1);
    }

    #[test]
    fn threshold_trigger_silent_below() {
        let mut env = Environment::new(5, 5);
        env.set(2, 2, SignalType::Pheromone, 0.5);
        let resp = ResponseFunction::ThresholdTrigger {
            threshold: 1.0,
            direction: Direction::new(1, 0),
        };
        assert!(resp.evaluate(&env, 2, 2, SignalType::Pheromone).is_none());
    }

    #[test]
    fn direction_apply_in_bounds() {
        let d = Direction::new(1, 0);
        assert_eq!(d.apply(0, 0, 5, 5), Some((1, 0)));
    }

    #[test]
    fn direction_apply_out_of_bounds() {
        let d = Direction::new(-1, 0);
        assert_eq!(d.apply(0, 0, 5, 5), None);
    }

    #[test]
    fn response_function_serde_roundtrip() {
        let fns = vec![
            ResponseFunction::FollowStrongest,
            ResponseFunction::Avoid,
            ResponseFunction::Aggregate { radius: 3 },
            ResponseFunction::ThresholdTrigger {
                threshold: 0.5,
                direction: Direction::new(0, 1),
            },
        ];
        for f in &fns {
            let json = serde_json::to_string(f).unwrap();
            let back: ResponseFunction = serde_json::from_str(&json).unwrap();
            assert_eq!(*f, back);
        }
    }
}
