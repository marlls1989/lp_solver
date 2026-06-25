use std::collections::HashMap;

use grb::constr::IneqExpr;
use grb::expr::{Expr, LinExpr};
use grb::{ConstrSense, Env, Model, ModelSense, Status, Var, VarType, attr};

use crate::{
    ConstraintSense, LPModelBuilder, LPSolution, LinearExpression, ModelError, NoSolution,
    OptimisationSense, SolutionStatus, SolveError, VariableId, VariableType,
};

/// Build a Gurobi linear expression from a crate [`LinearExpression`], validating that every
/// referenced variable belongs to the model. `grb`'s expression arithmetic sums repeated terms,
/// so duplicate variables need no special handling.
fn build_lin_expr<Brand>(
    expr: &LinearExpression<Brand>,
    var_map: &HashMap<VariableId<Brand>, Var>,
    num_vars: usize,
) -> Result<LinExpr, SolveError> {
    let mut lin = LinExpr::new();
    for term in &expr.terms {
        match var_map.get(&term.variable) {
            Some(var) => {
                lin.add_term(term.coefficient, *var);
            }
            None => {
                return Err(ModelError::UnknownVariable {
                    id: term.variable.id,
                    count: num_vars,
                }
                .into());
            }
        }
    }
    lin.add_constant(expr.constant);
    Ok(lin)
}

/// Solve an LP model using Gurobi (via the `grb` bindings).
pub fn solve_gurobi<Brand>(
    builder: &LPModelBuilder<Brand>,
) -> Result<LPSolution<Brand>, SolveError> {
    // Gurobi writes progress to stdout; muting it (if desired) is the caller's responsibility.
    // An empty log-file name avoids creating a `gurobi.log` on disk.
    let env = Env::new("")?;
    let mut model = Model::with_env("lp_model", &env)?;

    let num_vars = builder.variables.len();

    // Add variables.
    let mut var_map: HashMap<VariableId<Brand>, Var> = HashMap::with_capacity(num_vars);
    for (idx, var_info) in builder.variables.iter().enumerate() {
        let vtype = match var_info.var_type {
            VariableType::Continuous => VarType::Continuous,
            VariableType::Integer => VarType::Integer,
            VariableType::Binary => VarType::Binary,
        };

        let var = model.add_var(
            &format!("x_{idx}"),
            vtype,
            0.0, // objective coefficient (set globally below)
            var_info.lower_bound,
            var_info.upper_bound,
            std::iter::empty::<(grb::Constr, f64)>(), // no column coefficients
        )?;

        let var_id = VariableId {
            id: idx,
            _brand: std::marker::PhantomData,
        };
        var_map.insert(var_id, var);
    }

    // Add constraints.
    for (constr_id, constraint) in builder.constraints.iter().flatten().enumerate() {
        let lhs = build_lin_expr(&constraint.expression, &var_map, num_vars)?;

        let sense = match constraint.sense {
            ConstraintSense::LessEqual => ConstrSense::Less,
            ConstraintSense::Equal => ConstrSense::Equal,
            ConstraintSense::GreaterEqual => ConstrSense::Greater,
        };

        let con = IneqExpr {
            lhs: Expr::Linear(lhs),
            sense,
            rhs: Expr::Constant(constraint.rhs),
        };
        model.add_constr(&format!("c_{constr_id}"), con)?;
    }

    // Set objective.
    if let Some(obj_info) = &builder.objective {
        let obj = build_lin_expr(&obj_info.expression, &var_map, num_vars)?;
        let sense = match obj_info.sense {
            OptimisationSense::Minimise => ModelSense::Minimize,
            OptimisationSense::Maximise => ModelSense::Maximize,
        };
        model.set_objective(Expr::Linear(obj), sense)?;
    }

    // Optimise.
    model.optimize()?;

    // Map the Gurobi status to an outcome. Negative outcomes return early as errors; only a
    // usable solution proceeds. On limit/interrupt statuses the solver may hold a feasible
    // incumbent: `grb` exposes the solution count, so return it as `Feasible` rather than
    // discarding it (an improvement over the previous, conservative mapping).
    let status = model.status()?;
    let solution_status = match status {
        Status::Optimal => SolutionStatus::Optimal,
        // A sub-optimal solution is feasible but not proven optimal.
        Status::SubOptimal => SolutionStatus::Feasible,
        Status::Infeasible => return Err(NoSolution::Infeasible.into()),
        Status::Unbounded => return Err(NoSolution::Unbounded.into()),
        Status::InfOrUnbd => return Err(NoSolution::InfeasibleOrUnbounded.into()),
        // No usable solution information is available for these.
        Status::Loaded | Status::CutOff | Status::InProgress => {
            return Err(NoSolution::Stopped.into());
        }
        // Any other terminal status (time/node/iteration/solution limit, interrupt, numerical
        // trouble, …): usable only if the solver found an incumbent.
        _ => {
            if model.get_attr(attr::SolCount)? > 0 {
                SolutionStatus::Feasible
            } else {
                return Err(NoSolution::Stopped.into());
            }
        }
    };

    // A usable solution exists, so read its values.
    let mut variable_values = vec![0.0; num_vars];
    for (var_id, var) in &var_map {
        variable_values[var_id.id] = model.get_obj_attr(attr::X, var)?;
    }
    let objective_value = model.get_attr(attr::ObjVal)?;

    Ok(LPSolution {
        status: solution_status,
        objective_value,
        variable_values,
        _brand: std::marker::PhantomData,
    })
}
