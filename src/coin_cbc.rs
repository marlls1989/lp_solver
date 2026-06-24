use std::collections::HashMap;

use ::coin_cbc::raw::SecondaryStatus;
use ::coin_cbc::{Model, Sense};

use crate::{
    ConstraintSense, LPModelBuilder, LPSolution, LinearTerm, ModelError, NoSolution,
    OptimisationSense, SolutionStatus, SolveError, VariableId, VariableType,
};

/// Solve an LP model using Coin CBC
pub fn solve_coin_cbc<Brand>(
    builder: &LPModelBuilder<Brand>,
) -> Result<LPSolution<Brand>, SolveError> {
    // CBC writes progress to stdout; muting it (if desired) is the caller's responsibility.
    let mut model = Model::default();
    let mut var_map = HashMap::new();

    // Add variables to the model
    for (idx, var_info) in builder.variables.iter().enumerate() {
        let col = match var_info.var_type {
            VariableType::Continuous => {
                let col = model.add_col();
                model.set_col_lower(col, var_info.lower_bound);
                model.set_col_upper(col, var_info.upper_bound);
                col
            }
            VariableType::Integer => {
                let col = model.add_integer();
                model.set_col_lower(col, var_info.lower_bound);
                model.set_col_upper(col, var_info.upper_bound);
                col
            }
            VariableType::Binary => model.add_binary(),
        };
        let var_id = VariableId {
            id: idx,
            _brand: std::marker::PhantomData,
        };
        var_map.insert(var_id, col);
    }

    // Add constraints
    for constraint in builder.constraints.iter().flatten() {
        let row = model.add_row();

        // Coalesce duplicate variables before handing off: CBC's `set_weight` OVERWRITES
        // the coefficient for a (row, col) pair, so a variable appearing more than once in
        // one expression would silently collapse to its last coefficient instead of summing
        // (Gurobi's `add_term` sums). Accumulate here so both backends agree.
        let coalesced = coalesce_terms(&constraint.expression.terms);
        for (variable, coefficient) in coalesced {
            if let Some(&col) = var_map.get(&variable) {
                model.set_weight(row, col, coefficient);
            } else {
                return Err(ModelError::UnknownVariable {
                    id: variable.id,
                    count: builder.variables.len(),
                }
                .into());
            }
        }

        // Handle constant term
        let rhs_adjusted = constraint.rhs - constraint.expression.constant;

        // Add constraint based on sense
        match constraint.sense {
            ConstraintSense::LessEqual => {
                model.set_row_upper(row, rhs_adjusted);
            }
            ConstraintSense::Equal => {
                model.set_row_equal(row, rhs_adjusted);
            }
            ConstraintSense::GreaterEqual => {
                model.set_row_lower(row, rhs_adjusted);
            }
        }
    }

    // Set objective function (coalesce duplicate variables, as with constraint rows).
    if let Some(obj_info) = &builder.objective {
        for (variable, coefficient) in coalesce_terms(&obj_info.expression.terms) {
            if let Some(&col) = var_map.get(&variable) {
                model.set_obj_coeff(col, coefficient);
            } else {
                return Err(ModelError::UnknownVariable {
                    id: variable.id,
                    count: builder.variables.len(),
                }
                .into());
            }
        }

        let sense = match obj_info.sense {
            OptimisationSense::Minimise => Sense::Minimize,
            OptimisationSense::Maximise => Sense::Maximize,
        };

        model.set_obj_sense(sense);
    }

    // Solve the model
    let solution = model.solve();

    // Determine the outcome BEFORE reading any values. A negative outcome is returned as an
    // error; only a usable solution (optimal, or a feasible incumbent from an early stop)
    // proceeds to value extraction.
    let raw = solution.raw();
    // Check unboundedness before infeasibility: CBC can report an unbounded model as
    // "proven infeasible", so the unbounded test must win when both are set.
    let status = if raw.is_proven_optimal() {
        SolutionStatus::Optimal
    } else if raw.is_continuous_unbounded() {
        return Err(NoSolution::Unbounded.into());
    } else if raw.is_proven_infeasible() {
        return Err(NoSolution::Infeasible.into());
    } else if raw.secondary_status() == SecondaryStatus::HasSolution {
        // The solve stopped early (a limit or user event) but a feasible incumbent is available.
        SolutionStatus::Feasible
    } else {
        // Stopped without a usable solution (limit reached, abandoned, numerical trouble).
        return Err(NoSolution::Stopped.into());
    };

    // A usable solution exists, so its columns are meaningful.
    let num_vars = builder.variables.len();
    let mut variable_values = vec![0.0; num_vars];
    for (var_id, col) in var_map.iter() {
        variable_values[var_id.id] = solution.col(*col);
    }

    let objective_value = if let Some(obj_info) = &builder.objective {
        let mut obj_val = obj_info.expression.constant;
        for term in &obj_info.expression.terms {
            obj_val += term.coefficient * variable_values[term.variable.id];
        }
        obj_val
    } else {
        0.0
    };

    Ok(LPSolution {
        status,
        objective_value,
        variable_values,
        _brand: std::marker::PhantomData,
    })
}

/// Sum coefficients of repeated variables so each variable is handed to CBC exactly once.
///
/// Insertion order of first appearance is preserved for deterministic model construction.
fn coalesce_terms<Brand>(terms: &[LinearTerm<Brand>]) -> Vec<(VariableId<Brand>, f64)> {
    let mut order: Vec<VariableId<Brand>> = Vec::new();
    let mut sums: HashMap<VariableId<Brand>, f64> = HashMap::new();
    for term in terms {
        if let std::collections::hash_map::Entry::Vacant(e) = sums.entry(term.variable) {
            e.insert(term.coefficient);
            order.push(term.variable);
        } else {
            *sums.get_mut(&term.variable).unwrap() += term.coefficient;
        }
    }
    order.into_iter().map(|v| (v, sums[&v])).collect()
}
