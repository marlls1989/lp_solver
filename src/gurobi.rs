use std::collections::HashMap;

use ::gurobi::{ConstrSense, Env, LinExpr, Model, ModelSense, Status, VarType, attr};

use crate::{
    ConstraintSense, LPModelBuilder, LPSolution, ModelError, NoSolution, OptimisationSense,
    SolutionStatus, SolveError, VariableId, VariableType,
};

/// Solve an LP model using Gurobi
pub fn solve_gurobi<Brand>(
    builder: &LPModelBuilder<Brand>,
) -> Result<LPSolution<Brand>, SolveError> {
    // Gurobi writes progress to stdout; muting it (if desired) is the caller's responsibility.
    let env = Env::new("")?;
    let mut model = Model::new("lp_model", &env)?;

    // Add variables
    let mut var_map = HashMap::new();
    for (idx, var_info) in builder.variables.iter().enumerate() {
        let vtype = match var_info.var_type {
            VariableType::Continuous => VarType::Continuous,
            VariableType::Integer => VarType::Integer,
            VariableType::Binary => VarType::Binary,
        };

        let var = model.add_var(
            &format!("x_{}", idx),
            vtype,
            0.0, // objective coefficient
            var_info.lower_bound,
            var_info.upper_bound,
            &[], // coefficients for existing constraints
            &[], // constraint indices
        )?;

        let var_id = VariableId {
            id: idx,
            _brand: std::marker::PhantomData,
        };
        var_map.insert(var_id, var);
    }

    // Add constraints
    for (constr_id, constraint) in builder.constraints.iter().flatten().enumerate() {
        let mut gurobi_expr = LinExpr::new();

        for term in &constraint.expression.terms {
            if let Some(var) = var_map.get(&term.variable) {
                gurobi_expr = gurobi_expr.add_term(term.coefficient, var.clone());
            } else {
                return Err(ModelError::UnknownVariable {
                    id: term.variable.id,
                    count: builder.variables.len(),
                }
                .into());
            }
        }
        gurobi_expr = gurobi_expr.add_constant(constraint.expression.constant);

        let sense = match constraint.sense {
            ConstraintSense::LessEqual => ConstrSense::Less,
            ConstraintSense::Equal => ConstrSense::Equal,
            ConstraintSense::GreaterEqual => ConstrSense::Greater,
        };

        model.add_constr(
            &format!("c_{}", constr_id),
            gurobi_expr,
            sense,
            constraint.rhs,
        )?;
    }

    // Update the model before setting objective
    model.update()?;

    // Set objective
    if let Some(obj_info) = &builder.objective {
        let mut gurobi_expr = LinExpr::new();

        for term in &obj_info.expression.terms {
            if let Some(var) = var_map.get(&term.variable) {
                gurobi_expr = gurobi_expr.add_term(term.coefficient, var.clone());
            } else {
                return Err(ModelError::UnknownVariable {
                    id: term.variable.id,
                    count: builder.variables.len(),
                }
                .into());
            }
        }
        gurobi_expr = gurobi_expr.add_constant(obj_info.expression.constant);

        let sense = match obj_info.sense {
            OptimisationSense::Minimise => ModelSense::Minimize,
            OptimisationSense::Maximise => ModelSense::Maximize,
        };

        model.set_objective(gurobi_expr, sense)?;
    }

    // Optimise
    model.optimize()?;

    // Map the Gurobi status to an outcome. Negative outcomes return early as errors; only a
    // usable solution proceeds. Every `gurobi::Status` is handled explicitly.
    let status = model.status()?;
    let solution_status = match status {
        Status::Optimal => SolutionStatus::Optimal,
        // A sub-optimal solution is feasible but not proven optimal.
        Status::SubOptimal => SolutionStatus::Feasible,
        Status::Infeasible => return Err(NoSolution::Infeasible.into()),
        Status::Unbounded => return Err(NoSolution::Unbounded.into()),
        Status::InfOrUnbd => return Err(NoSolution::InfeasibleOrUnbounded.into()),
        // The solver stopped before reaching a conclusion (or never ran). Gurobi may hold a
        // feasible incumbent in some of these cases, but the crate exposes no way to read the
        // solution count, so treat them all as "stopped without a usable solution".
        Status::Loaded
        | Status::CutOff
        | Status::IterationLimit
        | Status::NodeLimit
        | Status::TimeLimit
        | Status::SolutionLimit
        | Status::Interrupted
        | Status::Numeric
        | Status::InProgress => return Err(NoSolution::Stopped.into()),
    };

    // A usable solution exists, so read its values.
    let num_vars = builder.variables.len();
    let mut variable_values = vec![0.0; num_vars];
    for (var_id, var) in &var_map {
        variable_values[var_id.id] = var.get(&model, attr::X)?;
    }
    let objective_value = model.get(attr::ObjVal)?;

    Ok(LPSolution {
        status: solution_status,
        objective_value,
        variable_values,
        _brand: std::marker::PhantomData,
    })
}
