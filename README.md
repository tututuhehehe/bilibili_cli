# bcli（bilibili_cli）

B 站取用工具：**搜 → 登录 → 列投稿 → 看详情 → 下载**。纯 Rust 单二进制。

```
bcli                            # 本工具（Rust 二进制）
~/.bilibili_cli/creds.json      # 登录态（600）
~/Documents/bilibili_content/   # 下载产物
（Python 原版在 git 分支 legacy-python，main 仅 Rust）
```

---

## 它做什么 / 不做什么

| 做 | 不做 |
|---|---|
| 扫码登录（网页 / TV） | 订阅、轮询、last_seen 状态记录 |
| 列某 UP 主近 N 条投稿 | 番剧 / 影视（PGC）支持 |
| 单视频详情：分P、权益、通道可用性 | web 播放通道 |
| 下载视频或音频（app → tv 降级，**≥32MB 自动分段并行**） | 任何绕过大会员 / 充电付费墙的手段 |

**订阅编排属于调用方**（个人助理插件 / agent）：本工具只提供事实源（`list --json`、`info --json`）。

---

## 构建与依赖

```bash
brew install ffmpeg qrencode    # ffmpeg 合并必需；qrencode 终端二维码（可选）
cd rust/app && cargo build --release
# 产物：rust/app/target/release/bcli；bcli 启动脚本优先执行它
```

外部只依赖系统 `curl`（传输：TLS 指纹与真实浏览器同系 + 自带 HTTP/2/续传）、`ffmpeg`、可选 `qrencode`。**不依赖 Python 运行时。**

## 命令

```bash
bcli help                      # 总览；bcli help dl 看单命令详解
bcli search <关键词> [-n 20] [--sort X] [--users] [--desc] [--json]
bcli login                     # 网页扫码（决定 1080P / 字幕）；login --tv 走 TV 端
bcli list <uid|链接> [-n 20] [--json] [--rights]
bcli info <BV|链接> [--json]
bcli dl <BV|链接> [--audio] [--quality Q] [--codec C] [--parallel N] [--subs] [--keep-parts] [-o DIR] [--json]
bcli doctor
```

`--parallel N`：分段并行下载（默认 4；`0/1`=单流；上限 8；仅 ≥32MB 生效，音频同规则）。
实测同节点单连接被 CDN 压速时可达 **9.3×** 提速；分段与单流产物 md5 逐字节一致。

## 产物与 `.info.json`

```
~/Documents/bilibili_content/<UP主>/<YYYY-MM-DD>_<标题>.mp4   # 或 .m4a
                                   同名 .info.json
```

`.info.json` 关键字段（schema `bilibili_cli.v1`，字段只增不删）：

```json
{
  "schema": "bilibili_cli.v1",
  "id": "BV...", "title": "...", "url": "...",
  "author": {"uid": "...", "name": "..."},
  "published_at": "2026-10-20T11:00:00+08:00", "published_date": "2026-10-20",
  "duration_sec": 2369, "duration_text": "39:29",
  "quality": "1080P", "quality_id": 80, "codec": "avc1.640032",
  "channel": "app",
  "entitlement": {
    "kind": "free",
    "my_access": {"state": "yes", "text": "可播放", "best_quality": "1080P", "via": "app"}
  }
}
```

---

## 数据来源（全部实测可用）

| 用途 | 接口 | 鉴权 | 说明 |
|---|---|---|---|
| 视频/UP 主搜索 | `grpc.biliapi.net` → `bilibili.polymer.app.search.v1.Search/SearchAll` | 无 | **gRPC**；每页 20 条；匿名连打无风控 |
| UP 主搜索 | 同上 → `Search/SearchByType`（`type=2`） | 无 | `mid` 在 `Item.param` |
| 简介 / 统计 | `bilibili.app.view.v1.View/View` | 无 | `arc.desc`(10) / `arc.stat`(23) |
| 投稿列表 | `app.bilibili.com/x/v2/space/archive/cursor` | appkey + md5 签名 | 匿名可用；绕开 web 端 WBI 的 412 |
| 视频详情 | `api.bilibili.com/x/web-interface/view` | 无 | 分P、`rights`、`is_upower_exclusive` |
| 播放流（app） | `api.bilibili.com/x/player/playurl` | appkey 签名 + cookie | 首选通道 |
| 播放流（tv） | `api.bilibili.com/x/tv/playurl` | TV appkey + `access_token` | 降级通道；`is_proj=1`、`fnval=4048` |
| 字幕 | `api.bilibili.com/x/player/v2` | cookie | `subtitle.subtitles[].subtitle_url` |
| 网页/TV 扫码 | `passport.bilibili.com` | 无 / TV 签名 | generate → poll（poll 响应里收 cookie） |

appkey/appsec 为客户端内公开常量（源码 `glue/creds.rs`）。

> 下载流必须用浏览器 UA（`curl()` 默认 UA_WEB）：BiliDroid UA 直连流会被 CDN 403。

---

## 实测结论（省得重复踩）

1. **TV 端对普通 UGC 零收益**：app / tv 同视频逐帧 PSNR = ∞，差异仅 CDN 主机；app 的 CDN 更稳（tv 裸 IP 会断流）。默认顺序 `app → tv`。
2. **权益绑定账号**：匿名 480P；cookie → 1080P；1080P+/4K 要大会员；充电专属要充电；app 与 tv 都拿不到就是拿不到。
3. **App 搜索是 gRPC 不是 REST**：`x/v2/search/type` 已空壳；搜索卡无简介（服务端从不填 desc=11），`--desc` 逐条补一次 View。
4. **web 搜索/空间接口会被 412/gaia 挑战**：列表统一走 app 签名接口。
5. **搜索排序实时抖动**（totalrank 第 2 条起会变）：任何对照验收一律配对跑。
6. **长视频慢的根因是单连接压速**：≥32MB 自动分段并行（`--parallel` 可调）。

## 错误分层与退出码

| 层级 | 触发 | 含义 |
|---|---|---|
| 网络 / 域名 | curl 6/7/56 | DNS 或端口不通 |
| TLS / 超时 | 35/60 · 28 | 握手问题 / 被限速 |
| 风控拦截 | HTTP 403/412/429 或 HTML | 出口特征被标记 |
| 密钥 / 登录态 | `-101` / `-403` | 需重新登录 |
| 接口改版 | `-400` / `-404` / 缺字段 | 参数或结构变了 |
| 账号权益 | `-10403` / `87007` / `87008` / `76150` | 大会员 / 充电 / 版权 |
| 频率 / 服务端 | `-509` / `-799` · 5xx | 打太快 / B 站侧 |

退出码：`0` 成功 · `2` 参数错 · `3` 网络或接口 · `4` 搜索无结果 · `5` 需登录 · `6` 缺依赖 · `7` 权益不足

## 设计决策（为什么这么做）

1. **传输用系统 curl**：TLS 指纹同系浏览器，自带 HTTP/2 / `-C -` 续传 / `--retry`；不引入异步运行时。
2. **gRPC 手写 protobuf**（`rust/proto`）：字段表即契约，无 tonic/tokio 依赖。
3. **错误分层**（`rust/app/src/glue/err.rs`）：报错按层归属，文案与 Python 版逐字一致。
4. **argparse 复刻层**（`rust/app/src/frontspec.rs`）：参数校验/报错/帮助文本逐字节复刻
   Python argparse（含 repr 转义、报错优先级、缩写、负数字值）；`rust/app-cases/` 122 条
   离线用例是回归基线（`rust-anything app replay . --build rust/app/target/release/bcli`）。
5. **分段并行**（`--parallel`）：Range 探测 → 分段并发 → 拼接；任一段失败删半成品回退单流。
6. **断流自动续传 + 半成品必删**：绝不把半截文件留给 Whisper 之类的下游。
7. **单元即库**：`rust/{proto,parse,play,sign}` 是纯库（lift 后无绑定），各有 gate 验过的
   行为基线（`cases.jsonl`）与联网测试。

## 已知坑（踩过的）

| 坑 | 处理 |
|---|---|
| HttpOnly cookie | jar 里 `#HttpOnly_` 前缀**剥掉再解析**（`_read_jar`），丢前缀会丢 SESSDATA |
| Set-Cookie 位置 | 扫码登录的 cookie 在 **poll** 响应里；poll 必须挂 cookie jar |
| TV 轮询中间态 | `86039` 是顶层 code，用 `allow_codes` 收 |
| 流地址直连 403 | 下载走 curl 默认浏览器 UA + Referer |
| wire 生成器 | `rust-anything app wire` 每单元只认一个命令，多次 wire 后需手工核对路由表 |

## 开发

```
rust/app            前门二进制（clap + frontspec + glue + units）
rust/{proto,parse,play,sign}   纯单元库（gate ✓ 的行为基线在各自 cases.jsonl）
rust/app-cases/     122 条离线回归基线（trycmd 格式）
rust/DECISIONS.md   ADR 决策记录；rust/MAP.md 状态板
```

回归三件套：`cargo test`（五个 crate）· `cargo clippy -D warnings` ·
`rust-anything app replay . --build rust/app/target/release/bcli`。
网络命令的等价性用**配对冒烟**（Rust vs legacy Python 同查同下对比）。

## 未来可做（等真有需求再说）

- `follow`：拉关注列表初始化订阅（`x/relation/followings`，需登录）
- 番剧：`playurl_type=2` + `ep_id`；`pgc/view/web/season` 看剧集
- 笔记流水线：`bcli dl --audio` → `mlx-whisper` → LLM 结构化笔记
