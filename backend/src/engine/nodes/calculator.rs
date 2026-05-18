use std::collections::HashMap;
use crate::db::Database;
use crate::error::EngineError;
use crate::models::graph::{CalculatorOp, CalculatorParams};
use crate::models::stream::MaterialStream;

use super::NodeOutputs;

pub fn compute_calculator(
    params: &CalculatorParams,
    inputs: &HashMap<String, MaterialStream>,
    db: &Database,
) -> Result<NodeOutputs, EngineError> {
    let mw_lookup = |cas: &str| db.get_mw(cas).unwrap_or(0.0);

    let result = match &params.operation {
        CalculatorOp::Add | CalculatorOp::Subtract => {
            let mut input_iter = inputs.values();
            let a = input_iter.next()
                .ok_or_else(|| EngineError::MissingInput {
                    node: "Calculator".into(),
                    port: "in_A".into(),
                })?;
            let b = input_iter.next()
                .ok_or_else(|| EngineError::MissingInput {
                    node: "Calculator".into(),
                    port: "in_B".into(),
                })?;

            let mut all_cas: Vec<String> = a.components.keys()
                .chain(b.components.keys())
                .map(|c| c.clone())
                .collect();
            all_cas.sort();
            all_cas.dedup();

            let mut result_masses: HashMap<String, f64> = HashMap::new();
            for cas in &all_cas {
                let ma = a.components.get(cas).map(|c| c.mass_flow).unwrap_or(0.0);
                let mb = b.components.get(cas).map(|c| c.mass_flow).unwrap_or(0.0);
                let val = match params.operation {
                    CalculatorOp::Add => ma + mb,
                    CalculatorOp::Subtract => ma - mb,
                    _ => unreachable!(),
                };
                if val < -0.001 {
                    return Err(EngineError::NegativeFlow {
                        node: "Calculator".into(),
                        cas: cas.clone(),
                        value: val,
                    });
                }
                if val > 0.0 {
                    result_masses.insert(cas.clone(), val);
                }
            }
            MaterialStream::from_mass_map(result_masses, &mw_lookup)
        }
        CalculatorOp::Multiply(_factor) | CalculatorOp::Divide(_factor) => {
            let a = inputs.values().next()
                .ok_or_else(|| EngineError::MissingInput {
                    node: "Calculator".into(),
                    port: "in_A".into(),
                })?;

            let factor_val = match &params.operation {
                CalculatorOp::Multiply(f) | CalculatorOp::Divide(f) => *f,
                _ => unreachable!(),
            };

            if factor_val == 0.0 && matches!(params.operation, CalculatorOp::Divide(_)) {
                return Err(EngineError::Computation("除以零".into()));
            }

            let result_masses: HashMap<String, f64> = a.components
                .iter()
                .map(|(cas, cf)| {
                    let val = match params.operation {
                        CalculatorOp::Multiply(_) => cf.mass_flow * factor_val,
                        CalculatorOp::Divide(_) => cf.mass_flow / factor_val,
                        _ => cf.mass_flow,
                    };
                    (cas.clone(), val)
                })
                .filter(|(_, v)| *v > 0.0)
                .collect();

            MaterialStream::from_mass_map(result_masses, &mw_lookup)
        }
    };

    Ok(vec![("result".to_string(), result)])
}
