# DECISIONS — bcli Rust 移植决策记录（ADR）

只记录已与用户达成的决策；变更走新 ADR 顶替旧 ADR，不重编号。

## ADR-001 目标：整应用移植为纯 Rust CLI
Status: accepted (2026-11-19)
Context: bcli 为单文件 Python（1543 行）；用户要求用 rust-anything 全应用流程重写为 Rust。
Decision: 终态是独立 Rust 二进制；`bilibili_cli.py` 移植完成后移入 `legacy/` 仅作参照，不删。
Consequences: 走 record → front-door → 单元并行 → wire → lift；任何中间点 bcli 可用。

## ADR-002 等价标准：语义 + 结构级
Status: accepted (2026-11-19)
Context: 录制面离线可判定。
Decision: `--json` 字段名/类型/嵌套顺序、退出码（0/2/3/5/6/7/130）、help 文案、
报错分层文案逐字一致；不追求浮点/时间的字节巧合。
Consequences: diff/gate 是判据；任何差异必须显式列入「已知差异」并经用户认可。

## ADR-003 用例策略：离线录制 + 在线冒烟
Status: accepted (2026-11-19)
Decision: 单元与 app 录制只覆盖离线可判定面（help / 参数错 / 退出码 / 纯函数边界）；
search/list/info 各冒烟 1 次，dl 走音频小样本 + 续传 + 半成品删除，login 由用户扫码。
Consequences: 联网输出永不进 cases；风控/网络问题在冒烟层暴露。

## ADR-004 传输：shell 出系统 curl
Status: accepted (2026-11-19)
Decision: Rust 用 `std::process::Command` 调系统 curl（含 `--http2`、`--compressed`、
`-C -` 续传），不用 reqwest/rustls。
Consequences: TLS 指纹与现版完全同路径；app 无异步运行时。

## ADR-005 gRPC：手写 protobuf，走 curl
Status: accepted (2026-11-19)
Decision: protobuf wire 编解码手写（proto 单元 crate），传输仍是 curl `--http2`；
不引入 tonic/tokio。
Consequences: 字段表（README「数据来源」节）是唯一契约。

## ADR-006 依赖：app 级只 clap + serde + serde_json(preserve_order) + md-5
Status: accepted (2026-11-19)（修订：补 md-5）
Context: 答复 Q5 时未预见 appkey 签名需要 md5。
Decision: app crate 依赖上限即上述四项；日期/时长等手写算法，不引 chrono。
单元 crate 移植期可用 bridge 生成的 pyo3/serde 脚手架，lift 后消失。
Consequences: 新增任何 crate 都要先报批（对应 AGENTS.md 变更纪律）。

## ADR-007 布局与入口
Status: accepted (2026-11-19)
Decision: 单元 crate 在 `rust/<module>`；app crate 在 `rust/app`（二进制名 `bcli`）；
app 级录制用例在 `rust/app-cases`；`bcli` 启动脚本优先执行
`rust/app/target/release/bcli`，无构建产物时回落 `python3 bilibili_cli.py`。
Consequences: 用户 PATH 无感切换；构建产物不入库。

## ADR-008 兼容：登录态与产物
Status: accepted (2026-11-19)
Decision: `~/.bilibili_cli/creds.json` 路径/权限(600)/字段不变；cookie jar 解析语义
（`#HttpOnly_` 前缀）保留；产物目录 `~/Documents/bilibili_content/` 与
`.info.json` schema `bilibili_cli.v1` 字段不增不删。
Consequences: 移植期新旧实现可交替使用同一登录态。

## ADR-009 并行协议（含验收标准制度）
Status: accepted (2026-11-19)
Decision: wave 1 四个纯函数 crate 并行移植，**每个模块一个常驻 pi 会话**
（rex 标签页，本会话从头跟到 lift）。会话禁区：git（协调者统一提交）、
共享文件（MAP/DECISIONS/GLOSSARY/tasks）、原文件、其他模块目录、
真实凭据 `~/.bilibili_cli/`、联网。每模块的任务书（`rust/tasks/<module>.md`）
必须含**明确目标 + 验收标准**（diff N/N、gate 全绿、错误路径 Err、
边界用例清单、汇报格式），完成即汇报等接线，不自行 wire。
Consequences: 认领预先写在 MAP.md；会话只读认领不写 MAP。

## ADR-010 波次
Status: accepted (2026-11-19)
Decision: wave 0 地基（用例录制 + front-door + 共享文档）由协调者做；
wave 1 单元并行（proto/parse/play/sign）；wave 2 命令胶水（协调者，可派会话）；
wave 3 冒烟（含用户扫码的 login 里程碑）；全部通过后 lift。
Consequences: 每波结束向用户汇报 gate/diff/冒烟结果。

## ADR-011 范围：不加功能
Status: accepted (2026-11-19)
Decision: 只做等价移植；不新增命令/参数/字段；PGC、web 搜索通道、订阅仍不做。
Consequences: 与 AGENTS.md 变更纪律一致。

## ADR-012 版本库：每工具一个仓库
Status: accepted (2026-11-19)
Context: 用户确认（曾误建根级仓库，已拆除重排）。
Decision: `moon_cli_or_tool/<tool>/` 各自独立 git 仓库；bupt-cli 保留原有历史。
`rust/` 目录（含 cases/fixtures/tasks/docs）全量入库，`target/` 除外。
Consequences: 根目录 AGENTS.md 为不入库的共享说明。

## ADR-013 并行会话模型：opencode-go/glm-5.3:max
Status: accepted (2026-11-19)
Context: 用户指定；wave 1 的四个会话已由用户手动切换到该模型。
Decision: 本移植的所有 subagent / rex 会话（含后续 wave 的接线与修复会话）
统一使用 `opencode-go/glm-5.3:max`，新会话启动时显式传模型
（`pi --model opencode-go/glm-5.3:max`），不依赖默认路由。
Consequences: 协调者（主会话）模型不受此约束；此条同时已写入根 AGENTS.md。


## ADR-014 dl 新增 --parallel 分段并行下载
Status: accepted (2026-11-19)
Context: 用户反馈长视频下载慢；实测单连接被 CDN 压速（当晚 ~2MB/s），
4 段 Range 并行 ~19MB/s。探测矩阵：4/6/8 并行收益 1.9-2.2×（管道饱和），
被压速场景收益按倍数放大（实测同一 176MB 视频 89.3s → 9.6s，9.3×）。
Decision: `--parallel N`（默认 4，0/1=单流，上限 8）；流 ≥32MB 才分段（音频同规则）；
Range 0-0 探测大小，无 Range 支持回退单流；任一段 3 次整段重试失败或拼接异常
→ 删全部半成品 → 回退单流（保留「绝不留坏文件」承诺）。
分段 md5 与单流逐字节一致（176.7MB 实测同 md5）。
Consequences: 这是唯一超出 Python 原版的功能（用户批准）；
**lift 时须同步更新 app-cases 里 dl 的 usage 行与 help-dl 文本**（加 --parallel），
以及 argparse 模拟层用新 usage 行——已记入 MAP 待办。

## ADR-016 Python 原版移至 legacy-python 分支
Status: accepted (2026-11-20)
Context: lift 完成后原版仅作参照；用户指示删出 main、放独立分支。
Decision: 分支 legacy-python 指向 c88cd8a（纯 Python 时代的完整仓库）；
main 移除 legacy/ 目录，bcli shim 不再回落 Python（未构建时提示构建命令）。
Consequences: main 是纯 Rust 单一实现；需要对照 Python 行为时 checkout legacy-python。
