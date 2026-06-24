# Changelog

All notable changes to this project are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

_Add changes here as they land; rename this section to the new version on release._

### Planned

- Migrate the Gurobi backend from the unmaintained `gurobi` crate to the maintained
  `grb` 3.x bindings (modern Gurobi support).

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
