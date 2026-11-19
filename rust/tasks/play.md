# play 模块任务书

## 目标（验收在此，做到才算完）
把**权益判定 + 通道可播性 + 流挑选**移植到 `rust/play`（5 个函数）。
这是 info / dl 命令的核心。

## 函数清单（行号）
`entitlement_of`@502 `access_of`@524 `pick_streams`@590 `_url_of`@605 `_backup_of`@609

## 关键语义与坑
- `entitlement_of` 优先级：**charging > paid > vip > free**；flags dict 恒定
  五键（pay/ugc_pay/arc_pay/elec/movie）+ `upower_exclusive`；rights 缺失 → free。
- `access_of`：live = ok 且 `tracks>0`；best≥64 → yes 否则 limited；
  全 auth 失败 → 「需要登录后才能判断」文案；ent.kind≠free 时 no 分支带 reason。
- `pick_streams`：无视频流抛 `CliError("rights", ...)`——**该错误路径基线已录**
  （看 cases.jsonl 的 thrown 条目怎么编码，port 返回 Err 时格式照抄，gate 会查）。
  allowed 为空回退全集；同 codec 优选（avc=7/hevc=12/av1=13）；
  best 按 (id, height, bandwidth) 三元组 max；audio 无 → 空 dict。
- `baseUrl`/`base_url`、`backupUrl`/`backup_url` 双写法都要读（接口混用）。

## 基线已录，你要补：
1. `_enc` 技巧从 `rust/fixtures/playurl_app.json` 的 dash（12 视频轨/3 音轨，真实
   响应）生成：`pick_streams` ≥3 条（want_qn=80/32/200 × codec=avc/hevc）、
   `_url_of`/`_backup_of` 真实轨道各 2 条。
2. `rust/fixtures/view_free.json` 的 data 生成 `entitlement_of` 真实用例 1 条（预期 free）。
3. `pick_streams`：音轨缺失（audio → {}）、allowed 回退、codecid 全不匹配。

## 验收标准
1. `diff` 全绿 N/N；2. `gate` 全绿；
3. `CliError("rights")` 的 Err 输出与 cases.jsonl thrown 条目一致。
