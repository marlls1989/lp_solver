//! Error types for the LP model builder and its solver backends.
//!
//! The errors are split into focused leaf types — [`ConfigError`] for backend
//! selection and [`ModelError`] for model validation — and a composite
//! [`SolveError`] that joins them (and any backend error) behind a single type
//! returned by [`LPModelBuilder::solve`](crate::LPModelBuilder::solve). Each leaf
//! type can grow new variants independently because every enum is
//! `#[non_exhaustive]`.

use std::fmt;
use std::io;

/// Errors arising from backend selection and the `LP_SOLVER` environment variable.
///
/// These are configuration problems detected before any model is handed to a
/// solver: a solver was requested whose Cargo feature is absent, the requested
/// solver name is not recognised, or no backend was compiled in at all.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum ConfigError {
    /// `LP_SOLVER` named a solver whose Cargo feature was not enabled at build time.
    SolverNotEnabled {
        /// The solver requested via `LP_SOLVER` (e.g. `"gurobi"` or `"coin_cbc"`).
        requested: &'static str,
    },
    /// `LP_SOLVER` held a value that does not name any known solver.
    InvalidSolverName {
        /// The unrecognised value read from `LP_SOLVER`.
        name: String,
    },
    /// No solver backend feature was enabled at build time.
    NoBackendAvailable,
}

impl fmt::Display for ConfigError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ConfigError::SolverNotEnabled { requested } => write!(
                f,
                "solver '{}' was requested via LP_SOLVER but its Cargo feature is not enabled",
                requested
            ),
            ConfigError::InvalidSolverName { name } => write!(
                f,
                "invalid solver '{}' in LP_SOLVER; valid options are: gurobi, coin_cbc",
                name
            ),
            ConfigError::NoBackendAvailable => write!(
                f,
                "no LP solver backend available; enable a solver feature \
                 (e.g. 'gurobi' or 'coin_cbc')"
            ),
        }
    }
}

impl std::error::Error for ConfigError {}

impl From<ConfigError> for io::Error {
    fn from(err: ConfigError) -> Self {
        io::Error::new(io::ErrorKind::InvalidInput, err)
    }
}

/// Errors arising from validating a model before it is handed to a backend.
///
/// These guard internal invariants so that a malformed model returns a
/// recoverable error rather than panicking inside a backend (for example on an
/// out-of-range index while computing the objective value).
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum ModelError {
    /// An expression referenced a variable that is not part of the model.
    ///
    /// The branded-type design makes this unreachable through the public API
    /// (a [`VariableId`](crate::VariableId) can only come from the builder that
    /// minted it), but it is checked explicitly so a hand-constructed or
    /// otherwise malformed model fails gracefully instead of panicking.
    UnknownVariable {
        /// The offending variable index.
        id: usize,
        /// The number of variables actually present in the model.
        count: usize,
    },
}

impl fmt::Display for ModelError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ModelError::UnknownVariable { id, count } => write!(
                f,
                "variable index {} is out of range for a model with {} variable(s)",
                id, count
            ),
        }
    }
}

impl std::error::Error for ModelError {}

impl From<ModelError> for io::Error {
    fn from(err: ModelError) -> Self {
        io::Error::new(io::ErrorKind::InvalidData, err)
    }
}

/// A negative optimisation outcome: the solve ran but produced no usable solution.
///
/// Returned (wrapped in [`SolveError::NoSolution`]) instead of an
/// [`LPSolution`](crate::LPSolution), so a
/// successful `Ok` always carries a usable solution and callers need not inspect a status
/// to tell success from failure.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum NoSolution {
    /// The model was proven infeasible: no assignment satisfies the constraints.
    Infeasible,
    /// The model was proven unbounded: the objective can be improved without limit.
    Unbounded,
    /// The model is infeasible or unbounded; the solver could not distinguish the two.
    InfeasibleOrUnbounded,
    /// The solver stopped before reaching a conclusion and without a usable solution —
    /// for example a time, node, iteration or solution limit, a cutoff, numerical
    /// trouble, an interruption, or a model that was never optimised.
    Stopped,
}

impl fmt::Display for NoSolution {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let reason = match self {
            NoSolution::Infeasible => "the model is infeasible",
            NoSolution::Unbounded => "the model is unbounded",
            NoSolution::InfeasibleOrUnbounded => "the model is infeasible or unbounded",
            NoSolution::Stopped => "the solver stopped without finding a solution",
        };
        f.write_str(reason)
    }
}

impl std::error::Error for NoSolution {}

impl From<NoSolution> for io::Error {
    fn from(err: NoSolution) -> Self {
        io::Error::new(io::ErrorKind::InvalidData, err)
    }
}

/// Errors that can occur while solving a model.
///
/// This is the composite error returned by
/// [`LPModelBuilder::solve`](crate::LPModelBuilder::solve) and the backend entry
/// points. It wraps the focused [`ConfigError`] and [`ModelError`] leaf types as
/// well as any error reported by the Gurobi backend.
#[derive(Debug)]
#[non_exhaustive]
pub enum SolveError {
    /// Backend selection / configuration error.
    Config(ConfigError),
    /// Model validation error.
    Model(ModelError),
    /// The solve completed but yielded no usable solution (infeasible, unbounded, …).
    NoSolution(NoSolution),
    /// The Gurobi backend returned an error.
    #[cfg(feature = "gurobi")]
    Gurobi(::gurobi::Error),
}

impl fmt::Display for SolveError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SolveError::Config(e) => write!(f, "configuration error: {}", e),
            SolveError::Model(e) => write!(f, "model error: {}", e),
            SolveError::NoSolution(e) => write!(f, "no solution: {}", e),
            #[cfg(feature = "gurobi")]
            SolveError::Gurobi(e) => write!(f, "Gurobi backend error: {}", e),
        }
    }
}

impl std::error::Error for SolveError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            SolveError::Config(e) => Some(e),
            SolveError::Model(e) => Some(e),
            SolveError::NoSolution(e) => Some(e),
            #[cfg(feature = "gurobi")]
            SolveError::Gurobi(e) => Some(e),
        }
    }
}

impl From<ConfigError> for SolveError {
    fn from(err: ConfigError) -> Self {
        SolveError::Config(err)
    }
}

impl From<ModelError> for SolveError {
    fn from(err: ModelError) -> Self {
        SolveError::Model(err)
    }
}

impl From<NoSolution> for SolveError {
    fn from(err: NoSolution) -> Self {
        SolveError::NoSolution(err)
    }
}

#[cfg(feature = "gurobi")]
impl From<::gurobi::Error> for SolveError {
    fn from(err: ::gurobi::Error) -> Self {
        SolveError::Gurobi(err)
    }
}

impl From<SolveError> for io::Error {
    fn from(err: SolveError) -> Self {
        match err {
            SolveError::Config(e) => e.into(),
            SolveError::Model(e) => e.into(),
            SolveError::NoSolution(e) => e.into(),
            #[cfg(feature = "gurobi")]
            SolveError::Gurobi(e) => io::Error::other(e),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::error::Error;

    #[test]
    fn config_error_displays_requested_solver() {
        let err = ConfigError::SolverNotEnabled {
            requested: "gurobi",
        };
        let msg = err.to_string();
        assert!(msg.contains("gurobi"));
        assert!(msg.contains("not enabled"));
    }

    #[test]
    fn config_error_displays_invalid_name() {
        let err = ConfigError::InvalidSolverName {
            name: "glpk".to_string(),
        };
        let msg = err.to_string();
        assert!(msg.contains("glpk"));
        assert!(msg.contains("valid options"));
    }

    #[test]
    fn model_error_displays_index_and_count() {
        let err = ModelError::UnknownVariable { id: 7, count: 3 };
        let msg = err.to_string();
        assert!(msg.contains('7'));
        assert!(msg.contains('3'));
    }

    #[test]
    fn no_solution_displays_reason() {
        assert!(NoSolution::Infeasible.to_string().contains("infeasible"));
        assert!(NoSolution::Unbounded.to_string().contains("unbounded"));
        assert!(NoSolution::Stopped.to_string().contains("stopped"));
    }

    #[test]
    fn solve_error_from_no_solution_sets_variant_and_source() {
        let err: SolveError = NoSolution::Infeasible.into();
        assert!(matches!(
            err,
            SolveError::NoSolution(NoSolution::Infeasible)
        ));
        assert!(err.source().is_some());
    }

    #[test]
    fn no_solution_to_io_error_is_invalid_data() {
        let io_err: io::Error = NoSolution::Unbounded.into();
        assert_eq!(io_err.kind(), io::ErrorKind::InvalidData);
    }

    #[test]
    fn solve_error_from_config_sets_variant_and_source() {
        let err: SolveError = ConfigError::NoBackendAvailable.into();
        assert!(matches!(err, SolveError::Config(_)));
        assert!(err.source().is_some());
    }

    #[test]
    fn solve_error_from_model_sets_variant_and_source() {
        let err: SolveError = ModelError::UnknownVariable { id: 1, count: 0 }.into();
        assert!(matches!(err, SolveError::Model(_)));
        assert!(err.source().is_some());
    }

    #[test]
    fn config_error_to_io_error_is_invalid_input() {
        let io_err: io::Error = ConfigError::NoBackendAvailable.into();
        assert_eq!(io_err.kind(), io::ErrorKind::InvalidInput);
    }

    #[test]
    fn model_error_to_io_error_is_invalid_data() {
        let io_err: io::Error = ModelError::UnknownVariable { id: 1, count: 0 }.into();
        assert_eq!(io_err.kind(), io::ErrorKind::InvalidData);
    }

    #[test]
    fn solve_error_config_to_io_error_preserves_kind() {
        let solve_err = SolveError::Config(ConfigError::NoBackendAvailable);
        let io_err: io::Error = solve_err.into();
        assert_eq!(io_err.kind(), io::ErrorKind::InvalidInput);
    }

    #[test]
    fn solve_error_model_to_io_error_preserves_kind() {
        let solve_err = SolveError::Model(ModelError::UnknownVariable { id: 1, count: 0 });
        let io_err: io::Error = solve_err.into();
        assert_eq!(io_err.kind(), io::ErrorKind::InvalidData);
    }
}
