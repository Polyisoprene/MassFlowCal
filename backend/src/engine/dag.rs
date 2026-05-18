use petgraph::graph::{DiGraph, NodeIndex};
use petgraph::algo::toposort;
use std::collections::HashMap;
use uuid::Uuid;

use crate::error::EngineError;
use crate::models::graph::FlowGraph;

type DagGraph = DiGraph<Uuid, ()>;

/// 从 FlowGraph 构建 DAG，返回 (图, node_id → NodeIndex 映射)
pub fn build_dag(graph: &FlowGraph) -> Result<(DagGraph, HashMap<Uuid, NodeIndex>), EngineError> {
    let mut dag = DiGraph::<Uuid, ()>::new();
    let mut node_map: HashMap<Uuid, NodeIndex> = HashMap::new();

    // 添加所有节点
    for node in &graph.nodes {
        let idx = dag.add_node(node.id);
        node_map.insert(node.id, idx);
    }

    // 添加边（连线方向: origin → target）
    for conn in &graph.connections {
        let origin = node_map.get(&conn.origin_node);
        let target = node_map.get(&conn.target_node);

        if let (Some(&o), Some(&t)) = (origin, target) {
            dag.add_edge(o, t, ());
        }
    }

    Ok((dag, node_map))
}

/// 拓扑排序，返回按执行顺序排列的 node_id 列表
pub fn compute_order(
    dag: &DagGraph,
    node_map: &HashMap<Uuid, NodeIndex>,
) -> Result<Vec<Uuid>, EngineError> {
    toposort(dag, None)
        .map_err(|_| EngineError::CycleDetected)
        .map(|order| {
            let reverse: HashMap<NodeIndex, Uuid> =
                node_map.iter().map(|(id, idx)| (*idx, *id)).collect();
            order.into_iter().map(|idx| reverse[&idx]).collect()
        })
}
