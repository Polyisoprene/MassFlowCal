use thiserror::Error;

#[derive(Debug, Error)]
pub enum EngineError {
    #[error("图中存在环路")]
    CycleDetected,

    #[error("节点 '{node}' 的输入端口 '{port}' 缺少上游连接")]
    MissingInput { node: String, port: String },

    #[error("分流比例之和为 {0}，必须等于 1.0")]
    SplitFractionSum(f64),

    #[error("CAS '{0}' 在组分数据库中不存在")]
    UnknownComponent(String),

    #[error("节点 '{node}': 组分 {cas} 流量为负值 ({value})")]
    NegativeFlow { node: String, cas: String, value: f64 },

    #[error("总物料不平衡: 输入={input} kg/h, 输出={output} kg/h, Δ={delta} kg/h")]
    OverallBalanceError { input: f64, output: f64, delta: f64 },

    #[error("数据库错误: {0}")]
    Database(String),

    #[error("计算错误: {0}")]
    Computation(String),

    #[error("节点 '{node}' 上游节点 '{upstream}' 未计算")]
    UpstreamNotComputed { node: String, upstream: String },
}
