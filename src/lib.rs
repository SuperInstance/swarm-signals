//! # swarm-signals
//!
//! Stigmergy-based coordination — agents communicate through environment
//! modifications, not direct messages.
//!
//! Inspired by ant colony pheromone trails, this crate provides a 2D
//! environment where agents deposit signals that decay, diffuse, and trigger
//! responses — enabling emergent coordination without any central controller.

pub mod decay;
pub mod deposit;
pub mod environment;
pub mod response;
pub mod signal;
pub mod trail;
