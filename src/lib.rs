//! Stigmergic communication for swarm agents with pheromone trails and foraging.
//!
//! Implements indirect communication through environment signals:
//! pheromones with evaporation, trail following, gradient climbing,
//! nest-based signal concentration, and explore/exploit foraging.

use std::collections::HashMap;

// ── Module: pheromone ────────────────────────────────────────────────────

/// A chemical-like signal with evaporation.
#[derive(Debug, Clone)]
pub struct Pheromone {
    pub signal_type: String,
    pub strength: f64,
    pub evaporation_rate: f64, // per step (0.0 to 1.0)
    pub diffusion_rate: f64,   // fraction that spreads to neighbors
}

impl Pheromone {
    /// Create a new pheromone signal.
    pub fn new(signal_type: &str, strength: f64, evaporation_rate: f64) -> Self {
        Self {
            signal_type: signal_type.to_string(),
            strength,
            evaporation_rate: evaporation_rate.clamp(0.0, 1.0),
            diffusion_rate: 0.1,
        }
    }

    /// Deposit more pheromone (increase strength).
    pub fn deposit(&mut self, amount: f64) {
        self.strength += amount;
    }

    /// Evaporate one step.
    pub fn evaporate(&mut self) {
        self.strength *= 1.0 - self.evaporation_rate;
        if self.strength < 0.01 {
            self.strength = 0.0;
        }
    }

    /// Is the pheromone effectively gone?
    pub fn is_gone(&self) -> bool {
        self.strength < 0.01
    }

    /// Age the pheromone by n steps.
    pub fn age(&mut self, steps: usize) {
        for _ in 0..steps {
            self.evaporate();
        }
    }
}

/// A 2D grid of pheromone signals.
#[derive(Debug, Clone)]
pub struct PheromoneGrid {
    width: usize,
    height: usize,
    cells: HashMap<(usize, usize), Vec<Pheromone>>,
}

impl PheromoneGrid {
    /// Create a new empty grid.
    pub fn new(width: usize, height: usize) -> Self {
        Self {
            width,
            height,
            cells: HashMap::new(),
        }
    }

    /// Deposit pheromone at a cell.
    pub fn deposit(&mut self, x: usize, y: usize, pheromone: Pheromone) {
        let cell = self.cells.entry((x, y)).or_default();
        if let Some(existing) = cell.iter_mut().find(|p| p.signal_type == pheromone.signal_type) {
            existing.deposit(pheromone.strength);
        } else {
            cell.push(pheromone);
        }
    }

    /// Get pheromone strength at a cell for a given type.
    pub fn get_strength(&self, x: usize, y: usize, signal_type: &str) -> f64 {
        self.cells
            .get(&(x, y))
            .and_then(|cell| cell.iter().find(|p| p.signal_type == signal_type))
            .map(|p| p.strength)
            .unwrap_or(0.0)
    }

    /// Evaporate all pheromones and remove dead ones.
    pub fn evaporate_all(&mut self) {
        for cell in self.cells.values_mut() {
            for p in cell.iter_mut() {
                p.evaporate();
            }
            cell.retain(|p| !p.is_gone());
        }
        self.cells.retain(|_, cell| !cell.is_empty());
    }

    /// Get all signal types present at a cell.
    pub fn signal_types_at(&self, x: usize, y: usize) -> Vec<String> {
        self.cells
            .get(&(x, y))
            .map(|cell| cell.iter().map(|p| p.signal_type.clone()).collect())
            .unwrap_or_default()
    }

    /// Total pheromone strength across the grid.
    pub fn total_strength(&self) -> f64 {
        self.cells
            .values()
            .flat_map(|cell| cell.iter())
            .map(|p| p.strength)
            .sum()
    }

    /// Number of cells with active pheromone.
    pub fn active_cells(&self) -> usize {
        self.cells.len()
    }

    /// Grid dimensions.
    pub fn dimensions(&self) -> (usize, usize) {
        (self.width, self.height)
    }

    /// Diffuse pheromones to neighboring cells.
    pub fn diffuse(&mut self) {
        let mut deposits: Vec<(usize, usize, Pheromone)> = Vec::new();
        for (&(x, y), cell) in &self.cells {
            for p in cell {
                let diffused_amount = p.strength * p.diffusion_rate;
                if diffused_amount < 0.01 {
                    continue;
                }
                let neighbors = [
                    (x.wrapping_sub(1), y),
                    ((x + 1).min(self.width - 1), y),
                    (x, y.wrapping_sub(1)),
                    (x, (y + 1).min(self.height - 1)),
                ];
                for &(nx, ny) in &neighbors {
                    if nx < self.width && ny < self.height {
                        deposits.push((
                            nx,
                            ny,
                            Pheromone::new(&p.signal_type, diffused_amount / 4.0, p.evaporation_rate),
                        ));
                    }
                }
            }
        }
        for (x, y, p) in deposits {
            self.deposit(x, y, p);
        }
    }
}

// ── Module: trail ────────────────────────────────────────────────────────

/// A trail of pheromone signals left by an agent.
#[derive(Debug, Clone)]
pub struct Trail {
    pub path: Vec<(usize, usize)>,
    pub signal_type: String,
    pub strength: f64,
}

impl Trail {
    /// Create a new trail.
    pub fn new(signal_type: &str, strength: f64) -> Self {
        Self {
            path: Vec::new(),
            signal_type: signal_type.to_string(),
            strength,
        }
    }

    /// Extend the trail by one step.
    pub fn extend(&mut self, x: usize, y: usize) {
        self.path.push((x, y));
    }

    /// Length of the trail.
    pub fn len(&self) -> usize {
        self.path.len()
    }

    /// Is the trail empty?
    pub fn is_empty(&self) -> bool {
        self.path.is_empty()
    }

    /// Deposit the trail onto a pheromone grid.
    pub fn deposit_on_grid(&self, grid: &mut PheromoneGrid) {
        for &(x, y) in &self.path {
            grid.deposit(x, y, Pheromone::new(&self.signal_type, self.strength, 0.05));
        }
    }

    /// Follow a trail on the grid: given current position, find the next step
    /// that maximizes the signal.
    pub fn follow(grid: &PheromoneGrid, x: usize, y: usize, signal_type: &str) -> Option<(usize, usize)> {
        let neighbors = [
            (x.wrapping_sub(1), y),
            ((x + 1), y),
            (x, y.wrapping_sub(1)),
            (x, y + 1),
        ];
        let mut best: Option<(usize, usize, f64)> = None;
        for (nx, ny) in neighbors {
            let strength = grid.get_strength(nx, ny, signal_type);
            if strength > best.as_ref().map(|b| b.2).unwrap_or(0.0) {
                best = Some((nx, ny, strength));
            }
        }
        best.map(|(x, y, _)| (x, y))
    }

    /// Compute trail strength (decays with distance from end).
    pub fn strength_at(&self, index: usize) -> f64 {
        if index >= self.path.len() {
            return 0.0;
        }
        let distance_from_end = self.path.len() - 1 - index;
        self.strength * 0.9_f64.powi(distance_from_end as i32)
    }

    /// Merge another trail into this one.
    pub fn merge(&mut self, other: &Trail) {
        for &(x, y) in &other.path {
            if !self.path.contains(&(x, y)) {
                self.path.push((x, y));
            }
        }
    }
}

// ── Module: gradient ─────────────────────────────────────────────────────

/// Signal gradient climber: move toward increasing signal strength.
pub struct GradientClimber {
    pub x: usize,
    pub y: usize,
    pub signal_type: String,
}

impl GradientClimber {
    /// Create a new climber at a position.
    pub fn new(x: usize, y: usize, signal_type: &str) -> Self {
        Self {
            x,
            y,
            signal_type: signal_type.to_string(),
        }
    }

    /// Climb one step toward the gradient.
    /// Returns the new position.
    pub fn climb(&mut self, grid: &PheromoneGrid) -> (usize, usize) {
        let current = grid.get_strength(self.x, self.y, &self.signal_type);
        let candidates = [
            (self.x.wrapping_sub(1), self.y),
            (self.x + 1, self.y),
            (self.x, self.y.wrapping_sub(1)),
            (self.x, self.y + 1),
        ];
        let mut best_pos = (self.x, self.y);
        let mut best_strength = current;
        for (nx, ny) in candidates {
            let s = grid.get_strength(nx, ny, &self.signal_type);
            if s > best_strength {
                best_strength = s;
                best_pos = (nx, ny);
            }
        }
        self.x = best_pos.0;
        self.y = best_pos.1;
        (self.x, self.y)
    }

    /// Current position.
    pub fn position(&self) -> (usize, usize) {
        (self.x, self.y)
    }

    /// Read signal strength at current position.
    pub fn sense(&self, grid: &PheromoneGrid) -> f64 {
        grid.get_strength(self.x, self.y, &self.signal_type)
    }

    /// Climb multiple steps.
    pub fn climb_n(&mut self, grid: &PheromoneGrid, steps: usize) -> Vec<(usize, usize)> {
        let mut positions = Vec::new();
        for _ in 0..steps {
            let pos = self.climb(grid);
            positions.push(pos);
        }
        positions
    }
}

/// Compute the gradient direction at a point.
pub fn gradient_direction(grid: &PheromoneGrid, x: usize, y: usize, signal_type: &str) -> (i32, i32) {
    let _center = grid.get_strength(x, y, signal_type);
    let left = grid.get_strength(x.wrapping_sub(1), y, signal_type);
    let right = grid.get_strength(x + 1, y, signal_type);
    let down = grid.get_strength(x, y.wrapping_sub(1), signal_type);
    let up = grid.get_strength(x, y + 1, signal_type);

    let dx = if right > left { 1 } else if left > right { -1 } else { 0 };
    let dy = if up > down { 1 } else if down > up { -1 } else { 0 };
    (dx, dy)
}

/// Compute gradient magnitude at a point.
pub fn gradient_magnitude(grid: &PheromoneGrid, x: usize, y: usize, signal_type: &str) -> f64 {
    let left = grid.get_strength(x.wrapping_sub(1), y, signal_type);
    let right = grid.get_strength(x + 1, y, signal_type);
    let down = grid.get_strength(x, y.wrapping_sub(1), signal_type);
    let up = grid.get_strength(x, y + 1, signal_type);
    let _center = grid.get_strength(x, y, signal_type);

    let dx = (right - left) / 2.0;
    let dy = (up - down) / 2.0;
    (dx * dx + dy * dy).sqrt()
}

// ── Module: nest ─────────────────────────────────────────────────────────

/// Home base with signal concentration.
#[derive(Debug, Clone)]
pub struct Nest {
    pub id: String,
    pub x: usize,
    pub y: usize,
    pub signal_type: String,
    pub base_strength: f64,
    pub radius: usize,
    pub agents_home: usize,
}

impl Nest {
    /// Create a new nest.
    pub fn new(id: &str, x: usize, y: usize, signal_type: &str, base_strength: f64) -> Self {
        Self {
            id: id.to_string(),
            x,
            y,
            signal_type: signal_type.to_string(),
            base_strength,
            radius: 5,
            agents_home: 0,
        }
    }

    /// Emit pheromone signals around the nest.
    pub fn emit(&self, grid: &mut PheromoneGrid) {
        for dx in -(self.radius as i32)..=(self.radius as i32) {
            for dy in -(self.radius as i32)..=(self.radius as i32) {
                let dist = ((dx * dx + dy * dy) as f64).sqrt();
                if dist <= self.radius as f64 {
                    let nx = (self.x as i32 + dx) as usize;
                    let ny = (self.y as i32 + dy) as usize;
                    let strength = self.base_strength * (1.0 - dist / (self.radius as f64 + 1.0));
                    grid.deposit(nx, ny, Pheromone::new(&self.signal_type, strength, 0.02));
                }
            }
        }
    }

    /// Distance from a point to the nest.
    pub fn distance_to(&self, x: usize, y: usize) -> f64 {
        let dx = self.x as f64 - x as f64;
        let dy = self.y as f64 - y as f64;
        (dx * dx + dy * dy).sqrt()
    }

    /// Is a point within the nest radius?
    pub fn is_within(&self, x: usize, y: usize) -> bool {
        self.distance_to(x, y) <= self.radius as f64
    }

    /// An agent arrives home.
    pub fn agent_arrive(&mut self) {
        self.agents_home += 1;
    }

    /// An agent leaves.
    pub fn agent_leave(&mut self) {
        if self.agents_home > 0 {
            self.agents_home -= 1;
        }
    }

    /// Number of agents at home.
    pub fn agents_at_home(&self) -> usize {
        self.agents_home
    }
}

// ── Module: foraging ─────────────────────────────────────────────────────

/// An agent that forages using pheromone signals.
#[derive(Debug, Clone)]
pub struct Forager {
    pub id: String,
    pub x: usize,
    pub y: usize,
    pub carrying: bool,
    pub explore_bias: f64, // 0.0 = pure exploit, 1.0 = pure explore
    pub trail: Trail,
    pub steps_taken: usize,
}

impl Forager {
    /// Create a new forager.
    pub fn new(id: &str, x: usize, y: usize, explore_bias: f64) -> Self {
        Self {
            id: id.to_string(),
            x,
            y,
            carrying: false,
            explore_bias,
            trail: Trail::new("forage", 1.0),
            steps_taken: 0,
        }
    }

    /// Choose next position based on signal strength and exploration bias.
    pub fn choose_step(&self, grid: &PheromoneGrid, signal_type: &str, rng_val: f64) -> (usize, usize) {
        let candidates = [
            (self.x.wrapping_sub(1), self.y),
            (self.x + 1, self.y),
            (self.x, self.y.wrapping_sub(1)),
            (self.x, self.y + 1),
        ];

        if rng_val < self.explore_bias {
            // Explore: random direction
            let idx = (rng_val * 4.0) as usize % 4;
            candidates[idx]
        } else {
            // Exploit: follow signal
            let mut best = (self.x, self.y);
            let mut best_strength = grid.get_strength(self.x, self.y, signal_type);
            for &(nx, ny) in &candidates {
                let s = grid.get_strength(nx, ny, signal_type);
                if s > best_strength {
                    best_strength = s;
                    best = (nx, ny);
                }
            }
            best
        }
    }

    /// Take a step.
    pub fn step(&mut self, grid: &PheromoneGrid, signal_type: &str, rng_val: f64) {
        let (nx, ny) = self.choose_step(grid, signal_type, rng_val);
        self.x = nx;
        self.y = ny;
        self.trail.extend(nx, ny);
        self.steps_taken += 1;
    }

    /// Pick up food at current location.
    pub fn pickup(&mut self, food_grid: &PheromoneGrid) -> bool {
        if !self.carrying && food_grid.get_strength(self.x, self.y, "food") > 0.0 {
            self.carrying = true;
            return true;
        }
        false
    }

    /// Drop food at current location.
    pub fn drop(&mut self) -> bool {
        if self.carrying {
            self.carrying = false;
            return true;
        }
        false
    }

    /// Current position.
    pub fn position(&self) -> (usize, usize) {
        (self.x, self.y)
    }
}

/// Run a simple foraging simulation step.
pub fn foraging_step(
    foragers: &mut [Forager],
    grid: &mut PheromoneGrid,
    food_grid: &PheromoneGrid,
    nest: &mut Nest,
    signal_type: &str,
) -> ForagingStats {
    let mut food_collected = 0;
    let mut food_delivered = 0;

    for forager in foragers.iter_mut() {
        let rng_val = 0.5; // simplified deterministic
        if forager.carrying {
            // Head home
            let dist = nest.distance_to(forager.x, forager.y);
            if dist < 2.0 {
                if forager.drop() {
                    food_delivered += 1;
                    nest.agent_arrive();
                }
            } else {
                forager.step(grid, signal_type, rng_val);
            }
        } else {
            forager.step(grid, signal_type, rng_val);
            if forager.pickup(food_grid) {
                food_collected += 1;
            }
        }
    }

    grid.evaporate_all();

    ForagingStats {
        food_collected,
        food_delivered,
        active_foragers: foragers.len(),
    }
}

/// Statistics from a foraging step.
#[derive(Debug, Clone)]
pub struct ForagingStats {
    pub food_collected: usize,
    pub food_delivered: usize,
    pub active_foragers: usize,
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── Pheromone tests ──

    #[test]
    fn test_pheromone_new() {
        let p = Pheromone::new("food", 10.0, 0.1);
        assert_eq!(p.signal_type, "food");
        assert!((p.strength - 10.0).abs() < 1e-10);
    }

    #[test]
    fn test_pheromone_deposit() {
        let mut p = Pheromone::new("food", 5.0, 0.1);
        p.deposit(3.0);
        assert!((p.strength - 8.0).abs() < 1e-10);
    }

    #[test]
    fn test_pheromone_evaporate() {
        let mut p = Pheromone::new("food", 10.0, 0.5);
        p.evaporate();
        assert!((p.strength - 5.0).abs() < 1e-10);
    }

    #[test]
    fn test_pheromone_is_gone() {
        let p = Pheromone::new("food", 0.005, 0.5);
        assert!(p.is_gone());
    }

    #[test]
    fn test_pheromone_not_gone() {
        let p = Pheromone::new("food", 1.0, 0.1);
        assert!(!p.is_gone());
    }

    #[test]
    fn test_pheromone_age() {
        let mut p = Pheromone::new("food", 100.0, 0.5);
        p.age(3);
        // 100 * 0.5^3 = 12.5
        assert!((p.strength - 12.5).abs() < 1.0);
    }

    #[test]
    fn test_pheromone_evaporation_clamp() {
        let p = Pheromone::new("food", 1.0, 2.0); // rate > 1.0
        assert!((p.evaporation_rate - 1.0).abs() < 1e-10); // clamped to 1.0
    }

    // ── PheromoneGrid tests ──

    #[test]
    fn test_grid_new() {
        let g = PheromoneGrid::new(10, 10);
        assert_eq!(g.dimensions(), (10, 10));
        assert_eq!(g.active_cells(), 0);
    }

    #[test]
    fn test_grid_deposit_get() {
        let mut g = PheromoneGrid::new(10, 10);
        g.deposit(5, 5, Pheromone::new("food", 10.0, 0.1));
        assert!((g.get_strength(5, 5, "food") - 10.0).abs() < 1e-10);
    }

    #[test]
    fn test_grid_deposit_accumulate() {
        let mut g = PheromoneGrid::new(10, 10);
        g.deposit(5, 5, Pheromone::new("food", 5.0, 0.1));
        g.deposit(5, 5, Pheromone::new("food", 3.0, 0.1));
        assert!((g.get_strength(5, 5, "food") - 8.0).abs() < 1e-10);
    }

    #[test]
    fn test_grid_evaporate_all() {
        let mut g = PheromoneGrid::new(10, 10);
        g.deposit(5, 5, Pheromone::new("food", 1.0, 0.99));
        g.evaporate_all();
        // Very high evaporation should nearly eliminate it
        assert!(g.get_strength(5, 5, "food") < 0.5);
    }

    #[test]
    fn test_grid_total_strength() {
        let mut g = PheromoneGrid::new(10, 10);
        g.deposit(1, 1, Pheromone::new("food", 5.0, 0.1));
        g.deposit(2, 2, Pheromone::new("food", 3.0, 0.1));
        assert!((g.total_strength() - 8.0).abs() < 1e-10);
    }

    #[test]
    fn test_grid_signal_types() {
        let mut g = PheromoneGrid::new(10, 10);
        g.deposit(5, 5, Pheromone::new("food", 1.0, 0.1));
        g.deposit(5, 5, Pheromone::new("danger", 2.0, 0.1));
        let types = g.signal_types_at(5, 5);
        assert_eq!(types.len(), 2);
    }

    #[test]
    fn test_grid_diffuse() {
        let mut g = PheromoneGrid::new(10, 10);
        g.deposit(5, 5, Pheromone::new("food", 100.0, 0.01));
        g.diffuse();
        // Should spread to neighbors
        assert!(g.get_strength(6, 5, "food") > 0.0);
    }

    #[test]
    fn test_grid_empty_cell_strength() {
        let g = PheromoneGrid::new(10, 10);
        assert_eq!(g.get_strength(5, 5, "food"), 0.0);
    }

    // ── Trail tests ──

    #[test]
    fn test_trail_new() {
        let t = Trail::new("food", 1.0);
        assert!(t.is_empty());
        assert_eq!(t.signal_type, "food");
    }

    #[test]
    fn test_trail_extend() {
        let mut t = Trail::new("food", 1.0);
        t.extend(1, 2);
        t.extend(2, 2);
        assert_eq!(t.len(), 2);
    }

    #[test]
    fn test_trail_deposit_on_grid() {
        let mut t = Trail::new("food", 5.0);
        t.extend(3, 3);
        t.extend(4, 3);
        let mut g = PheromoneGrid::new(10, 10);
        t.deposit_on_grid(&mut g);
        assert!(g.get_strength(3, 3, "food") > 0.0);
        assert!(g.get_strength(4, 3, "food") > 0.0);
    }

    #[test]
    fn test_trail_strength_at() {
        let mut t = Trail::new("food", 10.0);
        t.extend(0, 0);
        t.extend(1, 0);
        t.extend(2, 0);
        assert!(t.strength_at(2) > t.strength_at(0)); // end is stronger
    }

    #[test]
    fn test_trail_strength_out_of_bounds() {
        let t = Trail::new("food", 1.0);
        assert_eq!(t.strength_at(0), 0.0);
    }

    #[test]
    fn test_trail_merge() {
        let mut t1 = Trail::new("food", 1.0);
        t1.extend(1, 1);
        t1.extend(2, 2);
        let mut t2 = Trail::new("food", 1.0);
        t2.extend(3, 3);
        t2.extend(1, 1); // overlap
        t1.merge(&t2);
        assert_eq!(t1.len(), 3); // deduplicated
    }

    #[test]
    fn test_trail_follow() {
        let mut g = PheromoneGrid::new(10, 10);
        g.deposit(6, 5, Pheromone::new("food", 10.0, 0.1));
        let next = Trail::follow(&g, 5, 5, "food");
        assert_eq!(next, Some((6, 5)));
    }

    // ── Gradient tests ──

    #[test]
    fn test_climber_new() {
        let c = GradientClimber::new(5, 5, "food");
        assert_eq!(c.position(), (5, 5));
    }

    #[test]
    fn test_climber_climb() {
        let mut g = PheromoneGrid::new(10, 10);
        g.deposit(7, 5, Pheromone::new("food", 10.0, 0.1));
        let mut c = GradientClimber::new(5, 5, "food");
        c.climb(&g);
        assert!(c.x >= 5); // should move toward 7
    }

    #[test]
    fn test_climber_sense() {
        let mut g = PheromoneGrid::new(10, 10);
        g.deposit(5, 5, Pheromone::new("food", 7.0, 0.1));
        let c = GradientClimber::new(5, 5, "food");
        assert!((c.sense(&g) - 7.0).abs() < 1e-10);
    }

    #[test]
    fn test_climber_climb_n() {
        let mut g = PheromoneGrid::new(20, 20);
        for i in 5..15 {
            g.deposit(i, 10, Pheromone::new("food", i as f64, 0.01));
        }
        let mut c = GradientClimber::new(5, 10, "food");
        let positions = c.climb_n(&g, 5);
        assert_eq!(positions.len(), 5);
    }

    #[test]
    fn test_gradient_direction() {
        let mut g = PheromoneGrid::new(10, 10);
        g.deposit(6, 5, Pheromone::new("food", 10.0, 0.1));
        let (dx, dy) = gradient_direction(&g, 5, 5, "food");
        assert_eq!(dx, 1); // toward higher x
        assert_eq!(dy, 0);
    }

    #[test]
    fn test_gradient_magnitude() {
        let g = PheromoneGrid::new(10, 10);
        let m = gradient_magnitude(&g, 5, 5, "food");
        assert!(m.abs() < 1e-10); // no signal, zero gradient
    }

    // ── Nest tests ──

    #[test]
    fn test_nest_new() {
        let n = Nest::new("home", 5, 5, "home", 100.0);
        assert_eq!(n.id, "home");
        assert_eq!(n.agents_at_home(), 0);
    }

    #[test]
    fn test_nest_emit() {
        let mut g = PheromoneGrid::new(20, 20);
        let n = Nest::new("home", 10, 10, "home", 50.0);
        n.emit(&mut g);
        assert!(g.get_strength(10, 10, "home") > 0.0);
    }

    #[test]
    fn test_nest_distance() {
        let n = Nest::new("home", 0, 0, "home", 1.0);
        assert!((n.distance_to(3, 4) - 5.0).abs() < 1e-10);
    }

    #[test]
    fn test_nest_within() {
        let n = Nest::new("home", 5, 5, "home", 1.0);
        assert!(n.is_within(5, 5));
        assert!(n.is_within(7, 5));
        assert!(!n.is_within(20, 20));
    }

    #[test]
    fn test_nest_agents() {
        let mut n = Nest::new("home", 5, 5, "home", 1.0);
        n.agent_arrive();
        n.agent_arrive();
        assert_eq!(n.agents_at_home(), 2);
        n.agent_leave();
        assert_eq!(n.agents_at_home(), 1);
    }

    #[test]
    fn test_nest_agent_leave_empty() {
        let mut n = Nest::new("home", 5, 5, "home", 1.0);
        n.agent_leave(); // shouldn't go negative
        assert_eq!(n.agents_at_home(), 0);
    }

    // ── Forager tests ──

    #[test]
    fn test_forager_new() {
        let f = Forager::new("ant1", 5, 5, 0.3);
        assert_eq!(f.id, "ant1");
        assert!(!f.carrying);
        assert_eq!(f.steps_taken, 0);
    }

    #[test]
    fn test_forager_step() {
        let f = Forager::new("ant1", 5, 5, 0.0);
        let mut f = f;
        let grid = PheromoneGrid::new(10, 10);
        f.step(&grid, "food", 0.5);
        assert_eq!(f.steps_taken, 1);
    }

    #[test]
    fn test_forager_pickup() {
        let mut food = PheromoneGrid::new(10, 10);
        food.deposit(5, 5, Pheromone::new("food", 1.0, 0.1));
        let mut f = Forager::new("ant1", 5, 5, 0.0);
        assert!(f.pickup(&food));
        assert!(f.carrying);
    }

    #[test]
    fn test_forager_pickup_no_food() {
        let food = PheromoneGrid::new(10, 10);
        let mut f = Forager::new("ant1", 5, 5, 0.0);
        assert!(!f.pickup(&food));
    }

    #[test]
    fn test_forager_pickup_already_carrying() {
        let mut food = PheromoneGrid::new(10, 10);
        food.deposit(5, 5, Pheromone::new("food", 1.0, 0.1));
        let mut f = Forager::new("ant1", 5, 5, 0.0);
        f.carrying = true;
        assert!(!f.pickup(&food));
    }

    #[test]
    fn test_forager_drop() {
        let mut f = Forager::new("ant1", 5, 5, 0.0);
        f.carrying = true;
        assert!(f.drop());
        assert!(!f.carrying);
    }

    #[test]
    fn test_forager_drop_not_carrying() {
        let mut f = Forager::new("ant1", 5, 5, 0.0);
        assert!(!f.drop());
    }

    #[test]
    fn test_foraging_step_basic() {
        let mut foragers = vec![Forager::new("ant1", 5, 5, 0.0)];
        let mut grid = PheromoneGrid::new(10, 10);
        let mut food = PheromoneGrid::new(10, 10);
        let mut nest = Nest::new("home", 5, 5, "home", 10.0);
        let stats = foraging_step(&mut foragers, &mut grid, &mut food, &mut nest, "food");
        assert_eq!(stats.active_foragers, 1);
    }

    #[test]
    fn test_forager_explore_bias() {
        let f = Forager::new("ant1", 5, 5, 1.0); // pure explore
        let grid = PheromoneGrid::new(10, 10);
        let (nx, ny) = f.choose_step(&grid, "food", 0.5);
        // With explore_bias=1.0, it should pick a candidate
        assert!(nx < 20 && ny < 20);
    }

    #[test]
    fn test_forager_position() {
        let f = Forager::new("ant1", 3, 7, 0.0);
        assert_eq!(f.position(), (3, 7));
    }

    #[test]
    fn test_forager_trail() {
        let mut f = Forager::new("ant1", 5, 5, 0.0);
        let grid = PheromoneGrid::new(10, 10);
        f.step(&grid, "food", 0.5);
        f.step(&grid, "food", 0.5);
        assert_eq!(f.trail.len(), 2);
    }
}
