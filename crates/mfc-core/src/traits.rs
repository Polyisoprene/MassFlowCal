use crate::component::ComponentDef;
use crate::error::EngineError;

/// 组分数据提供者接口
///
/// 引擎通过此 trait 查询分子量等组分数据，而非依赖具体数据库实现。
/// 这使得引擎可用于 SQLite、内存存储、REST API 等任意后端。
pub trait ComponentProvider {
    /// 查询组分的分子量 (g/mol)
    fn get_mw(&self, cas: &str) -> Result<f64, EngineError>;
    /// 查询单个组分完整信息
    fn get_component(&self, cas: &str) -> Result<Option<ComponentDef>, EngineError>;
    /// 获取全部组分列表
    fn get_all_components(&self) -> Result<Vec<ComponentDef>, EngineError>;
}
