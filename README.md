# swarm-signals

**Stigmergy-based coordination for Rust — agents communicate through environment modifications, not direct messages.**

---

## Why This Crate Exists

Coordinating autonomous agents is hard. Traditional approaches — message passing, shared state, consensus protocols — introduce coupling, bottlenecks, and single points of failure. Nature solved this problem millions of years ago.

Ant colonies build complex structures, find shortest paths, and allocate resources with no central controller and no direct communication between workers. They use **stigmergy**: agents modify a shared environment, and those modifications influence the behavior of other agents who encounter them. An ant doesn't tell another ant where the food is. It leaves a pheromone trail. The trail *is* the message.

`swarm-signals` brings this pattern to Rust. It provides a 2D environment where agents deposit typed signals (pheromones, markers, barriers) that decay, diffuse, and trigger responses. Trails emerge. Agents coordinate. No messages. No orchestration. Just the physics of the environment doing the work.

This crate is designed for simulations, games, multi-agent systems, and anywhere you need emergent coordination without a central planner.

---

## The Metaphor: Stigmergy and Ant Colonies

### What is Stigmergy?

Stigmergy (from Greek *stigma* "mark" + *ergon* "work") is a mechanism of indirect coordination through the environment. The concept was coined by French biologist Pierre-Paul Grassé in 1959 to explain how termites build elaborate mounds without any termite "knowing" the full design.

### How Ant Colonies Use It

1. **A scout ant discovers food** and begins returning to the nest.
2. **It deposits pheromone** along its path — a chemical signal.
3. **Other ants encounter the pheromone** and probabilistically follow it.
4. **Shorter paths accumulate more pheromone** because ants traverse them faster (reinforcement).
5. **Longer paths evaporate** as pheromone decays before being reinforced.
6. **The colony converges on the shortest path** — with no ant "deciding" this globally.

This is **emergent intelligence**: global optimization from local, simple rules. No ant knows the shortest path. The trail *discovers* it.

### Why It Matters for Software

| Centralized Coordination | Stigmergic Coordination |
|---|---|
| Single point of failure | No central controller |
| O(n²) communication | O(n) communication via environment |
| Requires shared knowledge | Agents only read local environment |
| Hard to scale | Naturally scales with grid size |
| Brittle topology changes | Self-healing through decay/deposit |

---

## Architecture

```
                    ┌─────────────────────────────────────────┐
                    │           swarm-signals                  │
                    │                                         │
  ┌─────────┐  deposit   ┌─────────────┐   read    ┌──────────────┐
  │ signal   │──────────▶│ environment │◀─────────│  response     │
  │          │           │             │           │              │
  │ SignalType│          │ 2D Grid     │           │ FollowStrong │
  │ strength  │          │ aggregation │           │ Avoid        │
  │ position  │          │ neighbors   │           │ Aggregate    │
  │ timestamp │          └──────┬──────┘           │ Threshold    │
  │ decay_rate│                 │                  └──────────────┘
  └─────────┘                  │                         ▲
                               │ step                     │
                      ┌────────▼────────┐                │
                      │      decay       │                │
                      │                  │         follow trail
                      │  evaporate       │                │
                      │  diffuse         │        ┌───────┴────────┐
                      │  prune           │        │     trail       │
                      └──────────────────┘        │                 │
                                                  │ TrailFormation  │
                      ┌──────────────────┐        │ TrailQuality    │
                      │     deposit      │        │ simulate_ants   │
                      │                  │        └─────────────────┘
                      │  Point           │
                      │  Gradient        │
                      │  Broadcast       │
                      └──────────────────┘
```

**Data flow:** Agents create `Signal`s, deposit them via `DepositStrategy` into an `Environment`, where `SignalDecay` processes evaporation/diffusion/pruning each step. `ResponseFunction`s read the environment to decide agent behavior. `TrailFormation` tracks emergent path quality.

---

## Quick Start

```toml
[dependencies]
swarm-signals = "0.1"
serde = { version = "1", features = ["derive"] }
```

```rust
use swarm_signals::{
    signal::{Signal, SignalType},
    environment::Environment,
    deposit::DepositStrategy,
    decay::SignalDecay,
    response::ResponseFunction,
    trail::TrailFormation,
};

fn main() {
    // Create a 50×50 environment
    let mut env = Environment::new(50, 50);

    // Configure decay: 5% evaporation, 10% diffusion, prune below 0.01
    let decay = SignalDecay::new(0.05, 0.10, 0.01);

    // Create a trail tracker for pheromone signals
    let mut trail = TrailFormation::new(SignalType::Pheromone, 0.01);

    // Simulate 10 ants walking from (0, 25) toward (49, 25)
    let agents: Vec<(usize, usize)> = (0..10).map(|_| (0, 25)).collect();
    trail.simulate_ants(&mut env, &agents, (49, 25), 40, 1.0, &decay);

    // Check trail quality
    let quality = trail.quality(&env);
    println!("Trail cells: {}", quality.cell_count);
    println!("Total strength: {:.2}", quality.total_strength);
    println!("Avg strength: {:.4}", quality.avg_strength);
    println!("Connectivity: {:.2}%", quality.connectivity * 100.0);

    // An agent at (10, 25) follows the trail
    if let Some((nx, ny)) = trail.follow(&env, 10, 25) {
        println!("Next trail step: ({}, {})", nx, ny);
    }
}
```

**Expected output (approximate):**
```
Trail cells: 42
Total strength: 18.73
Avg strength: 0.4460
Connectivity: 95.24%
Next trail step: (11, 25)
```

---

## Modules

| Module | Purpose | Key Types |
|--------|---------|-----------|
| `signal` | Core signal representation | `Signal`, `SignalType` |
| `environment` | 2D grid for signal deposit and retrieval | `Environment` |
| `deposit` | Strategies for placing signals | `DepositStrategy` (Point, Gradient, Broadcast) |
| `decay` | Signal lifecycle: evaporation, diffusion, pruning | `SignalDecay` |
| `response` | Agent decision-making from signals | `ResponseFunction` (FollowStrongest, Avoid, Aggregate, ThresholdTrigger) |
| `trail` | Emergent path formation and quality metrics | `TrailFormation`, `TrailQuality`, `Waypoint` |

---

## Module Details

### `signal` — Signal Representation

A `Signal` is the atomic unit of stigmergic communication:

```rust
use swarm_signals::signal::{Signal, SignalType};

// Create a pheromone signal at position (10, 20)
let s = Signal::pheromone(1.0, 10, 20, 0, 0.05);

// Three signal types:
let pheromone = Signal::pheromone(1.0, 5, 5, 0, 0.1);  // attractive
let marker = Signal::marker(0.5, 5, 5, 0, 0.0);         // neutral waypoint
let barrier = Signal::barrier(2.0, 5, 5, 0, 0.2);       // repulsive

// Compute decayed strength
let remaining = s.strength_after(10);
// remaining = 1.0 × (1 - 0.05)^10 ≈ 0.5987
```

**Signal types:**
- `Pheromone` — agents are attracted to stronger concentrations
- `Marker` — neutral presence indicator or waypoint
- `Barrier` — agents avoid cells with strong barrier signals

### `environment` — The Shared World

The environment is a 2D grid where each cell stores signal strengths by type:

```rust
use swarm_signals::environment::Environment;
use swarm_signals::signal::{Signal, SignalType};

let mut env = Environment::new(100, 100);

// Deposit a signal
let s = Signal::pheromone(2.0, 50, 50, 0, 0.1);
env.deposit(&s);

// Read signal strength
let val = env.read(50, 50, SignalType::Pheromone); // 2.0

// Signals aggregate — depositing again adds strength
env.deposit(&s);
let val = env.read(50, 50, SignalType::Pheromone); // 4.0

// Set directly
env.set(50, 50, SignalType::Barrier, 3.0);

// Query all signal types at a cell
let cell = env.read_cell(50, 50);
// { Pheromone: 4.0, Barrier: 3.0 }

// Check occupancy
let occupied = env.occupied_cells(); // vec of (x, y, &HashMap)

// Get neighbors (4-connected)
let neighbors = env.neighbors4(50, 50); // [(49,50), (51,50), (50,49), (50,51)]
```

**Design choice:** The grid uses `Vec<Vec<HashMap<SignalType, f64>>>` — simple, serde-friendly, and avoids complex key types that cause serialization issues.

### `deposit` — Signal Placement Strategies

Different deposit patterns produce different emergent behaviors:

```rust
use swarm_signals::deposit::DepositStrategy;
use swarm_signals::environment::Environment;
use swarm_signals::signal::SignalType;

let mut env = Environment::new(50, 50);

// Point: single cell
DepositStrategy::Point.deposit(
    &mut env, SignalType::Marker, 1.0, 25, 25, 0, 0.1
);

// Gradient: radial falloff over radius
DepositStrategy::Gradient { radius: 5 }.deposit(
    &mut env, SignalType::Pheromone, 2.0, 25, 25, 0, 0.05
);

// Broadcast: uniform over rectangular area
DepositStrategy::Broadcast { half_width: 3, half_height: 3 }.deposit(
    &mut env, SignalType::Barrier, 1.0, 25, 25, 0, 0.0
);
```

### `decay` — Signal Lifecycle

Three mechanisms process signals over time:

```rust
use swarm_signals::decay::SignalDecay;
use swarm_signals::environment::Environment;
use swarm_signals::signal::SignalType;

let mut env = Environment::new(20, 20);
env.set(10, 10, SignalType::Pheromone, 1.0);

// 5% evaporation per step, 10% diffusion, prune below 0.01
let decay = SignalDecay::new(0.05, 0.10, 0.01);

// Single step: diffuse → evaporate → prune
decay.step(&mut env);

// Or run individual phases:
decay.diffuse(&mut env);   // spread to neighbors
decay.evaporate(&mut env); // reduce strength
decay.prune(&mut env);     // remove weak signals
```

### `response` — Agent Decision Making

Agents evaluate the environment and decide how to act:

```rust
use swarm_signals::response::{ResponseFunction, Direction};
use swarm_signals::environment::Environment;
use swarm_signals::signal::SignalType;

let mut env = Environment::new(20, 20);
env.set(12, 10, SignalType::Pheromone, 3.0); // strong signal to the right
env.set(10, 10, SignalType::Pheromone, 1.0);

// Follow the strongest pheromone
let resp = ResponseFunction::FollowStrongest;
let dir = resp.evaluate(&env, 11, 10, SignalType::Pheromone);
// Some(Direction { dx: 1, dy: 0 }) — move right toward stronger signal

// Avoid barriers
let avoid = ResponseFunction::Avoid;
let dir = avoid.evaluate(&env, 11, 10, SignalType::Pheromone);
// Some(Direction { dx: -1, dy: 0 }) — move away from strongest

// Aggregate toward center of mass
let agg = ResponseFunction::Aggregate { radius: 5 };
let dir = agg.evaluate(&env, 8, 10, SignalType::Pheromone);
// Some(Direction { dx: 1, dy: 0 }) — toward weighted centroid

// Threshold-triggered response
let trigger = ResponseFunction::ThresholdTrigger {
    threshold: 2.0,
    direction: Direction::new(0, -1),
};
let dir = trigger.evaluate(&env, 12, 10, SignalType::Pheromone);
// Some(Direction { dx: 0, dy: -1 }) — threshold met, fire
```

### `trail` — Emergent Path Formation

The highest-level module: simulate agents walking, depositing, and forming trails:

```rust
use swarm_signals::trail::TrailFormation;
use swarm_signals::decay::SignalDecay;
use swarm_signals::environment::Environment;
use swarm_signals::signal::SignalType;

let mut env = Environment::new(30, 30);
let decay = SignalDecay::new(0.03, 0.08, 0.005);
let mut trail = TrailFormation::new(SignalType::Pheromone, 0.01);

// 5 agents walk from (0, 15) to (29, 15) for 30 steps
let agents = vec![(0, 15); 5];
trail.simulate_ants(&mut env, &agents, (29, 15), 30, 1.0, &decay);

// Analyze the trail
let q = trail.quality(&env);
assert!(q.cell_count > 5, "Trail should span multiple cells");
assert!(q.connectivity > 0.5, "Trail should be well-connected");

// Follow the trail from a point
if let Some((nx, ny)) = trail.follow(&env, 5, 15) {
    println!("Trail leads to ({}, {})", nx, ny);
}
```

---

## Mathematical Foundation

### Exponential Decay

Signal strength decreases according to:

```
s(t + Δt) = s(t) · (1 - ρ)^Δt
```

Where:
- `s(t)` = signal strength at time `t`
- `ρ` = evaporation rate (0 ≤ ρ ≤ 1)
- `Δt` = number of time steps elapsed

For a signal deposited at time `t₀` with initial strength `s₀`:

```
s(t) = s₀ · (1 - ρ)^(t - t₀)
```

### Spatial Diffusion

Signal diffuses to 4-connected neighbors each step:

```
s_cell(t + 1) = s_cell(t) · (1 - δ·|N|) + Σ_{n ∈ N} δ · s_n(t)
```

Where:
- `δ` = diffusion rate per neighbor
- `N` = set of 4-connected neighbors
- `|N|` = neighbor count (2 for corners, 3 for edges, 4 for interior)

The diffusion conserves total signal strength (minus evaporation), spreading it spatially while preserving the integral.

### Trail Quality Metric

Trail quality is measured by:

```
Q = (S_total, n_cells, S_avg, C)
```

Where:
- `S_total = Σ_{i ∈ trail} s_i` — total signal strength
- `n_cells = |{i : s_i ≥ θ}|` — cells above threshold θ
- `S_avg = S_total / n_cells` — average signal per trail cell
- `C = |{i ∈ trail : ∃j ∈ trail, |i - j|₁ = 1}| / n_cells` — connectivity (fraction of cells adjacent to another trail cell)

A good trail has high `S_avg` (strong signal) and high `C` (well-connected path).

---

## Design Decisions

### Why `Vec<Vec<HashMap<SignalType, f64>>>` for the Grid?

We deliberately chose this over alternatives:

| Alternative | Problem |
|---|---|
| `HashMap<(usize, usize, SignalType), f64>` | Tuple struct keys serialize poorly with serde; slow for dense grids |
| `Vec<Vec<Vec<f64>>>` (indexed by type) | Wastes memory for sparse signal types; harder to iterate per cell |
| `Vec<f64>` with index math | Loses per-type semantics; harder to extend with new signal types |

Our choice gives O(1) lookup by `(x, y, type)`, simple serde (nested arrays of objects with string keys), and efficient sparse storage (empty cells are empty HashMaps).

### Why Separate Deposit, Decay, and Response?

The stigmergy pattern has three distinct phases: writing to the environment, processing the environment, and reading from the environment. Separating these into distinct types makes it easy to:

1. Mix and match strategies (e.g., gradient deposit + fast evaporation)
2. Test each phase independently
3. Serialize configurations separately
4. Reason about the physics of the system

### Why No External Dependencies Beyond Serde?

This crate models fundamental physics: deposition, decay, diffusion, and response. These don't require linear algebra, random number generation, or parallelism. Keeping the dependency tree minimal makes `swarm-signals` easy to embed in WebAssembly, embedded systems, or any Rust project.

### Why Serde for a Simulation Crate?

Environments often need to be:
- Saved and loaded (checkpointing simulations)
- Sent over networks (distributed simulations)
- Logged for analysis (replaying trail formation)

Serde with simple key types ensures this works reliably across formats (JSON, MessagePack, bincode).

---

## Related SuperInstance Crates

`swarm-signals` is part of the SuperInstance ecosystem:

- **[cosmic-web](https://github.com/SuperInstance/cosmic-web)** — distributed systems and interconnected services
- **[dial-ecology](https://github.com/SuperInstance/dial-ecology)** — ecological and evolutionary simulation patterns
- **[swarm-signals](https://github.com/SuperInstance/swarm-signals)** — this crate; stigmergic coordination

Together, these crates provide building blocks for simulating, building, and understanding complex adaptive systems — from ant colonies to distributed microservices.

---

## Serde Compatibility

All public types derive `Serialize` and `Deserialize`. The serialized form uses simple string keys:

```json
{
  "signal_type": "pheromone",
  "strength": 1.5,
  "x": 10,
  "y": 20,
  "timestamp": 5,
  "decay_rate": 0.1
}
```

No complex tuple keys, no custom serializers — just plain JSON-compatible structures that work with any serde format.

---

## License

MIT
