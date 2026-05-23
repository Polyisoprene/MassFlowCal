use serde::{Deserialize, Serialize};

/// 数据库中存储的组分定义
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComponentDef {
    /// CAS 号，唯一主键 e.g. "7732-18-5"
    pub cas_number: String,
    /// 显示名称 e.g. "水"
    pub name: String,
    /// 分子式 e.g. "H2O"
    pub formula: String,
    /// 分子量 g/mol
    pub molecular_weight: f64,
}
