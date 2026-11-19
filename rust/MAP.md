# MAP — bcli → Rust 移植状态板

决策见 `DECISIONS.md`，术语见 `GLOSSARY.md`，任务书（目标+验收标准）见 `tasks/`。
**维护者：协调者会话。模块会话不编辑本文件**，只在会话里汇报，由协调者回填。

## wave 1 单元（4 个纯函数 crate，并行，每个一个常驻 rex 会话）

| 模块 | crate | 状态 | 备注 |
|---|---|---|---|
| proto | rust/proto | **done** 2026-11-19（gate ✓ · diff 80/80） | 13 函数；真实帧 cargo 测试过；`_pi` 负数 = 原版死循环 → port 返回 Err（已列已知差异） |
| parse | rust/parse | **done** 2026-11-19（gate ✓ · diff 95/95） | 6 函数；parse_target 失败路径由 app 层用例覆盖 |
| play  | rust/play  | **done** 2026-11-19（gate ✓ · diff 41/41） | 5 函数；CliError 异常类用 create_exception! 对齐类型名 |
| sign  | rust/sign  | **done** 2026-11-19（gate ✓ · diff 10/10） | 3 函数；手写 MD5（含已知向量测试）消除 md-5 外部依赖；sign_app 走固定 ts 基准 |

状态流：`open` → `claimed by <会话> <日期>` → `done`（回填 gate/diff 结果）。

## wave 2 胶水（协调者写，存 rust/app/src/）
- ✅ **search 已接线**（units/proto.rs + glue/{err,curl,grpc}.rs）：
  真实冒烟 py/rust 配对 10+ 次全一致（--json / 人类输出 / --users --desc / --sort new / 空结果）；
  回落日志为空（纯 Rust 路径）；kill switch `RUST_ANYTHING_UNIT_PROTO=original` 有效。
  注意：app-cases 里的 search 用例全是参数错路径，仍由 Python 回落答（符合 ADR-003），
  search 的真实行为由冒烟配对作证。
- 待做：curl 封装已就绪，待接 list / info / dl / doctor / login；
  view/user_videos/user_name；playurl；_curl_dl/_merge；_render_qr；load_creds/save_creds；
  cmd_list/info/dl/doctor + clap 补全 + HELP 文本。

## wave 3 冒烟（协调者跑，用户配合一次）
search 1 次 / `list --json --rights` 1 次 / info 1 次 / doctor；
`dl --audio` 小样本 + 断点续传 + 半成品删除；login（网页+TV，**用户扫码**）。

## lift 条件
fallback 率 0 + 4 单元 gate 全绿 + 冒烟全过 → `rust/app` 纯化 →
`bilibili_cli.py` 移 `legacy/`，README 重写。

## 已解决的坑（别再踩）
- **搜索排序会实时抖动**（totalrank 第 2 条起会变，py 自比也不稳定）：
  验收一律用「配对对比」，勿单次 diff 下结论。
- record 生成用例喂垃圾输入会触发 `die()`→SystemExit 杀录制进程
  → 一律 `--count 0 --inputs inputs.jsonl`。
- bridge 模板 lib 名默认 `bilibili_cli_rs` → 已改 `[lib] name = <module>`。
- 私有函数（`_` 开头）不在 bridge exports → 元数据已手工补，**别动 rust-anything.json**。
- `sign_app` 内部读时钟 → 固定 ts 基准（`rust/sign/fixtures/sign_fixed_ts.json`）。
