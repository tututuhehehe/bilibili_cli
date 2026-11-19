# MAP — bcli → Rust 移植状态板

决策见 `DECISIONS.md`，术语见 `GLOSSARY.md`，任务书（目标+验收标准）见 `tasks/`。
**维护者：协调者会话。模块会话不编辑本文件**，只在会话里汇报，由协调者回填。

## wave 1 单元（4 个纯函数 crate，并行，每个一个常驻 rex 会话）

| 模块 | crate | 状态 | 备注 |
|---|---|---|---|
| proto | rust/proto | claimed 2026-11-19 | 13 函数；真实帧 fixtures；`_pi` 负数=原版死循环（允许 Err+注明） |
| parse | rust/parse | claimed 2026-11-19 | 6 函数；parse_target 失败路径由 app 层用例覆盖 |
| play  | rust/play  | claimed 2026-11-19 | 5 函数；真实 playurl/view fixtures；CliError 错误用例已录 |
| sign  | rust/sign  | claimed 2026-11-19 | 3 函数；sign_app 走固定时钟基准 |

状态流：`open` → `claimed by <会话> <日期>` → `done`（回填 gate/diff 结果）。

## wave 2 胶水（协调者写，存 rust/app/src/）
curl 封装 + CliError/错误分层/report（LABELS/CURL_EXIT/BIZ_CODES/HINTS）；
fetch_json；grpc_call；view/user_videos/user_name；playurl；
_curl_dl/_merge（断点续传 + 半成品必删）；_render_qr（qrencode/uv/open 链）；
login_web/login_tv；load_creds/save_creds；
cmd_search/list/info/dl/doctor/help + clap 补全 + HELP 文本 + main。

## wave 3 冒烟（协调者跑，用户配合一次）
search 1 次 / `list --json --rights` 1 次 / info 1 次 / doctor；
`dl --audio` 小样本 + 断点续传 + 半成品删除；login（网页+TV，**用户扫码**）。

## lift 条件
fallback 率 0 + 4 单元 gate 全绿 + 冒烟全过 → `rust/app` 纯化 →
`bilibili_cli.py` 移 `legacy/`，README 重写。

## 已解决的坑（别再踩）
- record 生成用例喂垃圾输入会触发 `die()`→SystemExit 杀录制进程
  → 一律 `--count 0 --inputs inputs.jsonl`。
- bridge 模板 lib 名默认 `bilibili_cli_rs` → 已改 `[lib] name = <module>`。
- 私有函数（`_` 开头）不在 bridge exports → 元数据已手工补，**别动 rust-anything.json**。
- `sign_app` 内部读时钟 → 固定 ts 基准（`rust/sign/fixtures/sign_fixed_ts.json`）。
