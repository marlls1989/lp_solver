# lp_solver

A high-level, solver-agnostic abstraction layer for linear and mixed-integer
programming in Rust.

Describe an optimisation problem once — variables, linear constraints, an objective —
with `LPModelBuilder` and natural operator syntax, then hand it to a pluggable backend
([COIN-OR CBC](https://github.com/coin-or/Cbc) or [Gurobi](https://www.gurobi.com/)) to
solve. The same model code runs on either solver; switching backends is a configuration
choice, not a rewrite.

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
println!("status = {:?}", solution.status);
println!("objective = {}", solution.objective_value);
println!("x = {:?}", solution.get_value(x));
# Ok::<(), lp_solver::SolveError>(())
```

## Interpreting results

`solve()` splits the outcome across the `Result`: a usable solution is `Ok`, any negative
outcome is `Err`. So `if let Ok(solution) = builder.solve()` is enough to know the result is
valid — there is no per-status matching on the happy path.

- `Ok(LPSolution)` — a solution exists. Its `status` is a `SolutionStatus`: `Optimal` (proven
  optimal) or `Feasible` (a usable incumbent that was not proven optimal). `objective_value` and
  `get_value`/`values` are always meaningful.
- `Err(SolveError::NoSolution(_))` — the solve ran but produced no usable solution: `Infeasible`,
  `Unbounded`, `InfeasibleOrUnbounded`, or `Stopped` (a limit/cutoff/numerical/interrupted halt).
- `Err(SolveError::Config(_) | SolveError::Model(_) | SolveError::Gurobi(_))` — the solve could
  not run: a bad `LP_SOLVER` value or no backend enabled, an invalid model, or a backend error.

A model with no objective is treated as a feasibility problem and, if satisfiable, returns
`Ok` with `SolutionStatus::Optimal` and `objective_value == 0.0`.

## Backend selection

When both backends are compiled in, the choice is made at solve time:

- `LP_SOLVER=gurobi` — use Gurobi only.
- `LP_SOLVER=coin_cbc` (or `cbc`) — use CBC only.
- unset — try Gurobi first, fall back to CBC if Gurobi fails (for example, when no
  licence is available).

The solvers write progress to standard output. Suppressing that is the caller's
responsibility.

## Type safety

The core types carry a zero-sized phantom `Brand`, so a `VariableId` from one builder
cannot be used with another — a mix-up is a compile error, not a runtime fault, at no
runtime cost. Each `lp_model_builder!()` call mints a fresh brand; for a named brand pass
an identifier (`lp_model_builder!(MyModel)`) or use the explicit
`LPModelBuilder::<MyBrand>::new()`.

## Licence

Dual-licensed under either of [Apache-2.0](LICENSE-APACHE) or [MIT](LICENSE-MIT) at
your option.
