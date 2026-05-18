use std::collections::HashMap;
use crate::db::Database;
use crate::error::EngineError;
use crate::models::stream::MaterialStream;

use super::NodeOutputs;

pub fn compute_mixer(
    inputs: &HashMap<String, MaterialStream>,
    db: &Database,
) -> Result<NodeOutputs, EngineError> {
    if inputs.is_empty() {
        return Err(EngineError::MissingInput {
            node: "Mixer".into(),
            port: "in_1".into(),
        });
    }

    let mw_lookup = |cas: &str| db.get_mw(cas).unwrap_or(0.0);

    // 汇总所有入口的各组分质量流量
    let mut combined: HashMap<String, f64> = HashMap::new();
    for stream in inputs.values() {
        for (cas, cf) in &stream.components {
            *combined.entry(cas.clone()).or_insert(0.0) += cf.mass_flow;
        }
    }

    // 去除零流量的组分
    combined.retain(|_, v| *v > 0.0);

    let out = MaterialStream::from_mass_map(combined, &mw_lookup);

    Ok(vec![("out".to_string(), out)])
}
