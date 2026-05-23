//! 项目文件序列化与 CSV 导出

use std::path::Path;

use mfc_core::component::ComponentDef;
use mfc_core::graph::FlowGraph;
use mfc_core::stream::MaterialStream;
use mfc_core::error::EngineError;

/// 项目文件的 JSON 容器
#[derive(serde::Serialize, serde::Deserialize)]
pub struct ProjectFile {
    pub graph: FlowGraph,
    pub project_components: Vec<ComponentDef>,
}

/// 保存项目为 JSON 文件
pub fn save_project(path: &Path, graph: &FlowGraph, comps: &[ComponentDef]) -> Result<(), EngineError> {
    let proj = ProjectFile { graph: graph.clone(), project_components: comps.to_vec() };
    let json = serde_json::to_string_pretty(&proj)
        .map_err(|e| EngineError::Computation(format!("序列化失败: {}", e)))?;
    std::fs::write(path, &json)
        .map_err(|e| EngineError::Computation(format!("保存失败: {}", e)))?;
    Ok(())
}

/// 从 JSON 文件加载项目
pub fn load_project(path: &Path) -> Result<(FlowGraph, Vec<ComponentDef>), EngineError> {
    let json = std::fs::read_to_string(path)
        .map_err(|e| EngineError::Computation(format!("读取失败: {}", e)))?;
    let proj: ProjectFile = serde_json::from_str(&json)
        .map_err(|e| EngineError::Computation(format!("解析失败: {}", e)))?;
    Ok((proj.graph, proj.project_components))
}

/// 导出计算结果为 CSV 字符串（UTF-8 BOM）
pub fn export_csv(
    streams: &[(String, &MaterialStream)],
    component_names: &dyn Fn(&str) -> String,
) -> Result<String, EngineError> {
    if streams.is_empty() {
        return Ok(String::new());
    }

    let mut all_comps: Vec<String> = Vec::new();
    for (_, s) in streams.iter() {
        for cas in s.components.keys() {
            if !all_comps.contains(cas) {
                all_comps.push(cas.clone());
            }
        }
    }

    fn fmt_num(v: f64) -> String {
        if v.abs() < 0.0001 { String::new() } else { format!("{:.2}", v) }
    }

    let mut csv = String::from("\u{FEFF}");

    csv.push_str("物料平衡计算结果\n");
    csv.push(',');
    for (name, _) in streams.iter() {
        csv.push_str(&format!("{},", name));
    }
    csv.push('\n');

    csv.push_str("流股号,");
    for i in 1..=streams.len() {
        csv.push_str(&format!("{},", i));
    }
    csv.push('\n');

    csv.push_str("摩尔流量(kmol/h),");
    for (_, s) in streams.iter() {
        csv.push_str(&format!("{},", fmt_num(s.mole_flow)));
    }
    csv.push('\n');
    for cas in &all_comps {
        let name = component_names(cas);
        csv.push_str(&format!("{},", name));
        for (_, s) in streams.iter() {
            csv.push_str(&format!("{},", fmt_num(s.components.get(cas).map(|cf| cf.mole_flow).unwrap_or(0.0))));
        }
        csv.push('\n');
    }

    csv.push('\n');

    // 摩尔分率
    csv.push_str("摩尔分率(%),");
    for (_, _) in streams.iter() { csv.push(','); }
    csv.push('\n');
    for cas in &all_comps {
        let name = component_names(cas);
        csv.push_str(&format!("{},", name));
        for (_, s) in streams.iter() {
            let frac = s.components.get(cas).map(|cf| cf.mole_fraction * 100.0).unwrap_or(0.0);
            csv.push_str(&format!("{:.1}%,", frac));
        }
        csv.push('\n');
    }

    csv.push('\n');

    csv.push_str("质量流量(kg/h),");
    for (_, s) in streams.iter() {
        csv.push_str(&format!("{},", fmt_num(s.mass_flow)));
    }
    csv.push('\n');
    for cas in &all_comps {
        let name = component_names(cas);
        csv.push_str(&format!("{},", name));
        for (_, s) in streams.iter() {
            csv.push_str(&format!("{},", fmt_num(s.components.get(cas).map(|cf| cf.mass_flow).unwrap_or(0.0))));
        }
        csv.push('\n');
    }

    csv.push('\n');

    // 质量分率
    csv.push_str("质量分率(%),");
    for (_, _) in streams.iter() { csv.push(','); }
    csv.push('\n');
    for cas in &all_comps {
        let name = component_names(cas);
        csv.push_str(&format!("{},", name));
        for (_, s) in streams.iter() {
            let frac = s.components.get(cas).map(|cf| cf.mass_fraction * 100.0).unwrap_or(0.0);
            csv.push_str(&format!("{:.1}%,", frac));
        }
        csv.push('\n');
    }

    Ok(csv)
}
