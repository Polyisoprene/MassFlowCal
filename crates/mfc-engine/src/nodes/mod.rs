pub mod feed;
pub mod reactor;
pub mod splitter;
pub mod mixer;
pub mod separator;
pub mod decanter;
pub mod calculator;

use std::collections::HashMap;
use uuid::Uuid;

use mfc_core::error::EngineError;
use mfc_core::graph::{Connection, Node};
use mfc_core::stream::MaterialStream;

/// 节点计算结果: 按输出端口顺序排列的 (port_name, stream)
pub type NodeOutputs = Vec<(String, MaterialStream)>;

/// 上游结果缓存: node_id → outputs
pub type ResultCache = HashMap<Uuid, NodeOutputs>;

/// 收集某节点的所有输入端口的物料流
pub fn collect_inputs(
    node: &Node,
    connections: &[Connection],
    results: &ResultCache,
) -> Result<HashMap<String, MaterialStream>, EngineError> {
    let mut inputs = HashMap::new();

    for conn in connections {
        if conn.target_node != node.id {
            continue;
        }
        let port_name = node.inputs
            .get(conn.target_slot)
            .map(|p| p.name.clone())
            .unwrap_or_else(|| format!("slot_{}", conn.target_slot));

        let upstream = results.get(&conn.origin_node).ok_or_else(|| {
            EngineError::UpstreamNotComputed {
                node: node.title.clone(),
                upstream: conn.origin_node.to_string(),
            }
        })?;

        // 用 origin_slot 索引上游的 Vec（有序）
        if let Some((_, stream)) = upstream.get(conn.origin_slot) {
            inputs.insert(port_name, stream.clone());
        }
    }

    Ok(inputs)
}
