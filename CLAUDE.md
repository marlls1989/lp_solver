# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## What this is

`lp_solver` is a Rust library that builds linear-programming models and solves them
through a pluggable backend (COIN-OR CBC or Gurobi). It was extracted from the
`hbcn` constrainer (part of the Pulsar asynchronous-circuit framework), where it
began as an internal module, and is published as a standalone crate.

## Build & test

Requires Rust 2024 edition and at least one solver backend.

```bash
cargo build                                  # default: coin_cbc backend
cargo build --features gurobi                # Gurobi backend (requires Gurobi + licence)
cargo test                                   # unit tests + doctests
cargo fmt --all -- --check                   # CI gates on this
cargo clippy --all-targets --all-features -- -D warnings   # CI gates on this
cargo run --example constraint_building      # see examples/
```

The default `coin_cbc` backend needs the CBC system library
(`brew install coin-or-tools/coinor/cbc` on macOS; `apt install coinor-libcbc-dev`
on Debian/Ubuntu).

## Architecture

- `src/lib.rs` — crate root. The core types: `LPModelBuilder<Brand>`,
  `VariableId<Brand>`, `LinearExpression<Brand>`, `Constraint<Brand>`,
  `LPSolution<Brand>`, the public enums (`VariableType`, `ConstraintSense`,
  `OptimisationSense`, `OptimisationStatus`), and the `solve()` dispatch /
  `LP_SOLVER` backend selection.
- `src/ops.rs` — operator overloading for building `LinearExpression` from variables
  and scalars (`+`, `-`, `*`, including the reverse `f64`-on-the-left forms).
- `src/macros.rs` — the `lp_model_builder!` and `constraint!` macros (`#[macro_export]`,
  so they live at the crate root; they expand to `$crate::…` paths).
- `src/coin_cbc.rs`, `src/gurobi.rs` — backend implementations, each compiled only
  when its feature is enabled.

## Conventions

- **Branded types.** Every core type is generic over a phantom `Brand`. The
  `lp_model_builder!()` macro mints a unique brand per call, so variables from
  different builders cannot be mixed — the type system rejects it at compile time.
  Use the macros rather than constructing `LPModelBuilder` with an explicit brand.
- **British spelling** in identifiers, docs, and output (`optimise`, `behaviour`,
  `serialise`) — inherited from the Pulsar repositories.
- **No I/O side effects.** The solvers print to stdout; the crate does not redirect
  or suppress it. Muting solver chatter is the caller's responsibility.
- Documentation is factual and precise; avoid a marketing tone.

## Backend selection

`solve()` chooses a backend at runtime. With `LP_SOLVER` set to `gurobi` or
`coin_cbc`/`cbc`, that backend is used exclusively. Unset, it tries Gurobi first (if
compiled in) and falls back to CBC on failure.
