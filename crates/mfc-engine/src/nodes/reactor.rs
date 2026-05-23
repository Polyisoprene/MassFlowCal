use std::collections::HashMap;
use mfc_core::error::EngineError;
use mfc_core::graph::ReactorParams;
use mfc_core::stream::MaterialStream;
use mfc_core::traits::ComponentProvider;

use super::NodeOutputs;

pub fn compute_reactor(
    params: &ReactorParams,
    inputs: &HashMap<String, MaterialStream>,
    provider: &dyn ComponentProvider,
) -> Result<NodeOutputs, EngineError> {
    let feed = inputs.values().next()
        .ok_or_else(|| EngineError::MissingInput {
            node: "Reactor".into(),
            port: "feed".into(),
        })?;

    let mut out_mole: HashMap<String, f64> = feed.components
        .iter()
        .map(|(cas, cf)| (cas.clone(), cf.mole_flow))
        .collect();

    for reaction in &params.reactions {
        let key_in_mole = out_mole.get(&reaction.key_component).copied().unwrap_or(0.0);
        if key_in_mole <= 0.0 {
            continue;
        }

        let reacted = key_in_mole * reaction.conversion;

        let key_coeff = reaction.stoichiometry
            .get(&reaction.key_component)
            .copied()
            .unwrap_or(-1.0);
        if key_coeff.abs() < 1e-10 {
            return Err(EngineError::Computation(
                "关键组分化学计量系数不能为0".into()
            ));
        }
        let scale = reacted / key_coeff.abs();

        for (cas, coeff) in &reaction.stoichiometry {
            let delta_mole = scale * coeff;
            *out_mole.entry(cas.clone()).or_insert(0.0) += delta_mole;
        }
    }

    out_mole.retain(|_, v| *v > 0.0);

    for (cas, mole) in &out_mole {
        if *mole < 0.0 {
            return Err(EngineError::NegativeFlow {
                node: "Reactor".into(),
                cas: cas.clone(),
                value: *mole,
            });
        }
    }

    let out_mass: HashMap<String, f64> = out_mole.iter()
        .map(|(cas, mole)| {
            let mw = provider.get_mw(cas).unwrap_or(0.0);
            let mass = mole * mw;
            (cas.clone(), mass)
        })
        .collect();

    let outlet = MaterialStream::from_mass_map(out_mass, &|cas| provider.get_mw(cas).unwrap_or(0.0));

    Ok(vec![("outlet".to_string(), outlet)])
}
