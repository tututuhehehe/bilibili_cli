# bilibili_cli（`bcli`）

B 站取用工具：**搜 → 登录 → 列投稿 → 看详情 → 下载**。

无状态、无守护进程、单文件纯标准库；外部只依赖 `curl`、`ffmpeg`（可选 `qrencode`）。

```
bilibili_cli.py            主程序（唯一文件）
~/.bilibili_cli/creds.json 登录态（600）
~/Documents/bilibili_content/  下载产物
```

---

## 它做什么 / 不做什么

| 做 | 不做 |
|---|---|
| 扫码登录（网页 / TV） | 订阅、轮询、last_seen 状态记录 |
| 列某 UP 主近 N 条投稿 | 番剧 / 影视（PGC）支持 |
| 单视频详情：分P、权益、通道可用性 | web 播放通道 |
| 下载视频或音频（app → tv 降级） | 任何绕过大会员 / 充电付费墙的手段 |

**订阅编排属于调用方**（个人助理插件 / agent）：本工具只提供事实源（`list --json`、`info --json`），由上层决定「哪些 UP 只提醒、哪些要自动下载」。

---

## 依赖

```bash
brew install ffmpeg qrencode     # ffmpeg 合并音视频必需；qrencode 仅用于终端二维码
# curl 系统自带
```

安装（软链）：

```bash
ln -sf ~/Documents/moon_cli_or_tool/bilibili_cli/bilibili_cli.py ~/.local/bin/bcli
chmod +x ~/Documents/moon_cli_or_tool/bilibili_cli/bilibili_cli.py
```

---

## 命令

```bash
bcli help                     # 总览
bcli help dl                  # 单命令详解
```

| 命令 | 说明 |
|---|---|
| `bcli search <关键词> [-n 20] [--sort X] [--users] [--desc] [--json]` | 搜视频 / 搜 UP 主 |
| `bcli login` | 网页扫码，存 cookie（决定能否 1080P / 字幕）|
| `bcli login --tv` | 云视听小电视扫码，存 access_token（仅作降级通道）|
| `bcli list <uid\|链接> [-n 20] [--json] [--rights]` | 近 N 条投稿 |
| `bcli info <BV\|链接> [--json]` | 分P / 权益 / 两通道探测 |
| `bcli dl <BV\|链接> [--audio] [--quality Q] [--codec C] [--subs] [--keep-parts] [-o DIR] [--json]` | 下载 |
| `bcli doctor` | 环境 + 登录态 + 接口自检 |

### 例子

```bash
bcli search "露营 装备" -n 5           # 搜视频（默认综合排序）
bcli search "露营" --sort new          # 最新发布
bcli search "影视飓风" --users         # 搜 UP 主，拿 mid
bcli search "露营" -n 3 --desc         # 顺带简介 + 收藏/点赞/投币
bcli login
bcli list 23947287 -n 5
bcli list "https://space.bilibili.com/23947287?spm_id_from=..." --json
bcli list 23947287 -n 10 --rights      # 逐条标 [充电专属] / [会员专享]
bcli info BV1ocsd6jErz                 # 要不要钱、我能不能播
bcli dl BV1Vbnn6iEfG --audio           # 只要音频（做笔记，约 1/10 体积）
bcli dl BV1Vbnn6iEfG --quality 720P --subs
```

### 产物

```
~/Documents/bilibili_content/<UP主>/<YYYY-MM-DD>_<标题>.mp4   # 或 .m4a
                                   同名 .info.json           # 元数据 + 权益快照
```

`.info.json` 关键字段：

```json
{
  "schema": "bilibili_cli.v1",
  "id": "BV...", "title": "...", "url": "...",
  "author": {"uid": "...", "name": "..."},
  "published_at": "2026-10-20T11:00:00+08:00",
  "published_date": "2026-10-20",
  "duration_sec": 2369, "duration_text": "39:29",
  "quality": "1080P", "quality_id": 80, "codec": "avc1.640033",
  "channel": "app",
  "entitlement": {
    "kind": "free",            // free | vip | charging | paid
    "label": "免费",
    "vip_only": false, "charging_only": false, "paid": false,
    "my_access": {"state": "yes", "text": "可播放", "best_quality": "1080P", "via": "app"}
  }
}
```

---

## 数据来源（全部实测可用）

| 用途 | 接口 | 鉴权 | 说明 |
|---|---|---|---|
| 视频/UP 主搜索 | `grpc.biliapi.net` → `bilibili.polymer.app.search.v1.Search/SearchAll` | 无 | **gRPC**；每页 20 条，`order` 0-4；实测连打 10 次无风控 |
| UP 主搜索 | 同上 → `...Search/SearchByType`（`type=2`）| 无 | UP 主卡；`mid` 在 `Item.param` 而不是卡里 |
| 简介 / 统计 | `grpc.biliapi.net` → `bilibili.app.view.v1.View/View` | 无 | `arc.desc`(10) / `arc.stat`(23)；约 130ms/条 |
| 投稿列表 | `app.bilibili.com/x/v2/space/archive/cursor` | appkey + md5 签名 | 匿名可用；**绕开了 web 端 WBI 的 412 风控** |
| 视频详情 | `api.bilibili.com/x/web-interface/view` | 无 | 分P、`rights`、`is_upower_exclusive` |
| 播放流（app） | `api.bilibili.com/x/player/playurl` | appkey 签名 + cookie | 首选通道 |
| 播放流（tv） | `api.bilibili.com/x/tv/playurl` | TV appkey 签名 + `access_token` | 降级通道；需 `is_proj=1`、`playurl_type=1`、`object_id`、`fnval=4048` 才给 DASH |
| 字幕 | `api.bilibili.com/x/player/v2` | cookie | `subtitle.subtitles[].subtitle_url` |
| 网页扫码 | `passport.bilibili.com/x/passport-login/web/qrcode/*` | 无 | `generate` → `poll` |
| TV 扫码 | `passport.bilibili.com/x/passport-tv-login/qrcode/*` | TV appkey 签名 | 需 POST |

appkey/appsec 为 B 站客户端内的公开常量（社区长期公开）：

```
Android  1d8b6e7d45233436 / 560c52ccd288fed045859ed18bffd973
TV       4409e2ce8ffd12b8 / 59b43e04ad6965f34319062b478f83dd
```

> 注意：`x/tv/playurl` 的正确参数是 `object_id`(aid) + `is_proj=1` + `playurl_type=1` + `platform` + `protocol=0` + `fnval=4048`，不是 `avid`。

---

## 实测结论（重要，省得以后重复踩）

### 1. 暴露 TV 端对普通 UGC **零收益**

同一视频分别用 app / tv 通道取流，逐帧 PSNR 对比：

```
app  1080P 1920x1080 avc1.640033 1473kbps | 12 条轨 | 音轨 149kbps
tv   1080P 1920x1080 avc1.640033 1473kbps | 12 条轨 | 音轨 149kbps
PSNR（前 76 帧） = inf dB   ← 像素级完全相同
```

差异只有 CDN 主机（tv 走裸 IP）与 tv 多返回 `size` 字段。
**因此默认顺序是 `app → tv`**：内容相同，app 的 CDN 更稳（tv 裸 IP 实测会断流）。

### 2. 权益是账号绑定的，伪装身份无效

| 内容 | 需要什么 | 结果 |
|---|---|---|
| 720P | 匿名 | ✅ |
| 1080P | 登录 | ✅ |
| 1080P+ / 4K | **大会员** | ❌ app 与 tv 都不给流 |
| 充电专属（`is_upower_exclusive`）| **给该 UP 充电** | ❌ app 无流；tv 回 `-404` / `76150` |
| 番剧 / 影视 | 大会员 + 版权 | 未实现（`playurl_type=2` + `ep_id`）|

### 3. App 搜索是 gRPC，不是 REST

`app.bilibili.com/x/v2/search/type` 仍然有响应，但 `code=0` 且 `total=0`——
**这个端点已经空了**（带 TV `access_token`、POST、换 appkey 都一样）。
真正的 App 搜索是 gRPC：`bilibili.polymer.app.search.v1.Search/SearchAll`，
`grpc.biliapi.net`，**不需要登录也不需要 access_key**，连打 10 次全部 `grpc-status: 0`。

字段（按 `search.proto` 反解，并逐条与 web 真值对过）：

| 位置 | 含义 |
|---|---|
| `Item.37` | 视频卡 `SearchVideoCard` |
| `card.1` / `8` / `9` / `10` / `12` / `14` | 标题 / 播放 / 弹幕 / 作者 / 时长文本 / UP mid |
| `card.23` | **发布时间戳**（5/5 与 `web-interface/view` 的 `pubdate` 完全一致）|
| `card.6.video.1` | BV 号 |
| `Item.2` | **aid**（不是 `card.6.video.2`，那个是别的 id）|
| `Item.36` | UP 主卡 `SearchUpperCard`（`title` 昵称 / `4` 粉丝 / `5` 投稿数 / `3` 签名 / `8` 认证）|

时长文本**不补零**（`25:7`），转秒要按 `\d+:\d+(:\d+)?` 匹配。

### 4. 搜索卡里**没有简介**（别以为是解析错了）

`search.proto` 里 `SearchVideoCard.desc = 11` 是存在的，但服务端**从来不填**：
6 个关键词 × 20 张卡全部缺失；`full_text(17)` 也没有。
成熟开源客户端 `aaa1115910/bv` 的搜索映射同样只有
`aid/bvid/title/cover/author/duration/play/danmaku`——它也不显示简介。

简介要从详情接口拿：gRPC `bilibili.app.view.v1.View/View` 的 `arc.desc`(10)，
或 REST `x/web-interface/view`（连打 5 次 0.4 秒全成功）。
所以 `--desc` 是**逐条补一次详情**（+130ms/条），不是搜索接口自带。

### 5. Web 搜索能用，但会 412

`api.bilibili.com/x/web-interface/search/type` 匿名可用、字段更全（含 `description`），
但属于窗口计数限流：连打约 6 次后 `-412`，静默约 90 秒恢复。
所以默认走 gRPC，web 仅作备选。

### 6. Web 端 `x/space/wbi/arc/search` 不可用

自研 WBI 签名 + visitor cookie + 完整浏览器头均被 `-412`；连真浏览器的页面请求也被 gaia 风控 SDK 挑战。**列表统一走 app 接口。**

---

## 错误分层与退出码

报错会拆成「哪一层坏了」，而不是一句 `curl 失败`：

| 层级 | 触发 | 含义 |
|---|---|---|
| 网络 / 域名 | curl 6/7/56 | DNS 或端口不通，与 key 无关 |
| TLS | 35/60 | 握手或证书问题 |
| 超时 | 28 | 网络慢或被限速 |
| 风控拦截 | HTTP 403/412/429，或返回 HTML | 出口特征被标记 |
| 密钥 / 登录态 | `-101` / `-403` | appkey 失效或需重新登录 |
| 接口改版 | `-400` / `-404` / 缺 `data.item` | 参数或结构变了 |
| 账号权益 | `-10403` / `87007` / `87008` / `76150` | 大会员 / 充电 / 版权 |
| 频率限制 | `-509` / `-799` | 打太快 |
| 服务端 | HTTP 5xx / `-500` / `-1` | B 站侧问题 |

退出码：`0` 成功 · `2` 参数错 · `3` 网络或接口 · `5` 需登录 · `6` 缺依赖 · `7` 权益不足

---

## 设计决策（为什么这么做）

1. **传输用系统 `curl`，不用 requests/httpx** —— curl 的 TLS 指纹与真实浏览器同系，且自带 HTTP/2、续传、重试。
2. **不做订阅/轮询** —— 单一职责，订阅策略放调用方（agent / 助理插件），改策略不用改代码。
3. **砍掉 web 播放通道** —— 限制多、风控重；app/tv 两条已覆盖。
4. **下载默认 app 优先** —— 实测两通道画面完全相同（PSNR=∞），app 的 CDN 更稳定。
5. **断流自动续传 + 半成品必删** —— TV 裸 IP CDN 会中途断开；绝不把半截文件留给 Whisper 之类的下游。

---

## 已知坑（踩过的）

| 坑 | 现象 | 处理 |
|---|---|---|
| HttpOnly cookie | curl jar 里写作 `#HttpOnly_.bilibili.com\t...`，当注释跳过会丢 `SESSDATA` | `_read_jar` 剥前缀 |
| Set-Cookie 位置 | 扫码登录的 cookie 在 **poll** 响应里，不在二维码 URL 上 | poll 时挂 cookie jar |
| TV 轮询中间态 | `86039` 是**顶层 code**（尚未确认），不是 `data.code` | `fetch_json(allow_codes=...)` |
| 番剧字段名 | `pgc/view/web/season` 把数据放 `result` 而非 `data` | 若将来做番剧需注意 |
| 流地址直连 403 | ffmpeg 直接拉流缺 UA | 下载统一走 curl，带 UA + Referer |

---

## 未来可做（等真有需求再说）

- **`follow`**：拉取当前账号关注的 UP 主列表（`x/relation/followings`，需登录，实测可返回 1176 人）。用途是给订阅初始化时免去手填 uid。**当前不需要，故未实现。**
- **番剧**：`playurl_type=2` + `ep_id` 下载；`pgc/view/web/season` 看剧集与 `badge: 会员` 标记；`pgc/web/timeline` 看全站更新日历。
- **订阅编排**：在上层（助理插件）读 `bcli list --json`，按 `subs.json` 里的 `download: true` 决定是否 `bcli dl`。
- **笔记流水线**：`bcli dl --audio` → `mlx-whisper` 转写 → LLM 结构化笔记。
