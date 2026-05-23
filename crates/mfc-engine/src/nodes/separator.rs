use std::collections::HashMap;
use mfc_core::error::EngineError;
use mfc_core::graph::{FlowBasis, SeparatorKind, SeparatorParams, SplitMode};
use mfc_core::stream::MaterialStream;
use mfc_core::traits::ComponentProvider;

use super::NodeOutputs;

pub fn compute_separator(
    params: &SeparatorParams,
    inputs: &HashMap<String, MaterialStream>,
    provider: &dyn ComponentProvider,
) -> Result<NodeOutputs, EngineError> {
    let feed = inputs.values().next()
        .ok_or_else(|| EngineError::MissingInput {
            node: "Separator".into(),
            port: "feed".into(),
        })?;

    let (out1_name, out2_name) = match params.separator_kind {
        SeparatorKind::GasLiquid => ("gas", "liquid"),
        SeparatorKind::GasSolid => ("gas", "solid"),
        SeparatorKind::General => ("outlet_1", "outlet_2"),
    };

    let mw_lookup = |cas: &str| provider.get_mw(cas).unwrap_or(0.0);

    let mut out1_masses: HashMap<String, f64> = HashMap::new();
    let mut out2_masses: HashMap<String, f64> = HashMap::new();

    match &params.split_mode {
        SplitMode::Fraction { fractions } => {
            for (cas, cf) in &feed.components {
                let frac = fractions.get(cas).copied().unwrap_or(0.0);
                if frac < 0.0 || frac > 1.0 {
                    return Err(EngineError::Computation(
                        format!("组分 {} 的分割分率 {} 超出 0~1 范围", cas, frac)
                    ));
                }
                out1_masses.insert(cas.clone(), cf.mass_flow * frac);
                out2_masses.insert(cas.clone(), cf.mass_flow * (1.0 - frac));
            }
            for (cas, cf) in &feed.components {
                if !fractions.contains_key(cas) {
                    out1_masses.insert(cas.clone(), 0.0);
                    out2_masses.insert(cas.clone(), cf.mass_flow);
                }
            }
        }
        SplitMode::FlowRate { flow_basis, flow_rates } => {
            for (cas, cf) in &feed.components {
                let split_amount = flow_rates.get(cas).copied().unwrap_or(0.0);
                let split_mass = match flow_basis {
                    FlowBasis::Mass => split_amount,
                    FlowBasis::Mole => {
                        let mw = provider.get_mw(cas)?;
                        split_amount * mw
                    }
                };

                if split_mass > cf.mass_flow + 0.001 {
                    return Err(EngineError::Computation(
                        format!("组分 {} 的分割流量 {} kg/h 超过进料流量 {} kg/h",
                            cas, split_mass, cf.mass_flow)
                    ));
                }

                let actual_split = if split_mass > cf.mass_flow { cf.mass_flow } else { split_mass };
                out1_masses.insert(cas.clone(), actual_split);
                out2_masses.insert(cas.clone(), cf.mass_flow - actual_split);
            }
        }
    }

    out1_masses.retain(|_, v| *v > 0.0);
    out2_masses.retain(|_, v| *v > 0.0);

    let stream1 = MaterialStream::from_mass_map(out1_masses, &mw_lookup);
    let stream2 = MaterialStream::from_mass_map(out2_masses, &mw_lookup);

    Ok(vec![
        (out1_name.to_string(), stream1),
        (out2_name.to_string(), stream2),
    ])
}
