//! Core signal types for stigmergic communication.
//!
//! A `Signal` represents a single marker deposited by an agent into the
//! environment. Each signal carries a type, strength, 2D position, timestamp,
//! and a decay rate governing how quickly it evaporates.

use serde::{Deserialize, Serialize};

/// The kind of signal an agent deposits.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SignalType {
    /// Attractive pheromone — agents tend to move toward stronger concentrations.
    Pheromone,
    /// A neutral marker indicating presence or a waypoint.
    Marker,
    /// A repulsive barrier — agents avoid cells with strong barrier signals.
    Barrier,
}

/// A single signal deposited at a position in the environment.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Signal {
    /// What kind of signal this is.
    pub signal_type: SignalType,
    /// Current strength (≥ 0). Decays over time according to `decay_rate`.
    pub strength: f64,
    /// X coordinate on the environment grid.
    pub x: usize,
    /// Y coordinate on the environment grid.
    pub y: usize,
    /// Logical time step when the signal was deposited.
    pub timestamp: u64,
    /// Fraction of strength lost per time step (0.0 = permanent, 1.0 = instant).
    pub decay_rate: f64,
}

impl Signal {
    /// Create a new signal with all fields specified.
    pub fn new(
        signal_type: SignalType,
        strength: f64,
        x: usize,
        y: usize,
        timestamp: u64,
        decay_rate: f64,
    ) -> Self {
        Self {
            signal_type,
            strength,
            x,
            y,
            timestamp,
            decay_rate,
        }
    }

    /// Convenience constructor for a pheromone signal.
    pub fn pheromone(strength: f64, x: usize, y: usize, timestamp: u64, decay_rate: f64) -> Self {
        Self::new(SignalType::Pheromone, strength, x, y, timestamp, decay_rate)
    }

    /// Convenience constructor for a marker signal.
    pub fn marker(strength: f64, x: usize, y: usize, timestamp: u64, decay_rate: f64) -> Self {
        Self::new(SignalType::Marker, strength, x, y, timestamp, decay_rate)
    }

    /// Convenience constructor for a barrier signal.
    pub fn barrier(strength: f64, x: usize, y: usize, timestamp: u64, decay_rate: f64) -> Self {
        Self::new(SignalType::Barrier, strength, x, y, timestamp, decay_rate)
    }

    /// Compute the age of this signal at the given time step.
    pub fn age(&self, current_time: u64) -> u64 {
        current_time.saturating_sub(self.timestamp)
    }

    /// Compute the remaining strength after `steps` time steps of exponential decay.
    ///
    /// ```text
    /// remaining = strength × (1 - decay_rate)^steps
    /// ```
    pub fn strength_after(&self, steps: u64) -> f64 {
        self.strength * (1.0 - self.decay_rate).powi(steps as i32)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn signal_new() {
        let s = Signal::new(SignalType::Pheromone, 1.0, 5, 3, 0, 0.1);
        assert_eq!(s.signal_type, SignalType::Pheromone);
        assert!((s.strength - 1.0).abs() < f64::EPSILON);
        assert_eq!(s.x, 5);
        assert_eq!(s.y, 3);
        assert_eq!(s.timestamp, 0);
        assert!((s.decay_rate - 0.1).abs() < f64::EPSILON);
    }

    #[test]
    fn convenience_constructors() {
        let p = Signal::pheromone(2.0, 0, 0, 0, 0.05);
        assert_eq!(p.signal_type, SignalType::Pheromone);
        let m = Signal::marker(1.0, 1, 1, 0, 0.0);
        assert_eq!(m.signal_type, SignalType::Marker);
        let b = Signal::barrier(3.0, 2, 2, 0, 0.2);
        assert_eq!(b.signal_type, SignalType::Barrier);
    }

    #[test]
    fn signal_age() {
        let s = Signal::pheromone(1.0, 0, 0, 10, 0.1);
        assert_eq!(s.age(15), 5);
        assert_eq!(s.age(5), 0); // age saturates at 0
    }

    #[test]
    fn strength_decay() {
        let s = Signal::pheromone(1.0, 0, 0, 0, 0.5);
        let after = s.strength_after(1);
        assert!((after - 0.5).abs() < 1e-9);
        let after3 = s.strength_after(3);
        assert!((after3 - 0.125).abs() < 1e-9);
    }

    #[test]
    fn zero_decay_permanent() {
        let s = Signal::pheromone(1.0, 0, 0, 0, 0.0);
        assert!((s.strength_after(1000) - 1.0).abs() < 1e-9);
    }

    #[test]
    fn serialize_deserialize_roundtrip() {
        let s = Signal::pheromone(1.5, 10, 20, 5, 0.1);
        let json = serde_json::to_string(&s).unwrap();
        let back: Signal = serde_json::from_str(&json).unwrap();
        assert_eq!(s, back);
    }

    #[test]
    fn signal_type_serde_keys() {
        let t = SignalType::Pheromone;
        let json = serde_json::to_string(&t).unwrap();
        assert!(json.contains("pheromone"));
    }
}
