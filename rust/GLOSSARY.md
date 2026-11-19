# GLOSSARY — bcli 领域术语 → Rust 命名

单元会话遇到新术语在汇报里提出，协调者回填本表。

| 术语 | 含义 | Rust 侧约定 |
|---|---|---|
| aid | 视频 int64 id（链接 `av`/`aid=` 形式） | i64 |
| bvid | "BV" + 10 位 base58 风格视频号 | String |
| cid | 分P id | i64 |
| mid / uid | 用户 id。**类型注意**：搜索卡里 `mid` 是 int，`parse_target` 输出 `uid` 是 str | i64（卡）/ String（target） |
| qn | 清晰度档位：127=8K 126=杜比 125=HDR 120=4K 116=1080P60 112=1080P+ 80=1080P 64=720 32=480 16=360 | i64 + QN 表常量 |
| codecid | 视频编码：7=avc 12=hevc 13=av1 | i64 + CODECS 表 |
| fnval=4048 | DASH 请求标记（tv 通道必须） | 常量 |
| DASH | 音视频分离流（dash.video[] / dash.audio[]） | serde_json::Value |
| baseUrl / base_url、backupUrl / backup_url | 流地址双写法，B 站接口混用 | 两键都读 |
| 权益 kind | free / vip / charging / paid；判定优先级 charging > paid > vip > free | 输出按原版字符串，flags 五键恒定 |
| my_access.state | yes / limited / no / unknown | 按原版字符串 |
| 通道 channel | app（首选）/ tv（降级） | &str，逐字对齐 |
| appkey / appsec | Android/TV 客户端公开常量；签名 = md5(排序query + sec) | 常量 + sign_with_ts |
| creds.json | `~/.bilibili_cli/creds.json`（600）：`cookie`、`tv_access_token`、`access_key` 等字段 | Value，字段不增不删 |
| cookie jar | curl `-c` 写的 Netscape 格式；`#HttpOnly_` 前缀行不是注释 | _read_jar |
| CliError 分类 | network / tls / timeout / empty / risk / auth / args / interface / rights / missing…（以 LABELS、BIZ_CODES 源码为准，app 层移植时逐条核对） | app 层 enum + 中文文案表 |
| 退出码 | 0 成功 · 2 参数 · 3 网络/接口 · 5 需登录 · 6 缺依赖 · 7 权益 · 130 Ctrl-C | app 层映射 |
| $bytes / $dict / $tuple / $repr | bridge 的 JSON 标记（bytes、int键dict、元组、repr） | pyconv 约定，cases.jsonl 即样例 |
