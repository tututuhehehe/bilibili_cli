//! 前端文本层：由 fixture / argparse 复刻器生成（lift 基线的一部分）。
//! 大段文本逐字来自 app-cases/*.stdout（录自 Python 原版，复刻保真断言 0 失真）。
//! dl 文本含 --parallel（ADR-014）。生成脚本：lift 提交说明。
#![allow(clippy::needless_raw_string_hashes)]

pub const TOP_HELP: &str = r#"usage: bilibili_cli [-h] {login,list,search,info,dl,doctor,help} ...

B 站取用工具：登录 / 列投稿 / 看详情 / 下载（无状态，不含订阅逻辑）

positional arguments:
  {login,list,search,info,dl,doctor,help}
    login               扫码登录（默认网页；--tv 为云视听小电视）
    list                列出某 UP 主近 N 条投稿
    search              按关键词搜视频（--users 搜 UP 主）
    info                单视频详情：分P / 清晰度 / 各通道可用性
    dl                  下载视频或音频（app 首选，tv 降级备用）
    doctor              环境与登录态自检
    help                查看帮助（bcli help [command]）

optional arguments:
  -h, --help            show this help message and exit
"#;

pub const HELP_OVERVIEW: &str = r#"bilibili_cli — B 站取用工具（无状态：只登录、只列、只看、只下）

用法: bcli <command> [参数]      # 看单个命令: bcli help <command>

命令:
  login      扫码登录（--tv 走 TV 端，仅作降级通道）
  search     关键词搜视频 / UP 主（走 App gRPC，无需登录）
  list       列出某 UP 主近 N 条投稿
  info       单视频详情：分P / 权益 / 两条通道可用性
  dl         下载视频或音频（app 首选，tv 降级）
  doctor     环境与登录态自检
  help       查看帮助

常用示例:
  bcli login                               登录一次，长期有效
  bcli search "露营 装备" -n 5              按关键词搜视频
  bcli search "影视飓风" --users            搜 UP 主，拿 mid
  bcli search "露营" -n 3 --desc            带简介和收藏/点赞数
  bcli list 23947287 -n 5                  看最近 5 条投稿
  bcli list 23947287 -n 10 --json         给 agent 用的结构化输出
  bcli info BV1ocsd6jErz                   这条能不能下、要不要钱
  bcli dl BV1Vbnn6iEfG --audio             只要音频（做笔记）
  bcli dl BV1Vbnn6iEfG --quality 720P      限制画质省磁盘

约定:
  产物目录   ~/Documents/bilibili_content/<UP主>/<日期>_<标题>.{mp4|m4a}
  元数据     同名 .info.json（含 channel 与权益快照）
  登录态     ~/.bilibili_cli/creds.json（权限 600）
  不做什么   订阅/轮询（交给 agent 侧）、番剧、web 播放通道

退出码: 0 成功 / 2 参数错 / 3 网络或接口失败 / 5 需要登录
        6 缺依赖 / 7 权益不足（大会员/充电）
"#;

pub const HELP_DETAIL_LOGIN: &str = r#"bcli login              网页扫码登录 → cookie（决定能否 1080P / 字幕）
bcli login --tv         云视听小电视扫码 → access_token（仅作为下载的降级通道）

扫码位置：真终端里直接画 ASCII 二维码；非终端（脚本 / agent）生成 PNG 并自动打开。
有效期：cookie 约数月，TV token 约数月；过期会明确提示重新登录。
存储：~/.bilibili_cli/creds.json（权限 600）。
兜底：环境变量 BILI_SESSDATA 可直接提供 cookie。
"#;

pub const HELP_DETAIL_LIST: &str = r#"bcli list <uid|链接> [-n 20] [--json] [--rights]

  target    纯数字 uid，或 space.bilibili.com 链接（带 spm 参数也行）
  -n        条数，默认 20
  --json    机器可读（schema: bilibili_cli.v1）
  --rights  逐条判定是否会员/充电专属（每条多一次请求，慢）

例：bcli list 23947287 -n 5
    bcli list "https://space.bilibili.com/23947287?spm_id_from=..." --json

字段：id / title / url / published_at(ISO8601) / published_date
      duration_sec / duration_text / views / cover / paid / badges
"#;

pub const HELP_DETAIL_SEARCH: &str = r#"bcli search <关键词> [-n 20] [--sort X] [--users] [--desc] [--json]

  -n        条数，默认 20，上限 100（gRPC 每页 20，自动翻页）
  --sort    totalrank 综合(默认) | play 最多播放 | new 最新 | dm 最多弹幕 | stow 最多收藏
  --users   搜 UP 主（返回 mid / 粉丝 / 投稿 / 等级 / 认证 / 签名）
  --desc    补每条简介 + 收藏/点赞/投币（每条多一次请求，约 +130ms）
  --json    schema: bilibili_cli.v1，type=search

通道：B 站 App 的 gRPC 接口（bilibili.polymer.app.search.v1），
      不需要登录、不需要 cookie、不碰浏览器，实测连打不触发风控。

字段：视频 → bvid / aid / title / author / mid / play / danmaku
                duration_text / duration_sec / pubdate / published_at / url
      UP 主 → mid / name / fans / archives / level / sign / official / url
      --desc 追加 → desc（作者简介）/ stats{play,danmaku,reply,favorite,coin,share,like}

例：bcli search "露营 装备" -n 5
    bcli search "露营" --sort new --json
    bcli search "影视飓风" --users
    bcli search "露营" -n 3 --desc          # 带简介，适合直接看要不要点开
"#;

pub const HELP_DETAIL_INFO: &str = r#"bcli info <BV|链接> [--json]

用途：下载前探路 —— 这条视频要不要钱、我能不能播、有哪些清晰度。

输出：标题 / UP / 发布日期 / 时长 / 分P
      🏷 内容类型：free(免费) | vip(会员专享) | charging(充电专属) | paid(付费)
      🔑 账号权限：yes(可播放) | limited(仅低清晰度) | no(无权限) | unknown(需登录判断)
      通道探测：app / tv 各自最高清晰度与轨道数

例：bcli info BV1ocsd6jErz --json
"#;

pub const HELP_DETAIL_DL: &str = r#"bcli dl <BV|链接> [--audio] [--quality Q] [--codec C] [--parallel N] [--subs] [--keep-parts] [-o DIR] [--json]

  --audio        只下音频 m4a（做笔记最快，体积约 1/10）
  --quality      最高清晰度：1080P / 720P / 480P / 4K，默认最高可用
  --codec        视频编码偏好 avc | hevc | av1（默认 avc，兼容性最好）
  --parallel     分段并行下载数（默认 4；0/1=单流，上限 8，仅 ≥32MB 生效）
  --subs         同时保存字幕 srt
  --keep-parts   保留下载的临时分轨（排查用）
  -o             输出根目录，默认 ~/Documents/bilibili_content

通道：app 首选 → tv 降级 → 都失败则报错（不碰 web 端）
产物：<DIR>/<UP主>/<YYYY-MM-DD>_<标题>.mp4（或 .m4a）
      + 同名 .info.json（含 channel、权益快照 my_access）
断流：自动续传最多 3 次；最终失败会删除半成品，绝不留下“看起来正常”的文件

例：bcli dl BV1Vbnn6iEfG --audio
    bcli dl BV1Vbnn6iEfG --quality 720P --subs
    bcli dl BV1Vbnn6iEfG -o /tmp/test
"#;

pub const HELP_DETAIL_DOCTOR: &str = r#"bcli doctor

检查：curl / ffmpeg / qrencode / uv 是否就绪
      网页 cookie 与 TV token 的登录态
      投稿接口连通性（用样例 UP 实测）
      gRPC 搜索接口连通性（用关键词「露营」实测 1 条）

报错会拆到“网络 / TLS / 超时 / 风控 / 密钥 / 权益 / 接口改版 / 频率 / 服务端”。
"#;

pub const SUBHELP_LOGIN: &str = r#"usage: bilibili_cli login [-h] [--tv]

optional arguments:
  -h, --help  show this help message and exit
  --tv        走 TV 端登录，供 TV 通道使用
"#;

pub const USAGE_LOGIN: &str = r#"usage: bilibili_cli login [-h] [--tv]
"#;

pub const SUBHELP_LIST: &str = r#"usage: bilibili_cli list [-h] [-n LIMIT] [--json] [--rights] target

positional arguments:
  target

optional arguments:
  -h, --help            show this help message and exit
  -n LIMIT, --limit LIMIT
  --json
  --rights              逐条判定是否会员/充电专属（每条多一次请求，慢）
"#;

pub const USAGE_LIST: &str = r#"usage: bilibili_cli list [-h] [-n LIMIT] [--json] [--rights] target
"#;

pub const SUBHELP_SEARCH: &str = r#"usage: bilibili_cli search [-h] [-n LIMIT]
                           [--sort {totalrank,play,new,dm,stow}] [--users]
                           [--desc] [--json]
                           keyword

positional arguments:
  keyword

optional arguments:
  -h, --help            show this help message and exit
  -n LIMIT, --limit LIMIT
                        条数，默认 20，上限 100
  --sort {totalrank,play,new,dm,stow}
                        totalrank 综合(默认) / play 最多播放 / new 最新 / dm 弹幕 / stow
                        收藏
  --users               搜 UP 主而不是视频
  --desc                补每条简介与收藏/点赞/投币（每条多一次请求，约 +130ms）
  --json
"#;

pub const USAGE_SEARCH: &str = r#"usage: bilibili_cli search [-h] [-n LIMIT]
                           [--sort {totalrank,play,new,dm,stow}] [--users]
                           [--desc] [--json]
                           keyword
"#;

pub const SUBHELP_INFO: &str = r#"usage: bilibili_cli info [-h] [--json] target

positional arguments:
  target

optional arguments:
  -h, --help  show this help message and exit
  --json
"#;

pub const USAGE_INFO: &str = r#"usage: bilibili_cli info [-h] [--json] target
"#;

pub const SUBHELP_DL: &str = r#"usage: bilibili_cli dl [-h] [--audio] [--quality QUALITY]
                       [--codec {avc,hevc,av1}] [--parallel N] [-o OUT]
                       [--subs] [--keep-parts] [--json]
                       target

positional arguments:
  target

optional arguments:
  -h, --help            show this help message and exit
  --audio               只下音频（m4a）
  --quality QUALITY     最高清晰度，如 1080P / 4K / 720P（默认最高可用）
  --codec {avc,hevc,av1}
                        视频编码偏好
  --parallel N          分段并行下载数（0/1 关闭）
  -o OUT, --out OUT     输出根目录（默认 /Users/limoon/Documents/bilibili_content）
  --subs                同时保存字幕（srt）
  --keep-parts          保留下载的临时分轨
  --json
"#;

pub const USAGE_DL: &str = r#"usage: bilibili_cli dl [-h] [--audio] [--quality QUALITY]
                       [--codec {avc,hevc,av1}] [--parallel N] [-o OUT]
                       [--subs] [--keep-parts] [--json]
                       target
"#;

pub const SUBHELP_DOCTOR: &str = r#"usage: bilibili_cli doctor [-h]

optional arguments:
  -h, --help  show this help message and exit
"#;

pub const USAGE_DOCTOR: &str = r#"usage: bilibili_cli doctor [-h]
"#;

pub const SUBHELP_HELP: &str = r#"usage: bilibili_cli help [-h] [topic]

positional arguments:
  topic       命令名，如 dl；留空看总览

optional arguments:
  -h, --help  show this help message and exit
"#;

pub const USAGE_HELP: &str = r#"usage: bilibili_cli help [-h] [topic]
"#;

pub const TOP_USAGE: &str = r#"usage: bilibili_cli [-h] {login,list,search,info,dl,doctor,help} ...
"#;
