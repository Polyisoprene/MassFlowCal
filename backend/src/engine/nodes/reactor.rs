use std::collections::HashMap;
use crate::db::Database;
use crate::error::EngineError;
use crate::models::graph::ReactorParams;
use crate::models::stream::MaterialStream;

use super::NodeOutputs;

pub fn compute_reactor(
    params: &ReactorParams,
    inputs: &HashMap<String, MaterialStream>,
    db: &Database,
) -> Result<NodeOutputs, EngineError> {
    let feed = inputs.values().next()
        .ok_or_else(|| EngineError::MissingInput {
            node: "Reactor".into(),
            port: "feed".into(),
        })?;

    // 以摩尔基准计算反应
    let mut out_mole: HashMap<String, f64> = feed.components
        .iter()
        .map(|(cas, cf)| (cas.clone(), cf.mole_flow))
        .collect();

    for reaction in &params.reactions {
        // 关键组分的进口摩尔流量
        let key_in_mole = out_mole.get(&reaction.key_component).copied().unwrap_or(0.0);
        if key_in_mole <= 0.0 {
            continue; // 关键组分不存在，跳过
        }

        // 反应量 = 关键组分进口 × 转化率
        let reacted = key_in_mole * reaction.conversion;

        // 关键组分消耗量与化学计量系数的比例因子
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

        // 按比例计算其余组分的变化量
        for (cas, coeff) in &reaction.stoichiometry {
            let delta_mole = scale * coeff;
            *out_mole.entry(cas.clone()).or_insert(0.0) += delta_mole;
        }
    }

    // 过滤掉 ≤ 0 的组分
    out_mole.retain(|_, v| *v > 0.0);

    // 检查负流量
    for (cas, mole) in &out_mole {
        if *mole < 0.0 {
            return Err(EngineError::NegativeFlow {
                node: "Reactor".into(),
                cas: cas.clone(),
                value: *mole,
            });
        }
    }

    // 转为质量基准
    let out_mass: HashMap<String, f64> = out_mole.iter()
        .map(|(cas, mole)| {
            let mw = db.get_mw(cas).unwrap_or(0.0);
            let mass = mole * mw; // kmol/h → kg/h
            (cas.clone(), mass)
        })
        .collect();

    let outlet = MaterialStream::from_mass_map(out_mass, &|cas| db.get_mw(cas).unwrap_or(0.0));

    Ok(vec![("outlet".to_string(), outlet)])
}
