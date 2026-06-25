# Changelog

All notable changes to this project are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

_Add changes here as they land; rename this section to the new version on release._

### Changed

- Migrated the Gurobi backend from the unmaintained `gurobi` crate (Gurobi 9.x) to the maintained
  `grb` 3.x bindings (modern Gurobi). The `gurobi` Cargo feature now pins grb's `gurobi12` binding,
  whose env-creation FFI links against Gurobi 11/12/13; verified end-to-end against Gurobi 12.0.3.
  Building the feature needs a Gurobi install discoverable via `GUROBI_HOME` or `gurobi_cl` on
  `PATH` — see the README.
- The Gurobi backend now distinguishes a feasible incumbent on early-stopped solves: for limit /
  interrupt statuses it returns `SolutionStatus::Feasible` when the solver holds a solution
  (`SolCount > 0`) instead of always reporting `NoSolution::Stopped`. This was not expressible with
  the previous `gurobi` crate, which could not query the solution count.

### Build

- On macOS, `grb-sys2`'s upstream build script only matches `libgurobi*.so` and so fails to locate
  `libgurobi*.dylib`. This repo pins a small fork via `[patch.crates-io]` whose build script matches
  the library by regex (incl. `.dylib`) and derives `GUROBI_HOME` from `gurobi_cl` on `PATH` when
  unset. (Published-crate consumers do not inherit `[patch]`; they set `GUROBI_LIBNAME` themselves.)

## [0.1.1] - 2026-06-24

### Changed

- Reframed the crate landing page (docs.rs and README) around the value proposition — a
  high-level, solver-agnostic abstraction for linear and mixed-integer programming — leading
  with a quick-start example. Branded types are demoted to a concise "Type safety" section and
  internal implementation notes were removed from the public docs. Documentation only; no API
  or behaviour changes.

## [0.1.0] - 2026-06-24

Initial release.

### Added

- `LPModelBuilder` for building LP/MILP models: continuous, integer, and binary variables,
  linear constraints, and an objective.
- Natural expression syntax via operator overloading (including `f64`-on-the-left forms and
  unary negation) and the `constraint!` macro (`==`, `<=`, `>=`).
- Pluggable backends — COIN-OR CBC (default) and Gurobi — selected at solve time via the
  `LP_SOLVER` environment variable, with automatic Gurobi→CBC fallback on operational failure.
- Result-typed outcomes: `solve()` returns `Ok(LPSolution)` for a usable solution
  (`SolutionStatus::Optimal`/`Feasible`) and `Err(SolveError::NoSolution(_))` for
  infeasible/unbounded/stopped problems.
- Hand-rolled, zero-dependency error types (`ConfigError`, `ModelError`, `NoSolution`, and the
  composite `SolveError`).
- Compile-time branded types preventing a variable from one model being used with another, at
  no runtime cost.
- Constraint removal via stable `ConstraintId` handles and an `LPSolution::values()` iterator.

### Requirements

- Rust 1.85+ (edition 2024); a backend system library (COIN-OR CBC, or a licensed Gurobi).

[Unreleased]: https://github.com/marlls1989/lp_solver/compare/v0.1.1...HEAD
[0.1.1]: https://github.com/marlls1989/lp_solver/compare/v0.1.0...v0.1.1
[0.1.0]: https://github.com/marlls1989/lp_solver/releases/tag/v0.1.0
