//! End-to-end solve tests exercising the active backend through the public API.
//!
//! These run against whichever backend is compiled in (CBC by default). They assert real
//! numbers — variable values and the objective — not just that a solve succeeded.

use lp_solver::{
    NoSolution, OptimisationSense, SolutionStatus, SolveError, VariableType, constraint,
    lp_model_builder,
};

/// Absolute tolerance for comparing solver output (raw, unrounded).
const EPS: f64 = 1e-6;

fn close(a: f64, b: f64) -> bool {
    (a - b).abs() < EPS
}

#[test]
fn maximise_continuous_reports_optimal_objective_and_values() {
    let mut b = lp_model_builder!();
    let x = b.add_variable(VariableType::Continuous, 0.0, 10.0);
    let y = b.add_variable(VariableType::Continuous, 0.0, 10.0);
    b.add_constraint(constraint!((x + y) <= 10.0));
    b.set_objective(2.0 * x + 3.0 * y, OptimisationSense::Maximise);

    let sol = b.solve().expect("feasible model should solve");
    assert_eq!(sol.status, SolutionStatus::Optimal);

    let xv = sol.get_value(x).unwrap();
    let yv = sol.get_value(y).unwrap();
    // 3 beats 2 per unit, so push y to its bound and spend the rest on x.
    assert!(close(yv, 10.0), "y = {yv}");
    assert!(close(xv, 0.0), "x = {xv}");
    // The objective is reported consistently with the variable values.
    assert!(close(sol.objective_value, 2.0 * xv + 3.0 * yv));
    assert!(
        close(sol.objective_value, 30.0),
        "obj = {}",
        sol.objective_value
    );
}

#[test]
fn all_supported_senses_are_honoured() {
    let mut b = lp_model_builder!();
    let x = b.add_variable(VariableType::Continuous, 0.0, 10.0);
    let y = b.add_variable(VariableType::Continuous, 0.0, 10.0);
    b.add_constraint(constraint!((x) >= 2.0));
    b.add_constraint(constraint!((x) <= 8.0));
    b.add_constraint(constraint!((y) == 4.0));
    b.set_objective(x + y, OptimisationSense::Maximise);

    let sol = b.solve().unwrap();
    assert_eq!(sol.status, SolutionStatus::Optimal);
    assert!(close(sol.get_value(x).unwrap(), 8.0));
    assert!(close(sol.get_value(y).unwrap(), 4.0));
}

#[test]
fn constant_term_in_constraint_is_applied() {
    let mut b = lp_model_builder!();
    let x = b.add_variable(VariableType::Continuous, 0.0, 20.0);
    // x + 2 == 7  =>  x == 5
    b.add_constraint(constraint!((x + 2.0) == 7.0));
    b.set_objective(x, OptimisationSense::Maximise);

    let sol = b.solve().unwrap();
    assert!(close(sol.get_value(x).unwrap(), 5.0));
}

#[test]
fn negative_coefficient_is_applied() {
    let mut b = lp_model_builder!();
    let x = b.add_variable(VariableType::Continuous, 0.0, 5.0);
    let y = b.add_variable(VariableType::Continuous, 0.0, 10.0);
    // -x + y <= 0  =>  y <= x
    b.add_constraint(constraint!((-1.0 * x + y) <= 0.0));
    b.set_objective(y, OptimisationSense::Maximise);

    let sol = b.solve().unwrap();
    // y is capped by x, which is capped at 5.
    assert!(close(sol.get_value(y).unwrap(), 5.0));
}

#[test]
fn integer_variable_is_rounded_to_an_integer() {
    let mut b = lp_model_builder!();
    let x = b.add_variable(VariableType::Integer, 0.0, 10.0);
    b.add_constraint(constraint!((x) >= 3.5));
    b.set_objective(x, OptimisationSense::Minimise);

    let sol = b.solve().unwrap();
    assert_eq!(sol.status, SolutionStatus::Optimal);
    // Smallest integer >= 3.5 is 4.
    assert!(
        close(sol.get_value(x).unwrap(), 4.0),
        "x = {}",
        sol.get_value(x).unwrap()
    );
}

#[test]
fn binary_variables_take_zero_or_one() {
    let mut b = lp_model_builder!();
    let x = b.add_variable(VariableType::Binary, 0.0, 1.0);
    let y = b.add_variable(VariableType::Binary, 0.0, 1.0);
    b.add_constraint(constraint!((x + y) >= 1.0));
    b.set_objective(x + y, OptimisationSense::Minimise);

    let sol = b.solve().unwrap();
    assert_eq!(sol.status, SolutionStatus::Optimal);
    let xv = sol.get_value(x).unwrap();
    let yv = sol.get_value(y).unwrap();
    for v in [xv, yv] {
        assert!(close(v, 0.0) || close(v, 1.0), "binary value was {v}");
    }
    // Minimising the sum under x + y >= 1 gives exactly one set bit.
    assert!(close(xv + yv, 1.0));
}

#[test]
fn infeasible_model_is_an_error() {
    let mut b = lp_model_builder!();
    let x = b.add_variable(VariableType::Continuous, 0.0, 10.0);
    b.add_constraint(constraint!((x) >= 8.0));
    b.add_constraint(constraint!((x) <= 5.0));
    b.set_objective(x, OptimisationSense::Maximise);

    assert!(matches!(
        b.solve(),
        Err(SolveError::NoSolution(NoSolution::Infeasible))
    ));
}

#[test]
fn unbounded_model_yields_no_solution() {
    let mut b = lp_model_builder!();
    let x = b.add_variable(VariableType::Continuous, 0.0, f64::INFINITY);
    b.set_objective(x, OptimisationSense::Maximise);

    // An unbounded objective has no usable solution, so the result must be a negative
    // outcome. The exact variant is backend-specific: Gurobi reports `Unbounded`, but CBC
    // cannot always distinguish unbounded from infeasible and may report either — so this
    // test only pins down that it is *some* `NoSolution`, not which one.
    match b.solve() {
        Err(SolveError::NoSolution(_)) => {}
        Err(other) => panic!("expected a no-solution outcome, got error {other:?}"),
        Ok(_) => panic!("expected a no-solution outcome, but got a solution"),
    }
}

#[test]
fn model_without_objective_is_a_feasibility_problem() {
    let mut b = lp_model_builder!();
    let x = b.add_variable(VariableType::Continuous, 0.0, 10.0);
    b.add_constraint(constraint!((x) <= 5.0));

    let sol = b.solve().expect("feasible model should solve");
    assert_eq!(sol.status, SolutionStatus::Optimal);
    assert!(close(sol.objective_value, 0.0));
    // The single feasible value is within bounds and satisfies the constraint.
    let xv = sol.get_value(x).unwrap();
    assert!((0.0..=5.0 + EPS).contains(&xv), "x = {xv}");
}

#[test]
fn empty_model_solves_trivially() {
    let b = lp_model_builder!();
    let sol = b.solve().expect("an empty model is trivially feasible");
    assert_eq!(sol.status, SolutionStatus::Optimal);
    assert!(close(sol.objective_value, 0.0));
    assert_eq!(sol.values().count(), 0);
}

#[test]
fn values_iterates_in_variable_order() {
    let mut b = lp_model_builder!();
    let x = b.add_variable(VariableType::Continuous, 0.0, 10.0);
    let y = b.add_variable(VariableType::Continuous, 0.0, 10.0);
    b.add_constraint(constraint!((x) == 3.0));
    b.add_constraint(constraint!((y) == 7.0));
    b.set_objective(x + y, OptimisationSense::Maximise);

    let sol = b.solve().unwrap();
    let values: Vec<f64> = sol.values().collect();
    assert_eq!(values.len(), 2);
    assert!(close(values[0], 3.0));
    assert!(close(values[1], 7.0));
}

#[test]
fn removing_a_constraint_changes_the_optimum() {
    let mut b = lp_model_builder!();
    let x = b.add_variable(VariableType::Continuous, 0.0, 10.0);
    let limiting = b.add_constraint(constraint!((x) <= 4.0));
    b.set_objective(x, OptimisationSense::Maximise);

    // Remove the binding constraint; the optimum should rise to the variable's bound.
    let removed = b.remove_constraint(limiting);
    assert!(removed.is_some());

    let sol = b.solve().unwrap();
    assert!(close(sol.get_value(x).unwrap(), 10.0));
}
