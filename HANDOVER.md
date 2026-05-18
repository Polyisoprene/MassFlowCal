# MassFlowCal 项目交接文档

## 1. 项目概述

MassFlowCal 是一个 Rust 原生桌面应用，用于化工流程的物料平衡计算。用户通过节点图编辑器搭建流程模型，系统自动按拓扑顺序计算各节点输出。

### 核心设计理念

- **Feed 节点 = 单一纯物质**，多组分通过 Mixer 混合
- **物料流同时持有质量和摩尔流量**，通过分子量数据库互算
- **组分以 CAS 号唯一标识**，存储在 SQLite 中
- **UI 对标 ComfyUI** 的节点图交互（右键菜单、拖拽连线、节点内参数编辑）

## 2. 技术架构

```
┌─────────────────────────────────────────┐
│              UI 层 (egui + egui-snarl)   │
│  app.rs: 应用状态、面板、窗口管理        │
│  canvas.rs: SnarlViewer 节点渲染         │
├─────────────────────────────────────────┤
│              计算引擎 (engine/)          │
│  mod.rs: DAG构建 → 拓扑排序 → 逐节点求值 │
│  nodes/: Feed/Reactor/Mixer/Splitter... │
├─────────────────────────────────────────┤
│              数据层                      │
│  models/: 图/流/组分 数据结构            │
│  db/: SQLite CRUD                       │
└─────────────────────────────────────────┘
```

### 关键技术决策

| 决策                                | 原因                                  |
| --------------------------------- | ----------------------------------- |
| egui-snarl v0.9                   | 原生支持节点图编辑器，右键菜单，拖拽连线                |
| NodeOutputs = Vec<(name, stream)> | HashMap 顺序不稳定导致连线端口映射错误，改用有序 Vec    |
| 全局物料平衡容差 = 总量×0.1%                | 浮点运算累积误差，绝对精度约 0.001%               |
| Feed 透传逻辑                         | 连接上游时取上游摩尔流量数值，按自身基准解释              |
| Serialize params as HashMap       | 保持 JSON 对象格式（`{"Feed":{...}}`），兼容文件 |

## 3. 数据模型

### 核心类型

**FlowGraph** — 完整的流程定义：

```rust
struct FlowGraph {
    id: Uuid,
    nodes: Vec<Node>,          // 节点列表
    connections: Vec<Connection>, // 连线列表
}
```

**Node** — 一个单元操作：

```rust
struct Node {
    id: Uuid,
    node_type: NodeType,       // Feed/Reactor/Mixer/...
    params: Option<NodeParams>, // 节点参数（Mixer/Product/Copy/Filter 为 None）
    inputs: Vec<Port>,         // 输入端口定义
    outputs: Vec<Port>,        // 输出端口定义
}
```

**MaterialStream** — 物料流（计算结果的载体）：

```rust
struct MaterialStream {
    mass_flow: f64,            // 总质量流量 kg/h
    mole_flow: f64,            // 总摩尔流量 kmol/h
    components: HashMap<String, ComponentFlow>, // CAS -> 组分流量
}
```

### 节点类型与参数映射

| NodeType       | NodeParams 变体                                        | 说明       |
| -------------- | ---------------------------------------------------- | -------- |
| Feed           | `Feed(cas, flow_basis, total_flow)`                  | 单一纯物质进料  |
| Product        | `None`                                               | 出料标记     |
| Reactor        | `Reactor(reactions: Vec<ReactionDef>)`               | 多反应定义    |
| Splitter       | `Splitter(splits: Vec<f64>)`                         | 出口分率列表   |
| Mixer          | `None`                                               | 混合器      |
| Separator      | `Separator(kind, split_mode)`                        | 气液/气固/通用 |
| Decanter       | `Decanter(partition_coefficients, phase_mass_ratio)` | 轻相分率     |
| Calculator     | `Calculator(operation)`                              | 四则运算     |
| UnitConversion | `UnitConversion(target_basis)`                       | 单位变换     |
| Copy           | `None`                                               | 流股复制     |
| Filter         | `None`                                               | 组分过滤     |

## 4. 计算引擎流程

```
1. 构建 DAG (petgraph)
   └── 每个节点一个 graph node，连线为 edge

2. 拓扑排序 (petgraph::algo::toposort)
   └── 检测环路 → 返回 CycleDetected 错误

3. 按序执行 compute_node()
   └── collect_inputs() 收集上游节点的输出
   └── 根据 NodeType 分发到具体计算函数
   └── 计算结果存入 ResultCache (HashMap<Uuid, Vec<(name, stream)>>)

4. 全局物料平衡校验
   └── Σ所有 Feed 输出质量 = Σ所有 Product 输入质量
   └── 容差 = max(总流量 × 0.001, 0.1) kg/h
```

### NodeOutputs 类型

**重要**：`NodeOutputs = Vec<(String, MaterialStream)>`，不是 HashMap。原因：

- HashMap 迭代顺序不稳定，导致 `collect_inputs()` 中 `origin_slot` 索引无法正确映射到端口
- Vec 保证插入顺序 = 输出端口定义顺序，`origin_slot=0` 总是第一个输出端口

### Feed 节点透传逻辑

当 Feed 节点有上游输入连接时：

1. 取上游的 `mole_flow` 数值
2. 使用 Feed 自身的 CAS 和 flow_basis
3. Mass 基准：`mass = upstream_mole_flow × MW`（取上游摩尔数作为质量数）
4. Mole 基准：`mole = upstream_mole_flow`（直接使用上游摩尔数）

## 5. UI 架构

### MassFlowApp (app.rs)

主应用状态，实现 `eframe::App` trait：

| 字段                                                         | 用途                     |
| ---------------------------------------------------------- | ---------------------- |
| `snarl: Snarl<MassFlowNode>`                               | 节点图数据（egui-snarl 管理）   |
| `graph_nodes: HashMap<usize, NodeData>`                    | snarl_id → 图节点+引擎节点的映射 |
| `engine: Engine`                                           | 计算引擎                   |
| `db_components / project_components`                       | 数据库组分 / 项目组分           |
| `compute_results`                                          | 最近一次计算结果               |
| `show_db_mgr / show_project_mgr / show_results / show_log` | 各窗口显隐标志                |

**关键方法**：

- `sync_from_snarl()` — 将 UI 编辑同步到引擎数据（保存前/计算前调用）
- `sync_params_from_snarl()` — 将编辑后的参数写回 Node 结构体
- `compute()` — 校验 + 构建 FlowGraph + 调用引擎
- `to_graph_node()` / `graph_node_to_massflow()` — MassFlowNode ↔ Node 互转

### MassFlowViewer (canvas.rs)

实现 `SnarlViewer<MassFlowNode>` trait：

| 方法                                   | 作用              |
| ------------------------------------ | --------------- |
| `title()`                            | 节点标题栏文字         |
| `inputs()/outputs()`                 | 返回端口数量          |
| `show_body()`                        | 渲染节点体内容（参数编辑控件） |
| `show_input()/show_output()`         | 端口样式（圆形，颜色区分）   |
| `has_graph_menu()/show_graph_menu()` | 右键画布 → 添加节点菜单   |
| `has_node_menu()/show_node_menu()`   | 右键节点 → 删除节点菜单   |

**辅助函数**：

- `narrow_combo()` — 紧凑型组分下拉框
- `draw_reaction()` — 单个反应的 UI 渲染
- `draw_split_editor()` — 分率/流量编辑（分离器/倾析器共用）

### 节点颜色方案

| 节点                | 颜色值            |
| ----------------- | -------------- |
| Feed 进料           | `#166534` 深绿   |
| Product 出料        | `#991b1b` 深红   |
| Reactor 反应器       | `#9a3412` 深橙   |
| Splitter 分流器      | `#6b21a8` 深紫   |
| Mixer 混合器         | `#4c1d95` 深紫罗兰 |
| Separator 分离器     | `#1e3a5f` 深蓝   |
| Decanter 倾析器      | `#155e75` 深青   |
| Calculator 计算器    | `#374151` 深灰   |
| UnitConversion 变换 | `#854d0e` 深黄   |
| Copy 复制           | `#0d5e5e` 深青绿  |
| Filter 过滤         | `#5e0d0d` 深暗红  |

## 6. 数据库

### 表结构

```sql
CREATE TABLE components (
    cas_number    TEXT PRIMARY KEY,  -- CAS 号，如 "7732-18-5"
    name          TEXT NOT NULL,     -- 中文名，如 "水"
    formula       TEXT NOT NULL,     -- 分子式，如 "H2O"
    mol_weight    REAL NOT NULL,     -- 分子量 g/mol
    created_at    TEXT
);
```

### 预置数据

20 种常见化工组分（水、乙醇、甲醇、乙酸、NaCl、H2SO4、NaOH、N2、O2、CO2、乙烯、乙炔、乙烷、甲烷、苯、甲苯、苯乙烯、乙醛、丙酮、乙酸乙酯）

数据库文件默认路径：`./massflowcal.db`

## 7. 文件保存格式

项目保存为 JSON（`.mfc` 文件）：

```json
{
  "graph": {
    "id": "00000000-...",
    "name": "current",
    "nodes": [...],
    "connections": [...]
  },
  "project_components": [...]
}
```

### 版本兼容注意事项

- `Decanter::partition_coefficients` 在旧版本中存储的是分配系数 K，当前版本存储的是直接分率（0~1）
- `phase_mass_ratio` 用作模式标志：`≥0` 表示分率模式，`<0` 表示流量模式
- `Mixer/Product/Copy/Filter` 的 `params` 为 `null`
- 其他节点的 `params` 为 `{"TypeName": {...}}` 外部标签格式

## 8. 已知限制和待改进

| 限制              | 影响               | 改进方向                         |
| --------------- | ---------------- | ---------------------------- |
| 不支持循环流          | 无法模拟带回流工艺        | 实现 Tear Stream + Wegstein 迭代 |
| HashMap 迭代顺序不稳定 | 需要 sort 保证 UI 稳定 | 改用 IndexMap                  |
| 没有撤销/重做         | 误操作无法恢复          | egui-snarl 自带 undo 支持        |
| 节点参数校验有限        | 可能输入非法值          | 增强前端校验                       |
| 无多语言支持          | UI 固定中文          | 国际化                          |

## 9. 构建与运行

```bash
# 开发构建
cd backend && cargo build

# 发布构建
cargo build --release

# 设置日志级别
$env:RUST_LOG="debug"; cargo run

# 运行
cargo run
```

## 10. 开发环境

| 工具    | 版本                   |
| ----- | -------------------- |
| Rust  | 1.95+ (edition 2021) |
| Cargo | 1.95+                |
| 操作系统  | Windows 10/11        |

### 主要依赖版本

| crate            | 版本       | 用途        |
| ---------------- | -------- | --------- |
| eframe/egui      | 0.33     | GUI 框架    |
| egui-snarl       | 0.9      | 节点图编辑器    |
| rusqlite         | 0.32     | SQLite 操作 |
| petgraph         | 0.7      | 图算法       |
| serde/serde_json | 1        | 序列化       |
| rfd              | 0.15     | 文件对话框     |
| uuid             | 1        | 节点 ID 生成  |
| thiserror        | 2        | 错误类型      |
| env_logger/log   | 0.11/0.4 | 日志        |
