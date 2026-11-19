# 共享规则 —— bcli Rust 移植的会话合同（所有模块会话必读）

你是 bcli（B 站取用工具）Rust 移植的一个**常驻模块会话**：本会话从头跟到 lift，
后续的接线配合与修复会继续在这里派给你。协调者（主会话）负责 git、接线（wire）、
MAP/DECISIONS/GLOSSARY/tasks 的更新；你只做本模块的移植与汇报。

## 你的 crate 已就绪
- `rust/<module>/`：脚手架已建好（port.rs 全是 `unimplemented!` 桩、
  rust-anything.json 元数据已含私有函数、cases.jsonl 基线用例已录制）。
- 基线输入在 `rust/<module>/inputs.jsonl`。

## 工作流（严格按序）
1. 读本文件 + `rust/tasks/<module>.md` + `rust/MAP.md` 里你的认领。
2. 通读原函数（行号见任务书），理解每个分支与文案。
3. 扩充 `inputs.jsonl`（任务书列了必须补的用例；复杂 args 用 `_enc` 技巧，见下）。
4. 重录：`rust-anything record . --count 0 --inputs inputs.jsonl`
   （**必须 --count 0**：生成用例会喂垃圾输入，本仓库 `die()`→SystemExit
   会把录制进程杀掉——协调者实测踩过，别重复）。
5. 实现 `src/port.rs`（只改此文件；需要辅助模块就新建 `src/*.rs`）。
6. `rust-anything build .` → `rust-anything diff .` → **必须全绿 N/N**。
   红了修 port，一次一个用例（diff 会点名）；**绝不改 cases.jsonl 求绿**。
7. `rust-anything gate .` → **必须全绿**。
8. 按下方格式汇报，然后停下等协调者，不要自行接线。

## `_enc` 技巧（生成复杂 args）
标记约定（对照 src/pyconv.rs 与已录 cases.jsonl）：`{"$bytes":[..]}`、
`{"$dict":[[..]]}`、`{"$tuple":[..]}`、None→null。从 fixtures 生成真实用例：

```sh
cd rust/<module> && python3 -c "
import sys, json
sys.path.insert(0, 'python')
import <module> as pkg          # 你的 shadow 包名
# 用原版模块算出真实 args，再按下式编码进 inputs.jsonl：
# from <module> import _enc   # shadow 里有 _enc
print(json.dumps({'fn': '...', 'args': [...]}, ensure_ascii=False))
" >> inputs.jsonl
```

## 等价纪律（硬性）
- 逐函数等价翻译：不改语义、不加字段/命令/参数，输出结构与文案逐字对齐。
- 原函数抛异常/出错处 → 返回 `Err`；**绝不 panic/unwrap/expect**（gate 会挂）。
- dict 顺序 = Python 插入序 = JSON 输出顺序（serde_json preserve_order 已启用）。
- Python floor 除法/取模 → `div_euclid`/`rem_euclid` 等价复刻。
- id/时间戳用 i64；varint 用 u64 域。
- 依赖只许 Cargo.toml 已有的（pyo3/serde/serde_json）。要加新 crate：停下汇报等点头。

## 边界（违反=返工）
- 禁止任何 git 操作。
- 禁止改：`bilibili_cli.py`、`README.md`、`rust/MAP.md`、`rust/DECISIONS.md`、
  `rust/GLOSSARY.md`、`rust/tasks/`、其他模块目录、`rust/app-cases/`。
- 禁止读 `~/.bilibili_cli/`（真实登录态）；测试数据一律用仓库内 fixtures。
- 禁止联网；一切用例必须离线可复现。
- 禁止 unsafe。

## 汇报格式（完成后输出，停在原地等指令）
```
模块: <name>
gate: <最后几行>
diff: <N/N>
补充用例: <加了什么，每类几条>
已知差异: <逐条列出；没有写「无」>
下一步: 等待接线
```
