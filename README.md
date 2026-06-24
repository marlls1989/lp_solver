# lp_solver

A linear-programming model builder for Rust with pluggable solver backends.

Build a model with `LPModelBuilder` — add variables, add constraints, set an
objective — then solve it with either [COIN-OR CBC](https://github.com/coin-or/Cbc)
or [Gurobi](https://www.gurobi.com/). The model-building API is the same regardless
of which backend runs.

The core types (`VariableId`, `LinearExpression`, `Constraint`, `LPModelBuilder`)
carry a phantom `Brand` type parameter. A `VariableId` from one builder cannot be
used with another: such a mix-up is a compile error, not a runtime fault. The brand
is a zero-sized type and adds no runtime cost.

## Features

The crate selects backends through Cargo features:

- `coin_cbc` (default) — COIN-OR CBC. Requires the CBC system library
  (`brew install coin-or-tools/coinor/cbc`, `apt install coinor-libcbc-dev`).
- `gurobi` — Gurobi. Requires a Gurobi installation and a valid licence.

At least one backend feature must be enabled.

```toml
[dependencies]
lp_solver = "0.1"                                        # CBC (default)
lp_solver = { version = "0.1", features = ["gurobi"] }   # add Gurobi
```

## Example

```rust
use lp_solver::{constraint, lp_model_builder, OptimisationSense, VariableType};

let mut builder = lp_model_builder!();
let x = builder.add_variable(VariableType::Continuous, 0.0, f64::INFINITY);
let y = builder.add_variable(VariableType::Continuous, 0.0, f64::INFINITY);

builder.add_constraint(constraint!((x + y) <= 10.0));
builder.add_constraint(constraint!((x) >= 2.0));
builder.set_objective(x + 2.0 * y, OptimisationSense::Maximise);

let solution = builder.solve()?;
println!("objective = {}", solution.objective_value);
println!("x = {:?}", solution.get_value(x));
# Ok::<(), anyhow::Error>(())
```

Each `lp_model_builder!()` call mints a fresh brand. For a named brand, pass an
identifier: `lp_model_builder!(MyModel)`.

## Backend selection

When both backends are compiled in, the choice is made at solve time:

- `LP_SOLVER=gurobi` — use Gurobi only.
- `LP_SOLVER=coin_cbc` (or `cbc`) — use CBC only.
- unset — try Gurobi first, fall back to CBC if Gurobi fails (for example, when no
  licence is available).

The solvers write progress to standard output. Suppressing that is the caller's
responsibility.

## Licence

Dual-licensed under either of [Apache-2.0](LICENSE-APACHE) or [MIT](LICENSE-MIT) at
your option.
