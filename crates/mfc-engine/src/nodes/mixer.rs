use std::collections::HashMap;
use mfc_core::error::EngineError;
use mfc_core::stream::MaterialStream;
use mfc_core::traits::ComponentProvider;

use super::NodeOutputs;

pub fn compute_mixer(
    inputs: &HashMap<String, MaterialStream>,
    provider: &dyn ComponentProvider,
) -> Result<NodeOutputs, EngineError> {
    if inputs.is_empty() {
        return Err(EngineError::MissingInput {
            node: "Mixer".into(),
            port: "in_1".into(),
        });
    }

    let mw_lookup = |cas: &str| provider.get_mw(cas).unwrap_or(0.0);

    let mut combined: HashMap<String, f64> = HashMap::new();
    for stream in inputs.values() {
        for (cas, cf) in &stream.components {
            *combined.entry(cas.clone()).or_insert(0.0) += cf.mass_flow;
        }
    }

    combined.retain(|_, v| *v > 0.0);

    let out = MaterialStream::from_mass_map(combined, &mw_lookup);

    Ok(vec![("out".to_string(), out)])
}
