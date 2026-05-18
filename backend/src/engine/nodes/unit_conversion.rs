use std::collections::HashMap;
use crate::db::Database;
use crate::error::EngineError;
use crate::models::graph::UnitConversionParams;
use crate::models::stream::MaterialStream;

use super::NodeOutputs;

pub fn compute_unit_conversion(
    _params: &UnitConversionParams,
    inputs: &HashMap<String, MaterialStream>,
    db: &Database,
) -> Result<NodeOutputs, EngineError> {
    let feed = inputs.values().next()
        .ok_or_else(|| EngineError::MissingInput {
            node: "UnitConversion".into(),
            port: "input".into(),
        })?;

    // MaterialStream 已经同时持有 mass_flow 和 mole_flow
    // unit_conversion 只是透传（数据本身不变），标记目标基准
    // 实际换算在 feed 创建和后续节点中已通过 MW 自动完成
    let mw_lookup = |cas: &str| db.get_mw(cas).unwrap_or(0.0);

    // 按当前各组分质量流量重建流（确保一致性）
    let masses: HashMap<String, f64> = feed.components
        .iter()
        .map(|(cas, cf)| (cas.clone(), cf.mass_flow))
        .collect();

    let converted = MaterialStream::from_mass_map(masses, &mw_lookup);

    Ok(vec![("output".to_string(), converted)])
}
