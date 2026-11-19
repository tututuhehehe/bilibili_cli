# proto 模块任务书

## 目标（验收在此，做到才算完）
把 bilibili_cli.py 的 **protobuf wire 编解码 + App gRPC 搜索/详情卡解析**逐函数移植到
`rust/proto`（13 个函数，桩已在 src/port.rs）。无网络依赖——网络编排在 app 胶水层。

## 函数清单（行号）
`_pv`@1122 `_ps`@1134 `_pi`@1140 `_pm`@1145 `_pread`@1150 `_ptext`@1190
`_pint`@1198 `_pmsg`@1206 `_prep`@1213 `_strip_em`@1265 `_dur_seconds`@1269
`_video_from_item`@1280 `_user_from_item`@1310

## 关键语义与坑（实测/读码结论）
- **`_pi` 负数会让原版死循环**（record 实测跳过了 `[2,-1]` 并注明）。
  port 对负数返回 `Err("negative int: the original hangs here")`，
  并写进「已知差异」——这是唯一允许的显式差异。
- varint：u64 域；字段 tag = `(num << 3) | wire_type`。
- 卡片字段表见 README「数据来源」节：`Item.37`=视频卡、`card.23`=发布时间戳、
  `Item.2`=aid（不是 card.6.video.2）、`card.6.video.1`=BV、`Item.36`=UP 主卡。
- 时长文本不补零（"25:7"）；`_dur_seconds` 按 `\d+:\d+(:\d+)?` 匹配，不中 → 0。
- `_strip_em`：读源码确认剥的具体内容（em 高亮/emote 短代码），中英文/emoji 样本对齐。
- `_pread` 返回 **int 键 dict + tuple 值** → 走 `$dict`/`$tuple` 标记
  （cases.jsonl 已录条目就是样例，照它的形状来）。

## 基线已录（cases.jsonl），你要补：
1. `_video_from_item` / `_user_from_item` 各 ≥5 条**真实条目**：用 `_enc` 技巧从
   `rust/fixtures/searchall_1.resp.bin`、`view_1.resp.bin` 生成
   （先 python 调原版 `_pread(帧字节)` 拿 fields，逐 Item 编码 args）。
   必须覆盖：时长文本、em 高亮标题、UP 主卡（fans/sign/认证）。
2. cargo 单测：直接读两个 `.resp.bin`，`_pread` 输出与「原版一次性运行产生的参照
   JSON」逐字段比对（参照你自己用 python 生成一次，存 `tests/expected_*.json`）。
3. regex 陷阱用例（工具已自动加了 `_ps/_strip_em/_dur_seconds` 几条，确认覆盖
   中文/emoji/全角）。

## 验收标准
1. `diff` 全绿 N/N（含你补的用例）。
2. `gate` 全绿（fmt / clippy -D warnings / 无 unsafe / 错误路径 Err /
   port.rs 行数 ≤ 原版对应函数合计的 3 倍）。
3. 真实帧在 cargo 测试下与原版逐字段一致。
4. `_pi` 负数差异必须写进「已知差异」。
