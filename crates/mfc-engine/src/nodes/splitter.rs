use std::collections::HashMap;
use mfc_core::error::EngineError;
use mfc_core::graph::SplitterParams;
use mfc_core::stream::MaterialStream;
use mfc_core::traits::ComponentProvider;

use super::NodeOutputs;

pub fn compute_splitter(
    params: &SplitterParams,
    inputs: &HashMap<String, MaterialStream>,
    provider: &dyn ComponentProvider,
) -> Result<NodeOutputs, EngineError> {
    let feed = inputs.values().next()
        .ok_or_else(|| EngineError::MissingInput {
            node: "Splitter".into(),
            port: "feed".into(),
        })?;

    let sum: f64 = params.splits.iter().sum();
    if (sum - 1.0).abs() > 0.001 {
        return Err(EngineError::SplitFractionSum(sum));
    }

    let mut outputs: NodeOutputs = Vec::new();
    let mw_lookup = |cas: &str| provider.get_mw(cas).unwrap_or(0.0);

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
