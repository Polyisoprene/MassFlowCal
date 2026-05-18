use std::collections::HashMap;
use crate::db::Database;
use crate::error::EngineError;
use crate::models::graph::{DecanterParams, FlowBasis};
use crate::models::stream::MaterialStream;

use super::NodeOutputs;

pub fn compute_decanter(
    params: &DecanterParams,
    inputs: &HashMap<String, MaterialStream>,
    db: &Database,
) -> Result<NodeOutputs, EngineError> {
    let feed = inputs.values().next()
        .ok_or_else(|| EngineError::MissingInput {
            node: "Decanter".into(),
            port: "feed".into(),
        })?;

    let mw_lookup = |cas: &str| db.get_mw(cas).unwrap_or(0.0);
    let is_flow_rate = params.phase_mass_ratio < 0.0;

    let mut light_masses: HashMap<String, f64> = HashMap::new();
    let mut heavy_masses: HashMap<String, f64> = HashMap::new();

    for (cas, cf) in &feed.components {
        let light_i = if is_flow_rate {
            // 直接流量模式
            let split_amount = params.partition_coefficients.get(cas).copied().unwrap_or(0.0);
            split_amount.min(cf.mass_flow)
        } else {
            // 分率模式
            let frac = params.partition_coefficients.get(cas).copied().unwrap_or(0.5);
            cf.mass_flow * frac
        };
        let heavy_i = cf.mass_flow - light_i;

        if light_i > 0.0 { light_masses.insert(cas.clone(), light_i); }
        if heavy_i > 0.0 { heavy_masses.insert(cas.clone(), heavy_i); }
    }

    let light = MaterialStream::from_mass_map(light_masses, &mw_lookup);
    let heavy = MaterialStream::from_mass_map(heavy_masses, &mw_lookup);

    Ok(vec![
        ("light_phase".to_string(), light),
        ("heavy_phase".to_string(), heavy),
    ])
}
