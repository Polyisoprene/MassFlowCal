use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// 物料流: 同时持有质量流量和摩尔流量
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct MaterialStream {
    /// 总质量流量 kg/h
    pub mass_flow: f64,
    /// 总摩尔流量 kmol/h
    pub mole_flow: f64,
    /// CAS号 → 组分流量明细
    pub components: HashMap<String, ComponentFlow>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComponentFlow {
    /// CAS 号
    pub cas: String,
    /// 质量流量 kg/h
    pub mass_flow: f64,
    /// 摩尔流量 kmol/h
    pub mole_flow: f64,
    /// 质量分率 (0~1)
    pub mass_fraction: f64,
    /// 摩尔分率 (0~1)
    pub mole_fraction: f64,
}

impl MaterialStream {
    /// 从单一纯物质质量流量创建
    pub fn from_pure_mass(cas: &str, mass_flow: f64, mw: f64) -> Self {
        let mole_flow = mass_flow / mw; // kg/h → kmol/h (MW = kg/kmol)
        let comp = ComponentFlow {
            cas: cas.to_string(),
            mass_flow,
            mole_flow,
            mass_fraction: 1.0,
            mole_fraction: 1.0,
        };
        Self {
            mass_flow,
            mole_flow,
            components: HashMap::from([(cas.to_string(), comp)]),
        }
    }

    /// 从单一纯物质摩尔流量创建
    pub fn from_pure_mole(cas: &str, mole_flow: f64, mw: f64) -> Self {
        let mass_flow = mole_flow * mw; // kmol/h → kg/h (MW = kg/kmol)
        let comp = ComponentFlow {
            cas: cas.to_string(),
            mass_flow,
            mole_flow,
            mass_fraction: 1.0,
            mole_fraction: 1.0,
        };
        Self {
            mass_flow,
            mole_flow,
            components: HashMap::from([(cas.to_string(), comp)]),
        }
    }

    /// 从多组分质量流量 map 创建
    pub fn from_mass_map(component_masses: HashMap<String, f64>, mw_lookup: &dyn Fn(&str) -> f64) -> Self {
        let mut components = HashMap::new();
        let total_mass: f64 = component_masses.values().sum();
        let mut total_mole = 0.0f64;

        for (cas, mass) in &component_masses {
            let mw = mw_lookup(cas);
            let mole = mass / mw; // kg/h → kmol/h
            total_mole += mole;
            components.insert(
                cas.clone(),
                ComponentFlow {
                    cas: cas.clone(),
                    mass_flow: *mass,
                    mole_flow: mole,
                    mass_fraction: if total_mass > 0.0 { mass / total_mass } else { 0.0 },
                    mole_fraction: 0.0, // filled below
                },
            );
        }

        // 填充分率
        for comp in components.values_mut() {
            comp.mole_fraction = if total_mole > 0.0 { comp.mole_flow / total_mole } else { 0.0 };
        }

        Self { mass_flow: total_mass, mole_flow: total_mole, components }
    }

    /// 检查是否为空流
    pub fn is_empty(&self) -> bool {
        self.mass_flow == 0.0 && self.mole_flow == 0.0
    }
}
