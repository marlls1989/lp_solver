//! A high-level, solver-agnostic abstraction layer for linear and mixed-integer programming.
//!
//! `lp_solver` lets you describe an optimisation problem once — variables, linear constraints,
//! and an objective — through an ergonomic builder and natural operator syntax, then hand it to a
//! pluggable backend ([COIN-OR CBC] or [Gurobi]) to solve. The same model code runs on either
//! solver; switching backends is a configuration choice, not a rewrite.
//!
//! Build a model with [`LPModelBuilder`], express constraints with the [`constraint!`] macro or
//! operator overloading, then call [`solve`](LPModelBuilder::solve).
//!
//! [COIN-OR CBC]: https://github.com/coin-or/Cbc
//! [Gurobi]: https://www.gurobi.com/
//!
//! # Quick start
//!
//! ```rust,no_run
//! use lp_solver::{constraint, lp_model_builder, OptimisationSense, SolveError, VariableType};
//!
//! # fn main() -> Result<(), SolveError> {
//! // maximise 2x + 3y  subject to  x + y <= 10,  x >= 2,  with 0 <= x, y <= 10
//! let mut builder = lp_model_builder!();
//! let x = builder.add_variable(VariableType::Continuous, 0.0, 10.0);
//! let y = builder.add_variable(VariableType::Continuous, 0.0, 10.0);
//!
//! builder.add_constraint(constraint!((x + y) <= 10.0));
//! builder.add_constraint(constraint!((x) >= 2.0));
//! builder.set_objective(2.0 * x + 3.0 * y, OptimisationSense::Maximise);
//!
//! // `Ok` means a usable solution was found; a negative outcome is an `Err`.
//! let solution = builder.solve()?;
//! println!("objective = {}", solution.objective_value);
//! println!("x = {:?}, y = {:?}", solution.get_value(x), solution.get_value(y));
//! # Ok(())
//! # }
//! ```
//!
//! # Backends
//!
//! A model is solver-agnostic; the backend is chosen at solve time. Enable a backend with its
//! Cargo feature — `coin_cbc` (default) and/or `gurobi` — and select it with the `LP_SOLVER`
//! environment variable:
//!
//! - `LP_SOLVER=gurobi` — use Gurobi only.
//! - `LP_SOLVER=coin_cbc` (or `cbc`) — use CBC only.
//! - unset — try Gurobi first (if enabled) and automatically fall back to CBC if Gurobi cannot
//!   run (for example, no licence). The fallback reason is logged to stderr.
//!
//! # Building a model
//!
//! Add variables with [`LPModelBuilder::add_variable`] ([`Continuous`](VariableType::Continuous),
//! [`Integer`](VariableType::Integer), or [`Binary`](VariableType::Binary)). Build linear
//! expressions from variables and scalars with the usual operators (`2.0 * x + 3.0 * y - 5.0`),
//! and constraints with the [`constraint!`] macro (`==`, `<=`, `>=`):
//!
//! ```rust
//! use lp_solver::{constraint, lp_model_builder, VariableType};
//!
//! let mut builder = lp_model_builder!();
//! let x = builder.add_variable(VariableType::Continuous, 0.0, 10.0);
//! let y = builder.add_variable(VariableType::Continuous, 0.0, 10.0);
//!
//! builder.add_constraint(constraint!((x + y) == 10.0));
//! builder.add_constraint(constraint!((2.0 * x - y) <= 5.0));
//! ```
//!
//! The [`Constraint::eq`], [`le`](Constraint::le) and [`ge`](Constraint::ge) constructors (and
//! [`Constraint::new`]) are equivalent alternatives to the macro.
//!
//! # Results
//!
//! [`solve`](LPModelBuilder::solve) returns `Result<LPSolution, SolveError>`. An `Ok` always
//! carries a usable solution — its [`status`](LPSolution::status) is [`SolutionStatus::Optimal`]
//! or [`Feasible`](SolutionStatus::Feasible), and `objective_value` and
//! [`get_value`](LPSolution::get_value) are meaningful — so `if let Ok(solution) = builder.solve()`
//! is enough to know the result is valid. A problem with no usable solution (infeasible,
//! unbounded, or stopped) is reported as [`SolveError::NoSolution`]; configuration and backend
//! failures are the other [`SolveError`] variants.
//!
//! # Type safety
//!
//! The core types carry a zero-sized phantom `Brand`, so a [`VariableId`] from one builder cannot
//! be used with another — such a mix-up is a compile error, not a runtime fault, at no runtime
//! cost. [`lp_model_builder!`] mints a fresh brand per call; for a named brand use the explicit
//! form:
//!
//! ```rust
//! use lp_solver::{LPModelBuilder, VariableType};
//!
//! struct ProductionModel;
//! let mut builder = LPModelBuilder::<ProductionModel>::new();
//! let _x = builder.add_variable(VariableType::Continuous, 0.0, 10.0);
//! ```

#![warn(missing_docs)]

use std::env;
use std::marker::PhantomData;

/// Variable types supported by LP solvers
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)]
pub enum VariableType {
    /// Continuous variable (can take any real value)
    Continuous,
    /// Integer variable (can only take integer values)
    Integer,
    /// Binary variable (can only take values 0 or 1)
    Binary,
}

/// Constraint sense for linear constraints
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)]
pub enum ConstraintSense {
    /// Less than or equal to (≤)
    LessEqual,
    /// Equal to (=)
    Equal,
    /// Greater than or equal to (≥)
    GreaterEqual,
}

/// Optimisation direction
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OptimisationSense {
    /// Minimise the objective function
    Minimise,
    /// Maximise the objective function
    Maximise,
}

/// A successful optimisation outcome: a usable solution is available.
///
/// Carried by [`LPSolution`], which is only produced when the solve succeeds. Negative
/// outcomes (infeasible, unbounded, …) are reported as [`NoSolution`] via an `Err`, not
/// here. Marked `#[non_exhaustive]`: more positive outcomes may be distinguished later.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum SolutionStatus {
    /// An optimal solution was found and proven optimal.
    Optimal,
    /// A feasible solution is available but was not proven optimal — the solver
    /// relaxed its optimality tolerances or stopped early while holding an incumbent.
    Feasible,
}

/// Available LP solver backends
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)]
enum SolverBackend {
    #[cfg(feature = "gurobi")]
    /// Gurobi commercial solver
    Gurobi,
    #[cfg(feature = "coin_cbc")]
    /// Coin CBC open-source solver
    CoinCbc,
}

impl SolverBackend {
    /// Resolve a solver name (as accepted in `LP_SOLVER`) to a backend.
    ///
    /// Pure and case-insensitive, so it can be tested without touching the
    /// process environment. Returns [`ConfigError::SolverNotEnabled`] if the named
    /// solver's feature is not compiled in, or [`ConfigError::InvalidSolverName`]
    /// for an unrecognised name.
    fn backend_from_name(name: &str) -> Result<Self, ConfigError> {
        match name.to_lowercase().as_str() {
            "gurobi" => {
                #[cfg(feature = "gurobi")]
                return Ok(SolverBackend::Gurobi);
                #[cfg(not(feature = "gurobi"))]
                Err(ConfigError::SolverNotEnabled {
                    requested: "gurobi",
                })
            }
            "coin_cbc" | "coin-cbc" | "cbc" => {
                #[cfg(feature = "coin_cbc")]
                return Ok(SolverBackend::CoinCbc);
                #[cfg(not(feature = "coin_cbc"))]
                Err(ConfigError::SolverNotEnabled {
                    requested: "coin_cbc",
                })
            }
            _ => Err(ConfigError::InvalidSolverName {
                name: name.to_string(),
            }),
        }
    }

    /// Get the solver backend from environment variable or use fallback logic
    fn from_env_or_default() -> Result<Self, ConfigError> {
        // Check if LP_SOLVER environment variable is set
        if let Ok(solver_name) = env::var("LP_SOLVER") {
            return Self::backend_from_name(&solver_name);
        }

        // Fallback logic: prefer gurobi if available, then coin_cbc
        #[cfg(feature = "gurobi")]
        return Ok(SolverBackend::Gurobi);

        #[allow(unreachable_code)]
        #[cfg(feature = "coin_cbc")]
        return Ok(SolverBackend::CoinCbc);

        #[cfg(not(any(feature = "gurobi", feature = "coin_cbc")))]
        Err(ConfigError::NoBackendAvailable)
    }
}

/// A linear expression term: coefficient * variable
#[derive(Debug, Clone)]
pub struct LinearTerm<Brand> {
    /// The scalar multiplier applied to the variable.
    pub coefficient: f64,
    /// The variable this term refers to.
    pub variable: VariableId<Brand>,
}

/// A linear expression: sum of terms plus constant
#[derive(Debug, Clone)]
pub struct LinearExpression<Brand> {
    /// The weighted variable terms summed by this expression. A variable may appear
    /// more than once; backends coalesce repeated variables by summing coefficients.
    pub terms: Vec<LinearTerm<Brand>>,
    /// The constant added to the sum of the terms.
    pub constant: f64,
}

impl<Brand> LinearExpression<Brand> {
    /// Create a new linear expression with a constant term
    pub fn new(constant: f64) -> Self {
        Self {
            terms: Vec::new(),
            constant,
        }
    }

    /// Add a term to the expression
    pub fn add_term(&mut self, coefficient: f64, variable: VariableId<Brand>) {
        self.terms.push(LinearTerm {
            coefficient,
            variable,
        });
    }

    /// Create a linear expression from a single variable
    pub fn from_variable(variable: VariableId<Brand>) -> Self {
        Self {
            terms: vec![LinearTerm {
                coefficient: 1.0,
                variable,
            }],
            constant: 0.0,
        }
    }
}

impl<Brand> From<VariableId<Brand>> for LinearExpression<Brand> {
    fn from(variable: VariableId<Brand>) -> Self {
        Self::from_variable(variable)
    }
}

/// Unique identifier for a variable in the LP model
///
/// The `Brand` type parameter ensures that variables can only be used with the
/// builder that created them. This is enforced at compile time.
pub struct VariableId<Brand> {
    id: usize,
    _brand: PhantomData<fn() -> Brand>,
}

// Manual trait implementations that don't require Brand to implement anything
impl<Brand> std::fmt::Debug for VariableId<Brand> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("VariableId").field("id", &self.id).finish()
    }
}

impl<Brand> Clone for VariableId<Brand> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<Brand> Copy for VariableId<Brand> {}

impl<Brand> PartialEq for VariableId<Brand> {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}

impl<Brand> Eq for VariableId<Brand> {}

impl<Brand> std::hash::Hash for VariableId<Brand> {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.id.hash(state);
    }
}

/// A stable handle to a constraint in an [`LPModelBuilder`]
///
/// Returned by [`LPModelBuilder::add_constraint`] and accepted by
/// [`LPModelBuilder::remove_constraint`]. The handle remains valid for the lifetime
/// of the builder, even after other constraints are removed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ConstraintId(usize);

/// A linear constraint representation
///
/// Constraints define relationships between linear expressions and constants.
/// The `Brand` type parameter ensures type safety - constraints can only use
/// variables from the builder that will consume them.
///
/// # Examples
///
/// ```rust,no_run
/// use lp_solver::constraint;
/// use lp_solver::lp_model_builder;
/// use lp_solver::{Constraint, ConstraintSense, VariableType};
///
/// let mut builder = lp_model_builder!();
/// let x = builder.add_variable(VariableType::Continuous, 0.0, 10.0);
/// let y = builder.add_variable(VariableType::Continuous, 0.0, 10.0);
///
/// // Using the constraint! macro (recommended)
/// let c = constraint!((x + y) == 10.0);
///
/// // Using builder methods
/// let c = Constraint::eq(x + y, 10.0);
///
/// // Using the constructor directly
/// let c = Constraint::new(x + y, ConstraintSense::Equal, 10.0);
/// ```
#[derive(Debug, Clone)]
pub struct Constraint<Brand> {
    expression: LinearExpression<Brand>,
    sense: ConstraintSense,
    rhs: f64,
}

impl<Brand> Constraint<Brand> {
    /// Create a new constraint
    pub fn new(
        expression: impl Into<LinearExpression<Brand>>,
        sense: ConstraintSense,
        rhs: f64,
    ) -> Self {
        Self {
            expression: expression.into(),
            sense,
            rhs,
        }
    }

    /// Create an equality constraint: expression == rhs
    pub fn eq(expression: impl Into<LinearExpression<Brand>>, rhs: f64) -> Self {
        Self::new(expression, ConstraintSense::Equal, rhs)
    }

    /// Create a less-than-or-equal constraint: expression <= rhs
    pub fn le(expression: impl Into<LinearExpression<Brand>>, rhs: f64) -> Self {
        Self::new(expression, ConstraintSense::LessEqual, rhs)
    }

    /// Create a greater-than-or-equal constraint: expression >= rhs
    pub fn ge(expression: impl Into<LinearExpression<Brand>>, rhs: f64) -> Self {
        Self::new(expression, ConstraintSense::GreaterEqual, rhs)
    }
}

/// Variable information stored in the model
#[derive(Debug, Clone)]
struct VariableInfo {
    var_type: VariableType,
    lower_bound: f64,
    upper_bound: f64,
}

/// Objective function information
#[derive(Debug, Clone)]
struct ObjectiveInfo<Brand> {
    expression: LinearExpression<Brand>,
    sense: OptimisationSense,
}

/// Result of solving an LP model
///
/// Only produced for a successful solve, so its values are always meaningful; the
/// [`status`](Self::status) distinguishes a proven-optimal solution from a merely
/// feasible one.
pub struct LPSolution<Brand> {
    /// Whether the solution is proven optimal or only feasible.
    pub status: SolutionStatus,
    /// The objective value at the returned solution.
    pub objective_value: f64,
    variable_values: Vec<f64>,
    _brand: PhantomData<fn() -> Brand>,
}

// Manual `Debug`/`Clone` so they do not spuriously require `Brand: Debug`/`Brand: Clone`
// (the brand is a zero-sized phantom marker — often a bare unit struct that implements
// neither). Mirrors the hand-written impls on `VariableId`.
impl<Brand> std::fmt::Debug for LPSolution<Brand> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LPSolution")
            .field("status", &self.status)
            .field("objective_value", &self.objective_value)
            .field("variable_values", &self.variable_values)
            .finish()
    }
}

impl<Brand> Clone for LPSolution<Brand> {
    fn clone(&self) -> Self {
        Self {
            status: self.status,
            objective_value: self.objective_value,
            variable_values: self.variable_values.clone(),
            _brand: PhantomData,
        }
    }
}

impl<Brand> LPSolution<Brand> {
    /// Get the value of a variable from the solution
    pub fn get_value(&self, var_id: VariableId<Brand>) -> Option<f64> {
        self.variable_values.get(var_id.id).copied()
    }

    /// Iterate over every variable value, in the order the variables were added
    ///
    /// Yields one `f64` per variable. Pair it with [`get_value`](Self::get_value)
    /// when you need the value of a specific [`VariableId`].
    pub fn values(&self) -> impl Iterator<Item = f64> + '_ {
        self.variable_values.iter().copied()
    }
}

/// Builder for LP models that can work with different backends
///
/// The `Brand` type parameter ensures type safety - variables from one builder
/// cannot be accidentally used with another builder. This is enforced at compile time.
///
/// # Examples
///
/// ```rust,no_run
/// use lp_solver::lp_model_builder;
/// use lp_solver::{LPModelBuilder, VariableType};
///
/// // Each builder has its own brand
/// struct MyModel;
/// let mut builder1 = LPModelBuilder::<MyModel>::new();
///
/// // Variables are branded with the builder type
/// let x = builder1.add_variable(VariableType::Continuous, 0.0, 10.0);
///
/// // For simple cases, use the macro to create a unique brand
/// let mut builder2 = lp_model_builder!();  // Creates unique brand automatically
/// ```
pub struct LPModelBuilder<Brand> {
    variables: Vec<VariableInfo>,
    // Constraints are stored in tombstone slots: removing a constraint replaces its slot with
    // `None` rather than shifting the vector, so every `ConstraintId` handed out stays valid.
    constraints: Vec<Option<Constraint<Brand>>>,
    objective: Option<ObjectiveInfo<Brand>>,
    _brand: PhantomData<fn() -> Brand>,
}

impl<Brand> LPModelBuilder<Brand> {
    /// Create a new LP model builder
    pub fn new() -> Self {
        Self {
            variables: Vec::new(),
            constraints: Vec::new(),
            objective: None,
            _brand: PhantomData,
        }
    }

    /// Add a variable to the model
    pub fn add_variable(
        &mut self,
        var_type: VariableType,
        lower_bound: f64,
        upper_bound: f64,
    ) -> VariableId<Brand> {
        let var_id = VariableId {
            id: self.variables.len(),
            _brand: PhantomData,
        };
        self.variables.push(VariableInfo {
            var_type,
            lower_bound,
            upper_bound,
        });
        var_id
    }

    /// Add a constraint to the model
    ///
    /// Returns a [`ConstraintId`] handle that stays valid for the lifetime of the
    /// builder and can later be passed to [`remove_constraint`](Self::remove_constraint).
    pub fn add_constraint(&mut self, constraint: Constraint<Brand>) -> ConstraintId {
        let constr_id = ConstraintId(self.constraints.len());
        self.constraints.push(Some(constraint));
        constr_id
    }

    /// Remove a previously-added constraint from the model
    ///
    /// Returns the removed [`Constraint`], or `None` if the constraint was already
    /// removed or the id does not belong to this builder. Removing a constraint does
    /// not invalidate any other [`ConstraintId`].
    pub fn remove_constraint(&mut self, id: ConstraintId) -> Option<Constraint<Brand>> {
        self.constraints.get_mut(id.0).and_then(|slot| slot.take())
    }

    /// Set the objective function
    pub fn set_objective<E>(&mut self, expression: E, sense: OptimisationSense)
    where
        E: Into<LinearExpression<Brand>>,
    {
        self.objective = Some(ObjectiveInfo {
            expression: expression.into(),
            sense,
        });
    }

    /// Validate the model before handing it to a backend.
    ///
    /// Confirms that every variable referenced by a constraint or the objective
    /// is part of this model. Branded types make a foreign variable unreachable
    /// through the public API, but the check ensures a malformed model returns a
    /// recoverable [`ModelError`] rather than panicking on an out-of-range index
    /// inside a backend.
    fn validate(&self) -> Result<(), ModelError> {
        let count = self.variables.len();
        let check = |expr: &LinearExpression<Brand>| -> Result<(), ModelError> {
            for term in &expr.terms {
                if term.variable.id >= count {
                    return Err(ModelError::UnknownVariable {
                        id: term.variable.id,
                        count,
                    });
                }
            }
            Ok(())
        };
        for constraint in self.constraints.iter().flatten() {
            check(&constraint.expression)?;
        }
        if let Some(objective) = &self.objective {
            check(&objective.expression)?;
        }
        Ok(())
    }

    /// Solve the model with automatic fallback from Gurobi to Coin CBC
    ///
    /// This method implements the following solver selection strategy:
    /// 1. If LP_SOLVER environment variable is set, use the specified solver only
    /// 2. Otherwise, try Gurobi first (if available) and fallback to CBC on failure
    /// 3. Fallback is triggered by Gurobi license issues or other initialisation errors
    ///
    /// A model with no objective set is treated as a feasibility problem: if it is
    /// satisfiable the result is `Ok` with [`SolutionStatus::Optimal`] and an
    /// `objective_value` of `0.0`.
    ///
    /// A successful `Ok` always carries a usable solution; the only thing to check on it is
    /// whether the [`status`](LPSolution::status) is `Optimal` or merely `Feasible`.
    ///
    /// # Errors
    ///
    /// Returns [`SolveError::NoSolution`] when the solve completes but yields no usable
    /// solution (infeasible, unbounded, or stopped); [`SolveError::Config`] if `LP_SOLVER`
    /// names an unknown or not-compiled-in solver, or no backend feature is enabled;
    /// [`SolveError::Model`] if the model fails validation (a variable not belonging to this
    /// builder); and [`SolveError::Gurobi`] for an error reported by the Gurobi backend.
    pub fn solve(self) -> Result<LPSolution<Brand>, SolveError> {
        // Reject malformed models up front so backends never panic on a bad index.
        self.validate()?;

        // Check if user explicitly requested a specific solver
        if std::env::var("LP_SOLVER").is_ok() {
            let solver = SolverBackend::from_env_or_default()?;
            return match solver {
                #[cfg(feature = "gurobi")]
                SolverBackend::Gurobi => crate::gurobi::solve_gurobi(&self),

                #[cfg(feature = "coin_cbc")]
                SolverBackend::CoinCbc => crate::coin_cbc::solve_coin_cbc(&self),
            };
        }

        // Default behaviour: try Gurobi first, fallback to CBC on failure
        #[cfg(feature = "gurobi")]
        {
            match crate::gurobi::solve_gurobi(&self) {
                Ok(solution) => Ok(solution),
                // A definitive no-solution result is authoritative — CBC would only agree, so
                // do not waste a second solve. Only operational Gurobi failures fall back.
                Err(e @ SolveError::NoSolution(_)) => Err(e),
                Err(gurobi_error) => {
                    // Check if CBC is available as fallback
                    #[cfg(feature = "coin_cbc")]
                    {
                        eprintln!("Gurobi failed ({}), falling back to Coin CBC", gurobi_error);
                        crate::coin_cbc::solve_coin_cbc(&self)
                    }
                    #[cfg(not(feature = "coin_cbc"))]
                    {
                        // No fallback solver compiled in; surface the Gurobi error directly.
                        Err(gurobi_error)
                    }
                }
            }
        }

        // If Gurobi is not available, try CBC directly
        #[cfg(all(feature = "coin_cbc", not(feature = "gurobi")))]
        {
            crate::coin_cbc::solve_coin_cbc(&self)
        }

        // No solvers available
        #[cfg(not(any(feature = "gurobi", feature = "coin_cbc")))]
        Err(SolveError::Config(ConfigError::NoBackendAvailable))
    }
}

impl<Brand> Default for LPModelBuilder<Brand> {
    fn default() -> Self {
        Self::new()
    }
}

// Error types
pub mod error;
pub use error::{ConfigError, ModelError, NoSolution, SolveError};

// Macros for convenient syntax
pub mod macros;

// Operator overloading for linear expressions
pub mod ops;

/// Gurobi backend: the `solve_gurobi` entry point used by [`LPModelBuilder::solve`].
#[cfg(feature = "gurobi")]
pub mod gurobi;

/// COIN-OR CBC backend: the `solve_coin_cbc` entry point used by [`LPModelBuilder::solve`].
#[cfg(feature = "coin_cbc")]
pub mod coin_cbc;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{constraint, lp_model_builder};

    #[test]
    fn test_constraint_macro() {
        let mut builder = lp_model_builder!();
        let x = builder.add_variable(VariableType::Continuous, 0.0, 10.0);
        let y = builder.add_variable(VariableType::Continuous, 0.0, 10.0);

        // Test constraint creation
        let c = constraint!((x + y) == 10.0);
        assert_eq!(c.sense, ConstraintSense::Equal);
        assert_eq!(c.rhs, 10.0);

        let c = constraint!((2.0 * x) <= 5.0);
        assert_eq!(c.sense, ConstraintSense::LessEqual);
        assert_eq!(c.rhs, 5.0);

        let c = constraint!((x - y) >= 0.0);
        assert_eq!(c.sense, ConstraintSense::GreaterEqual);
        assert_eq!(c.rhs, 0.0);
    }

    #[test]
    fn test_constraint_macro_with_builder() {
        let mut builder = lp_model_builder!();
        let x = builder.add_variable(VariableType::Continuous, 0.0, 10.0);
        let y = builder.add_variable(VariableType::Continuous, 0.0, 10.0);

        // Test that constraints can be added to builder
        builder.add_constraint(constraint!((x + y) == 10.0));
        builder.add_constraint(constraint!((x) <= 5.0));

        assert_eq!(builder.constraints.len(), 2);
    }

    #[test]
    fn test_constraint_builders() {
        let mut builder = lp_model_builder!();
        let x = builder.add_variable(VariableType::Continuous, 0.0, 10.0);

        // Test convenience builders
        let c = Constraint::eq(x + 5.0, 10.0);
        assert_eq!(c.sense, ConstraintSense::Equal);

        let c = Constraint::le(x * 2.0, 10.0);
        assert_eq!(c.sense, ConstraintSense::LessEqual);

        let c = Constraint::ge(x - 1.0, 0.0);
        assert_eq!(c.sense, ConstraintSense::GreaterEqual);
    }

    #[test]
    fn test_add_variable_to_linear_expression() {
        let mut builder = lp_model_builder!();
        let x = builder.add_variable(VariableType::Continuous, 0.0, 10.0);
        let y = builder.add_variable(VariableType::Continuous, 0.0, 10.0);

        // Build an expression like 2.0 * x + 5.0
        let expr = 2.0 * x + 5.0;

        // Verify the initial expression
        assert_eq!(expr.terms.len(), 1);
        assert_eq!(expr.terms[0].coefficient, 2.0);
        assert_eq!(expr.terms[0].variable, x);
        assert_eq!(expr.constant, 5.0);

        // Perform expr + y using the Add<VariableId> for LinearExpression implementation
        let result = expr + y;

        // Assert that the resulting LinearExpression has two terms and preserves the constant (5.0)
        assert_eq!(
            result.terms.len(),
            2,
            "Result should have exactly two terms"
        );
        assert_eq!(result.constant, 5.0, "Constant should be preserved as 5.0");

        // Check first term (should be 2.0 * x)
        assert_eq!(result.terms[0].coefficient, 2.0);
        assert_eq!(result.terms[0].variable, x);

        // Check second term (should be 1.0 * y, added from the VariableId)
        assert_eq!(result.terms[1].coefficient, 1.0);
        assert_eq!(result.terms[1].variable, y);
    }

    #[test]
    fn test_duplicate_variable_coefficients_are_summed() {
        // Regression: a variable appearing more than once in a single expression must
        // SUM its coefficients, not collapse to the last one. `maximise x s.t. x + x <= 10`
        // has optimum x = 5; the pre-fix CBC backend overwrote the per-(row,col) weight and
        // effectively solved `x <= 10`, returning x = 10.
        let mut builder = lp_model_builder!();
        let x = builder.add_variable(VariableType::Continuous, 0.0, 10.0);
        builder.add_constraint(constraint!((x + x) <= 10.0));
        builder.set_objective(x, OptimisationSense::Maximise);

        let solution = builder.solve().expect("solve should succeed");
        assert_eq!(solution.status, SolutionStatus::Optimal);
        let x_val = solution.get_value(x).expect("x should have a value");
        assert!(
            (x_val - 5.0).abs() < 1e-6,
            "expected x = 5 (2x <= 10), got {x_val}"
        );
    }

    #[test]
    fn backend_from_name_rejects_unknown() {
        assert!(matches!(
            SolverBackend::backend_from_name("glpk"),
            Err(ConfigError::InvalidSolverName { name }) if name == "glpk"
        ));
    }

    #[cfg(feature = "coin_cbc")]
    #[test]
    fn backend_from_name_accepts_cbc_aliases() {
        for name in ["cbc", "coin_cbc", "coin-cbc", "CBC", "Coin_CBC"] {
            assert!(
                matches!(
                    SolverBackend::backend_from_name(name),
                    Ok(SolverBackend::CoinCbc)
                ),
                "{name} should resolve to CoinCbc"
            );
        }
    }

    #[cfg(not(feature = "gurobi"))]
    #[test]
    fn backend_from_name_reports_gurobi_not_enabled() {
        assert!(matches!(
            SolverBackend::backend_from_name("gurobi"),
            Err(ConfigError::SolverNotEnabled {
                requested: "gurobi"
            })
        ));
    }

    #[cfg(feature = "gurobi")]
    #[test]
    fn backend_from_name_accepts_gurobi() {
        assert!(matches!(
            SolverBackend::backend_from_name("gurobi"),
            Ok(SolverBackend::Gurobi)
        ));
    }

    #[test]
    fn validate_rejects_variable_from_outside_the_model() {
        // The branded API cannot express this, but a malformed model must fail gracefully
        // (recoverable error) rather than panicking inside a backend. Build the bad model by
        // hand using the crate-internal fields.
        let mut builder = lp_model_builder!();
        let _real = builder.add_variable(VariableType::Continuous, 0.0, 10.0);
        let bogus = VariableId {
            id: 99,
            _brand: PhantomData,
        };
        builder.add_constraint(Constraint::new(
            LinearExpression::from_variable(bogus),
            ConstraintSense::LessEqual,
            5.0,
        ));

        assert!(matches!(
            builder.solve(),
            Err(SolveError::Model(ModelError::UnknownVariable {
                id: 99,
                count: 1
            }))
        ));
    }

    #[test]
    fn get_value_returns_none_for_out_of_range_id() {
        let solution: LPSolution<()> = LPSolution {
            status: SolutionStatus::Optimal,
            objective_value: 0.0,
            variable_values: vec![1.0, 2.0],
            _brand: PhantomData,
        };
        let valid = VariableId {
            id: 1,
            _brand: PhantomData,
        };
        let invalid = VariableId {
            id: 5,
            _brand: PhantomData,
        };
        assert_eq!(solution.get_value(valid), Some(2.0));
        assert_eq!(solution.get_value(invalid), None);
    }
}
