# MassFlowCal — 物料平衡计算器

基于 Rust 的桌面端物料平衡计算软件，提供节点图编辑器（类似 ComfyUI 交互），支持多种化工单元操作节点的连线计算。

## 功能

- **节点图编辑** — 拖拽式节点画布，右键菜单添加节点，拖拽端口连线
- **9 种单元操作节点** — 进料、出料、反应器、分流器、混合器、分离器、倾析器、计算器、单位变换
- **3 种辅助节点** — 复制、过滤
- **组分数据库** — SQLite 存储，CAS 号唯一标识，支持增删改查
- **项目组分管理** — 每个项目独立选择参与计算的组分
- **实时物料平衡计算** — 支持多组分、质量/摩尔双基准
- **结果导出** — CSV 格式（Excel 兼容，UTF-8 BOM）
- **项目保存/加载** — JSON 格式持久化

## 构建

```bash
cd backend
cargo build --release
```

编译产物位于 `target/release/massflowcal.exe`

## 运行

```bash
cargo run --release
```

或直接双击 `target/release/massflowcal.exe`

## 技术栈

| 组件 | 技术 |
|------|------|
| GUI 框架 | egui + eframe |
| 节点图 | egui-snarl |
| 数据库 | SQLite (rusqlite) |
| 图算法 | petgraph (DAG + 拓扑排序) |
| 序列化 | serde + serde_json |
| 文件对话框 | rfd |
| 日志 | env_logger + log |

## 项目结构

```
MassFlowCal/
├── backend/
│   ├── Cargo.toml
│   ├── data/
│   │   └── seed.sql              # 预置 20 种常见化工组分
│   └── src/
│       ├── main.rs               # 程序入口，eframe 初始化
│       ├── config.rs             # 配置管理
│       ├── error.rs              # 错误类型定义
│       ├── db/mod.rs             # SQLite 数据库操作
│       ├── models/               # 数据模型
│       │   ├── component.rs      # 组分定义
│       │   ├── graph.rs          # 图结构、节点、连线、参数
│       │   └── stream.rs         # 物料流
│       ├── engine/               # 计算引擎
│       │   ├── mod.rs            # Engine 主入口 + DAG + 拓扑排序
│       │   ├── dag.rs            # 有向无环图构建
│       │   └── nodes/            # 各节点计算实现
│       └── ui/                   # 用户界面
│           ├── app.rs            # 主应用状态 + 所有面板/窗口
│           └── canvas.rs         # 节点画布渲染 + SnarlViewer
└── HANDOVER.md                   # 交接文档
```

## 许可证

MIT
