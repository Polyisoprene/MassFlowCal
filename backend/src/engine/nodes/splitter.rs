use std::collections::HashMap;
use crate::db::Database;
use crate::error::EngineError;
use crate::models::graph::SplitterParams;
use crate::models::stream::MaterialStream;

use super::NodeOutputs;

pub fn compute_splitter(
    params: &SplitterParams,
    inputs: &HashMap<String, MaterialStream>,
    db: &Database,
) -> Result<NodeOutputs, EngineError> {
    let feed = inputs.values().next()
        .ok_or_else(|| EngineError::MissingInput {
            node: "Splitter".into(),
            port: "feed".into(),
        })?;

    // 校验分率之和
    let sum: f64 = params.splits.iter().sum();
    if (sum - 1.0).abs() > 0.001 {
        return Err(EngineError::SplitFractionSum(sum));
    }

    let mut outputs: NodeOutputs = Vec::new();
    let mw_lookup = |cas: &str| db.get_mw(cas).unwrap_or(0.0);

    for (i, fraction) in params.splits.iter().enumerate() {
        let port_name = format!("out_{}", i + 1);

        let component_masses: HashMap<String, f64> = feed.components
            .iter()
            .map(|(cas, cf)| (cas.clone(), cf.mass_flow * fraction))
            .collect();

        let stream = MaterialStream::from_mass_map(component_masses, &mw_lookup);
        outputs.push((port_name, stream));
    }

    Ok(outputs)
}
