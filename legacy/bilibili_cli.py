#!/usr/bin/env python3
"""
bilibili_cli —— B 站取用工具：登录 / 列投稿 / 看详情 / 下载。

设计原则
  - 单文件、纯标准库；外部只依赖 curl 与 ffmpeg
  - 无状态：不做订阅、不做轮询、不存 last_seen（订阅逻辑交给 agent / 个人助理插件）
  - 通道：app（首选，标准 CDN）→ tv（降级备用）；不碰 web 端，失败逐层报原因
  - 权益标记：每条视频标注「免费 / 会员专享 / 充电专属 / 付费」+「当前账号能否播放」
  - 错误分层：网络 / TLS / 超时 / 风控 / 密钥 / 权益 / 接口改版 / 频率 / 服务端
  - 登录态存 ~/.bilibili_cli/creds.json（权限 600），或读环境变量 BILI_SESSDATA

退出码
  0 成功    2 参数错误    3 网络/接口失败
  5 需要登录（或登录已过期）    6 缺少依赖（ffmpeg）
  7 权益不足（大会员 / 充电专属 / 版权内容）

用法
  bilibili_cli login                # 网页扫码登录（存 cookie）
  bilibili_cli login --tv           # 云视听小电视扫码（降级通道用）
  bilibili_cli list <uid|链接> -n 20 [--json] [--rights]
  bilibili_cli info <BV|链接> [--json]
  bilibili_cli dl <BV|链接> [--audio] [--quality 1080P] [--codec avc] [--subs] [--json]
  bilibili_cli doctor
  bilibili_cli help [command]       # 内置帮助
"""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import re
import shutil
import struct
import subprocess
import sys
import tempfile
import time
import urllib.parse
from datetime import datetime, timezone

HOME = os.path.expanduser("~")
DATA_DIR = os.path.join(HOME, ".bilibili_cli")
CREDS_PATH = os.path.join(DATA_DIR, "creds.json")
DEFAULT_OUT = os.path.join(HOME, "Documents", "bilibili_content")
SCHEMA = "bilibili_cli.v1"

UA_WEB = ("Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 "
          "(KHTML, like Gecko) Chrome/141.0.0.0 Safari/537.36")
UA_APP = "Mozilla/5.0 BiliDroid/7.73.0 (bbcallen@gmail.com) os/android model/Pixel up=1.0"
UA_TV = "Mozilla/5.0 BiliDroid/7.73.0"

APP_KEY, APP_SEC = "1d8b6e7d45233436", "560c52ccd288fed045859ed18bffd973"
TV_KEY, TV_SEC = "4409e2ce8ffd12b8", "59b43e04ad6965f34319062b478f83dd"

# qn 档位
QN = {127: "8K", 126: "杜比视界", 125: "HDR", 120: "4K", 116: "1080P60",
      112: "1080P+", 80: "1080P", 74: "720P60", 64: "720P", 32: "480P", 16: "360P"}
QN_BY_NAME = {v: k for k, v in QN.items()}
QN_BY_NAME.update({"720p": 64, "1080p": 80, "480p": 32, "360p": 16,
                   "4k": 120, "1080p+": 112})

CODECS = {"avc": 7, "hevc": 12, "av1": 13}


# ------------------------------------------------------------------ 错误模型

CURL_EXIT = {
    6:  ("network", "DNS 解析失败：域名可能已停用或被屏蔽"),
    7:  ("network", "连接失败：域名或端口不可达"),
    28: ("timeout", "请求超时：网络慢或被限速"),
    35: ("tls", "TLS 握手失败：证书问题或中间设备干扰"),
    52: ("empty", "服务端返回空响应"),
    56: ("network", "接收数据失败：连接被重置（常伴随风控）"),
    60: ("tls", "证书校验失败"),
}

BIZ_CODES = {
    -101: ("auth", "账号未登录：需要登录后再试"),
    -352: ("blocked", "风控校验失败"), -412: ("blocked", "请求被风控拦截"),
    -403: ("auth", "访问权限不足：签名无效，或 appkey/appsec 已失效"),
    -404: ("interface", "资源不存在：ID 错误、接口路径变更，或 appkey 无效"),
    -400: ("interface", "请求参数不被接受：接口参数可能已改版"),
    -10403: ("rights", "地区或权限限制：需要对应权益"),
    -509: ("rate_limit", "请求过于频繁"), -799: ("rate_limit", "请求过于频繁"),
    -500: ("server", "服务端内部错误"), -1: ("server", "应用不存在或服务暂不可用"),
    87007: ("rights", "充电专属视频：需要给该 UP 主充电"),
    87008: ("rights", "充电专属视频：当前账号未充电"),
    76150: ("rights", "稿件不可见：通常是充电专属/会员专享等权限限制"),
}

HINTS = {
    "network": "检查网络 / DNS；key 本身没问题",
    "tls": "稍后重试，或换网络", "timeout": "稍后重试或降低频率",
    "empty": "重试一次", "blocked": "降低频率后重试；持续则换通道",
    "auth": "重新 `bilibili_cli login`；若刚登录仍报此错，说明接口改了鉴权",
    "interface": "接口可能已改版：多了必填参数或返回结构变了",
    "rights": "这是账号权益问题，不是程序问题：大会员 / 充电 / 版权内容各自需要对应许可",
    "rate_limit": "降低频率", "server": "B 站侧问题，稍后再试",
    "unknown": "把原始响应贴出来再判断",
}


class CliError(Exception):
    def __init__(self, category, summary, *, endpoint="", http=0, code=None,
                 raw="", hint="", channel=""):
        super().__init__(summary)
        self.category = category
        self.summary = summary
        self.endpoint = endpoint
        self.http = http
        self.code = code
        self.raw = raw
        self.hint = hint or HINTS.get(category, "")
        self.channel = channel


LABELS = {"network": "网络 / 域名", "tls": "TLS", "timeout": "超时", "empty": "空响应",
          "blocked": "风控拦截", "auth": "密钥 / 登录态", "interface": "接口改版",
          "rights": "账号权益", "rate_limit": "频率限制", "server": "服务端",
          "missing": "缺少依赖", "unknown": "未知"}


def report(e: CliError) -> None:
    print(f"\n✗ 失败层级：{LABELS.get(e.category, e.category)}"
          + (f"（通道 {e.channel}）" if e.channel else ""), file=sys.stderr)
    print(f"  原因：{e.summary}", file=sys.stderr)
    if e.endpoint:
        print(f"  接口：{e.endpoint}", file=sys.stderr)
    meta = []
    if e.http:
        meta.append(f"HTTP {e.http}")
    if e.code is not None:
        meta.append(f"业务码 {e.code}")
    if meta:
        print(f"  细节：{' · '.join(meta)}", file=sys.stderr)
    if e.raw:
        flat = re.sub(r"\s+", " ", str(e.raw))[:160]
        print(f"  原文：{flat}", file=sys.stderr)
    if e.hint:
        print(f"  建议：{e.hint}", file=sys.stderr)


def die(msg, code=2):
    print(f"错误: {msg}", file=sys.stderr)
    sys.exit(code)


# ------------------------------------------------------------------ HTTP

def curl(url: str, *, ua: str = UA_WEB, referer: str = "https://www.bilibili.com/",
         timeout: int = 30, cookie: str = "", post: str | None = None,
         save_to: str | None = None, resume: bool = False, quiet: bool = False,
         jar: str = "") -> tuple[str, int]:
    """返回 (body, http_status)。save_to 时 body 为空。jar 指定时用 cookie jar 收/发 cookie。"""
    cmd = ["curl", "-sS", "--compressed", "--http2", "--max-time", str(timeout), "-A", ua]
    if referer:
        cmd += ["-H", f"Referer: {referer}"]
    cmd += ["-H", "Accept: application/json, text/plain, */*"]
    if jar:
        cmd += ["-c", jar, "-b", jar]
    if cookie:
        cmd += ["-H", f"Cookie: {cookie}"]
    if post is not None:
        cmd += ["-X", "POST", "--data", post]
    if save_to:
        cmd += ["-o", save_to, "--retry", "3", "--retry-delay", "2"]
        if resume:
            cmd += ["-C", "-"]
        if not quiet:
            cmd += ["-w", "%{http_code} %{size_download}"]
        else:
            cmd += ["-w", "%{http_code}"]
    else:
        cmd += ["-w", "\n__HTTP__%{http_code}"]

    p = subprocess.run(cmd + [url], capture_output=True, text=True)
    endpoint = url.split("?", 1)[0].replace("https://", "")
    if p.returncode != 0:
        cat, msg = CURL_EXIT.get(p.returncode,
                                 ("network", f"curl 退出码 {p.returncode}：{p.stderr.strip()[:120]}"))
        raise CliError(cat, msg, endpoint=endpoint)

    if save_to:
        parts = p.stdout.split()
        try:
            http = int(parts[0])
        except (IndexError, ValueError):
            http = 0
        if http >= 400:
            raise CliError("network", f"下载返回 HTTP {http}", endpoint=endpoint, http=http)
        return "", http

    body, _, tail = p.stdout.rpartition("\n__HTTP__")
    try:
        http = int(tail.strip())
    except ValueError:
        http = 0
    return body, http


def fetch_json(url: str, *, ua=UA_WEB, referer="https://www.bilibili.com/",
               cookie="", post=None, channel="", jar: str = "",
               allow_codes: tuple = ()) -> dict:
    """取 JSON。allow_codes 里的业务码不当作错误（用于轮询类接口）。"""
    body, http = curl(url, ua=ua, referer=referer, cookie=cookie, post=post, jar=jar)
    endpoint = url.split("?", 1)[0].replace("https://", "")

    if http >= 500:
        raise CliError("server", f"HTTP {http}：服务端错误", endpoint=endpoint,
                       http=http, raw=body[:200], channel=channel)
    if http in (403, 412, 429) or body.lstrip()[:1] == "<":
        raise CliError("blocked", f"HTTP {http}，返回的是网页/拦截页而不是 JSON",
                       endpoint=endpoint, http=http, raw=body[:200], channel=channel)
    try:
        j = json.loads(body)
    except json.JSONDecodeError:
        raise CliError("unknown", "返回内容不是 JSON", endpoint=endpoint,
                       http=http, raw=body[:200], channel=channel)

    code = j.get("code")
    if code == 0 or code in allow_codes:
        return j
    entry = BIZ_CODES.get(code)
    if entry:
        cat, msg = entry
        hint = ""
    else:
        cat, msg = "unknown", f"业务码 {code}: {j.get('message')}"
        hint = HINTS["unknown"]
    raise CliError(cat, msg, endpoint=endpoint, http=http, code=code,
                   raw=str(j.get("message") or ""), hint=hint, channel=channel)


# ------------------------------------------------------------------ 签名

def sign_app(params: dict, key=APP_KEY, sec=APP_SEC) -> str:
    p = dict(params)
    p["appkey"] = key
    p["ts"] = int(time.time())
    q = "&".join(f"{k}={p[k]}" for k in sorted(p))
    p["sign"] = hashlib.md5((q + sec).encode()).hexdigest()
    return urllib.parse.urlencode(p)


# ------------------------------------------------------------------ 登录态

def load_creds() -> dict:
    try:
        with open(CREDS_PATH, encoding="utf-8") as f:
            c = json.load(f)
    except (FileNotFoundError, json.JSONDecodeError):
        c = {}
    env = os.environ.get("BILI_SESSDATA")
    if env and not c.get("cookie"):
        c["cookie"] = f"SESSDATA={env}"
    return c


def save_creds(c: dict) -> None:
    os.makedirs(DATA_DIR, exist_ok=True)
    tmp = CREDS_PATH + ".tmp"
    with open(tmp, "w", encoding="utf-8") as f:
        json.dump(c, f, ensure_ascii=False, indent=2)
    os.chmod(tmp, 0o600)
    os.replace(tmp, CREDS_PATH)


def cookie_header(c: dict) -> str:
    return c.get("cookie") or ""


# ------------------------------------------------------------------ 扫码登录

def _render_qr(text: str) -> bool:
    """真终端 → 直接画 ASCII 二维码；非 TTY（如 agent 代跑）→ 弹 PNG 窗口。"""
    qr = shutil.which("qrencode")
    if qr and sys.stdout.isatty():
        p = subprocess.run([qr, "-t", "ANSIUTF8", "-m", "1", text],
                           capture_output=True, text=True)
        if p.returncode == 0 and p.stdout:
            print(p.stdout)
            return True
    if shutil.which("uv"):
        png = "/tmp/bilibili_cli_qr.png"
        code = ("import qrcode,sys;"
                "img=qrcode.make(sys.argv[1]);img.save(sys.argv[2])")
        p = subprocess.run(["uv", "run", "--quiet", "--with", "qrcode[pil]",
                            "python", "-c", code, text, png], capture_output=True, text=True)
        if p.returncode == 0 and os.path.exists(png):
            print(f"（非终端环境，已生成二维码图片并打开：{png}）")
            subprocess.run(["open", png])
            return True
    if qr:  # 无 uv 时退而求其次：纯文本块字符
        p = subprocess.run([qr, "-t", "UTF8", "-m", "1", text],
                           capture_output=True, text=True)
        if p.returncode == 0 and p.stdout:
            print(p.stdout)
            return True
    return False


def login_web() -> int:
    gen = fetch_json("https://passport.bilibili.com/x/passport-login/web/qrcode/generate")
    data = gen.get("data") or {}
    url, key = data.get("url"), data.get("qrcode_key")
    if not (url and key):
        raise CliError("interface", "二维码接口返回结构异常",
                       raw=json.dumps(gen, ensure_ascii=False)[:200])

    print("请用「哔哩哔哩」App 扫码登录：\n")
    if not _render_qr(url):
        print(f"（无法渲染二维码，请手动在浏览器打开并在 App 内确认）\n{url}\n")

    deadline = time.time() + 180
    last = ""
    jar = "/tmp/bilibili_cli_login.jar"
    if os.path.exists(jar):
        os.unlink(jar)
    while time.time() < deadline:
        time.sleep(2)
        # poll 必须在 cookie jar 下请求，Set-Cookie（SESSDATA）才会落到 jar 里
        j = fetch_json("https://passport.bilibili.com/x/passport-login/web/qrcode/poll"
                       f"?qrcode_key={key}", jar=jar)
        d = j.get("data") or {}
        code = d.get("code")
        msg = d.get("message") or ""
        if msg and msg != last:
            print(f"  状态：{msg}")
            last = msg
        if code == 0:
            pairs = _read_jar(jar)
            if not pairs.get("SESSDATA"):
                raise CliError("auth", "登录成功但没拿到 SESSDATA：cookie 上报链路异常",
                               raw=", ".join(pairs) or "jar 为空")
            c = load_creds()
            c["cookie"] = "; ".join(f"{k}={v}" for k, v in pairs.items())
            c["cookie_map"] = pairs
            c["saved_at"] = int(time.time())
            save_creds(c)
            who = _nav_name(cookie_header(c))
            print(f"\n✓ 网页登录成功{'：' + who if who else ''}")
            print(f"  cookie 已存 {CREDS_PATH}（{len(pairs)} 项，权限 600）")
            return 0
        if code == 86038:
            raise CliError("auth", "二维码已过期，请重新执行 login")
    raise CliError("timeout", "等待扫码超时（180 秒）")


def _read_jar(path: str) -> dict:
    """读 curl 的 cookie jar。

    注意：HttpOnly cookie 在 Netscape 格式里写作 `#HttpOnly_.bilibili.com	...`，
    不能当注释跳过（SESSDATA 就是 HttpOnly）。
    """
    pairs: dict[str, str] = {}
    if os.path.exists(path):
        with open(path, encoding="utf-8") as f:
            for raw in f:
                line = raw.rstrip("\n")
                if not line.strip():
                    continue
                if line.startswith("#HttpOnly_"):
                    line = line[len("#HttpOnly_"):]
                elif line.startswith("#"):
                    continue
                parts = line.split("\t")
                if len(parts) >= 7:
                    pairs[parts[5]] = parts[6]
    return pairs


def login_tv() -> int:
    params = {"local_id": "0"}
    q = sign_app(params, TV_KEY, TV_SEC)
    gen = fetch_json("https://passport.bilibili.com/x/passport-tv-login/qrcode/auth_code",
                     ua=UA_TV, referer="", post=q)
    d = gen.get("data") or {}
    url, auth_code = d.get("url"), d.get("auth_code")
    if not (url and auth_code):
        raise CliError("interface", "TV 二维码接口返回结构异常",
                       raw=json.dumps(gen, ensure_ascii=False)[:200])
    print("请用「哔哩哔哩」App 扫码，并在手机上确认「在电视上登录」：\n")
    if not _render_qr(url):
        print(f"{url}\n")

    deadline = time.time() + 180
    last_tv = ""
    while time.time() < deadline:
        time.sleep(2)
        p = {"auth_code": auth_code, "local_id": "0"}
        j = fetch_json("https://passport.bilibili.com/x/passport-tv-login/qrcode/poll",
                       ua=UA_TV, referer="", post=sign_app(p, TV_KEY, TV_SEC),
                       allow_codes=(86039, 86090, 86101, 86038))
        td = j.get("data") or {}
        if td.get("access_token"):
            c = load_creds()
            c["tv_access_token"] = td["access_token"]
            c["tv_refresh_token"] = td.get("refresh_token")
            c["tv_mid"] = (td.get("token_info") or {}).get("mid")
            c["tv_saved_at"] = int(time.time())
            save_creds(c)
            print(f"\n✓ TV 登录成功，access_token 已存 {CREDS_PATH}")
            print("  已作为降级备用通道（首选仍是 app）")
            return 0
        state = j.get("code") or 0
        msg = j.get("message") or ""
        if msg and msg != last_tv:
            print(f"  状态：{msg}")
            last_tv = msg
        if state in (0, 86039, 86090, 86101):
            continue
        if state == 86038:
            raise CliError("auth", "TV 二维码已过期，请重新执行 login --tv")
        raise CliError("auth", f"TV 登录失败（{state}）：{msg or '未知'}")
    raise CliError("timeout", "等待扫码超时（180 秒）")


def _nav_name(cookie: str) -> str:
    try:
        nav = fetch_json("https://api.bilibili.com/x/web-interface/nav", cookie=cookie)
        return (nav.get("data") or {}).get("uname") or ""
    except CliError:
        return ""


# ------------------------------------------------------------------ 数据接口

def parse_target(t: str) -> dict:
    """识别 uid / BV 号 / 各种链接。"""
    t = t.strip().strip('"').strip("'")

    if "search.bilibili.com" in t:
        die("这是 B 站的「搜索结果页」，不是 UP 主主页，里面没有 uid。\n"
            "  正确做法：点进该 UP 主的主页，复制 space.bilibili.com/数字 的链接；\n"
            "  或者直接给数字 uid（例如 23947287）。", 2)

    m = re.search(r"(BV[0-9A-Za-z]{10})", t)
    if m:
        return {"type": "video", "bvid": m.group(1)}
    m = re.search(r"(?:av|aid=)(\d+)", t)
    if m:
        return {"type": "video", "aid": int(m.group(1))}
    m = re.search(r"space\.bilibili\.com/(\d+)", t)
    if m:
        return {"type": "user", "uid": m.group(1)}
    if t.isdigit():
        return {"type": "user", "uid": t}
    die(f"无法识别 {t!r}：既不是 uid 也不是 BV 号\n"
        f"  可用形式：数字 uid / space.bilibili.com/<uid>... / BV 号或视频链接\n"
        f"  提示：链接里有 ? 时请用引号包住，否则 shell 会当成通配符报错", 2)


def view(bvid: str) -> dict:
    j = fetch_json(f"https://api.bilibili.com/x/web-interface/view?bvid={bvid}")
    d = j.get("data")
    if not isinstance(d, dict):
        raise CliError("interface", "view 接口没有 data 字段", raw=json.dumps(j)[:200])
    return d


def user_videos(uid: str, n: int = 20) -> dict:
    params = {"vmid": uid, "order": "pubdate", "ps": str(max(n, 5)), "pn": "1",
              "mobi_app": "android", "platform": "android", "build": "7730200",
              "c_locale": "zh_CN", "s_locale": "zh_CN"}
    j = fetch_json("https://app.bilibili.com/x/v2/space/archive/cursor?" + sign_app(params),
                   ua=UA_APP, referer="")
    d = j.get("data")
    if not isinstance(d, dict) or "item" not in d:
        raise CliError("interface", "投稿接口结构变了（缺少 data.item）",
                       raw=json.dumps(list((d or {}).keys()), ensure_ascii=False)[:160])
    items = [it for it in (d.get("item") or [])
             if it.get("goto") == "av" and it.get("bvid")][:n]
    return {"total": d.get("count") or len(items), "items": items}


def user_name(uid: str) -> str:
    try:
        j = fetch_json(f"https://api.bilibili.com/x/web-interface/card?mid={uid}",
                       referer=f"https://space.bilibili.com/{uid}/")
        return ((j.get("data") or {}).get("card") or {}).get("name") or ""
    except CliError:
        return ""


def iso(ts: int) -> str:
    return datetime.fromtimestamp(ts, timezone.utc).astimezone().isoformat(timespec="seconds")


def fmt_dur(sec) -> str:
    sec = int(sec or 0)
    h, m, s = sec // 3600, sec % 3600 // 60, sec % 60
    return f"{h}:{m:02d}:{s:02d}" if h else f"{m}:{s:02d}"


ACCESS_TEXT = {"yes": "可播放", "limited": "仅低清晰度（未登录或权益不足）",
               "no": "无播放权限", "unknown": "未知"}


def entitlement_of(d: dict) -> dict:
    """从 view 数据判定这条视频“要不要钱”。"""
    rights = d.get("rights") or {}
    charging = bool(d.get("is_upower_exclusive")) or bool(rights.get("elec"))
    vip_only = bool(rights.get("pay") or rights.get("movie") or rights.get("arc_pay"))
    paid = bool(rights.get("ugc_pay"))
    if charging:
        kind, label = "charging", "充电专属"
    elif paid:
        kind, label = "paid", "付费内容"
    elif vip_only:
        kind, label = "vip", "会员专享"
    else:
        kind, label = "free", "免费"
    return {
        "kind": kind, "label": label,
        "vip_only": vip_only, "charging_only": charging, "paid": paid,
        "flags": {k: rights.get(k) for k in ("pay", "ugc_pay", "arc_pay", "elec", "movie")},
        "upower_exclusive": bool(d.get("is_upower_exclusive")),
    }


def access_of(probes: list, ent: dict | None = None) -> dict:
    """从各通道探测结果反推“当前账号能不能播”。"""
    ok = [p for p in probes if p.get("ok")]
    live = [p for p in ok if (p.get("tracks") or 0) > 0]
    if live:
        best = max(p.get("quality_id") or 0 for p in live)
        state = "yes" if best >= 64 else "limited"
        return {"state": state, "text": ACCESS_TEXT[state],
                "best_quality": QN.get(best, str(best)),
                "via": "/".join(p["channel"] for p in live)}
    if ok:
        if ent and ent.get("kind") != "free":
            return {"state": "no", "text": ACCESS_TEXT["no"],
                    "reason": f"{ent['label']}：通道未返回可用的视频流"}
        return {"state": "unknown", "text": "通道未返回可用的视频流"}
    fails = [p for p in probes if not p.get("ok")]
    rights_fail = [p for p in fails if p.get("category") == "rights"]
    if rights_fail:
        return {"state": "no", "text": ACCESS_TEXT["no"],
                "reason": rights_fail[0].get("reason")}
    if fails and all(p.get("category") == "auth" for p in fails):
        return {"state": "unknown", "text": "需要登录后才能判断",
                "reason": fails[0].get("reason")}
    return {"state": "unknown", "text": ACCESS_TEXT["unknown"],
            "reason": fails[0].get("reason") if fails else ""}


# ------------------------------------------------------------------ playurl（三通道）

def _qn_value(name: str | None) -> int:
    if not name:
        return 127
    k = name.strip().lower()
    if k.isdigit():
        return int(k)
    return QN_BY_NAME.get(name.strip(), QN_BY_NAME.get(k, 127))


def playurl(channel: str, aid: int, cid: int, qn: int, creds: dict,
            bvid: str = "") -> dict:
    cookie = cookie_header(creds)
    if channel == "tv":
        token = creds.get("tv_access_token")
        if not token:
            raise CliError("auth", "TV 通道需要 TV 登录态（先执行 login --tv）",
                           channel="tv")
        params = {"access_key": token, "actionKey": "appkey", "build": "36700100",
                  "cid": cid, "fourk": "1", "is_proj": "1",
                  "mobile_access_key": token, "object_id": aid, "ogv_aid": "",
                  "platform": "ios", "playurl_type": "1", "protocol": "0",
                  "qn": qn, "fnval": "4048", "fnver": "0"}
        j = fetch_json("https://api.bilibili.com/x/tv/playurl?" + sign_app(params, TV_KEY, TV_SEC),
                       ua=UA_TV, referer="", channel="tv")
    elif channel == "app":
        params = {"avid": aid, "cid": cid, "qn": qn, "fnval": "4048", "fnver": "0",
                  "fourk": "1", "mobi_app": "android", "platform": "android",
                  "build": "7730200"}
        if creds.get("access_key"):
            params["access_key"] = creds["access_key"]
        j = fetch_json("https://api.bilibili.com/x/player/playurl?" + sign_app(params),
                       ua=UA_APP, referer="", cookie=cookie, channel="app")
    else:
        raise CliError("interface", f"未知通道 {channel}", channel=channel)
    return j.get("data") or {}


def pick_streams(dash: dict, want_qn: int, codec: str = "avc") -> tuple[dict, dict]:
    videos = [v for v in (dash.get("video") or []) if v.get("baseUrl") or v.get("base_url")]
    audios = [a for a in (dash.get("audio") or []) if a.get("baseUrl") or a.get("base_url")]
    if not videos:
        raise CliError("rights", "playurl 没有返回任何视频流：通常是权益或风控限制")

    allowed = [v for v in videos if (v.get("id") or 0) <= want_qn] or videos
    pref = CODECS.get(codec, 7)
    same_codec = [v for v in allowed if v.get("codecid") == pref]
    pool = same_codec or allowed
    best = max(pool, key=lambda v: (v.get("id") or 0, v.get("height") or 0, v.get("bandwidth") or 0))
    audio = max(audios, key=lambda a: a.get("bandwidth") or 0) if audios else {}
    return best, audio


def _url_of(s: dict) -> str:
    return s.get("baseUrl") or s.get("base_url") or ""


def _backup_of(s: dict) -> list:
    return s.get("backupUrl") or s.get("backup_url") or []


# ------------------------------------------------------------------ 下载

def _curl_dl(url: str, dest: str, referer: str, resume: bool = True, attempts: int = 3) -> None:
    """下载分轨。中断可自动续传；最终失败则删除半成品，绝不留下“看起来正常”的文件。"""
    if not resume and os.path.exists(dest):
        os.unlink(dest)
    last: CliError | None = None
    for i in range(attempts):
        try:
            curl(url, referer=referer, save_to=dest, resume=resume, timeout=3600)
            size = os.path.getsize(dest) if os.path.exists(dest) else 0
            if size < 1024:
                raise CliError("empty", f"下载文件过小（{size} B）")
            return
        except CliError as e:
            last = e
            got = os.path.getsize(dest) if os.path.exists(dest) else 0
            print(f"  · 下载中断（第 {i + 1}/{attempts} 次，已 {got / 1048576:.1f} MB）："
                  f"{e.summary} → 续传重试", file=sys.stderr)
            time.sleep(1)
    if os.path.exists(dest):
        os.unlink(dest)
    if last:
        raise last
    raise CliError("network", "下载失败（未知原因）")


def _merge(video: str, audio: str, out: str) -> None:
    ff = shutil.which("ffmpeg")
    if not ff:
        raise CliError("missing", "没有 ffmpeg，无法合并音视频（brew install ffmpeg）")
    p = subprocess.run([ff, "-y", "-loglevel", "error", "-i", video, "-i", audio,
                        "-c", "copy", "-movflags", "+faststart", out],
                       capture_output=True, text=True)
    if p.returncode != 0:
        raise CliError("unknown", f"ffmpeg 合并失败：{p.stderr.strip()[:200]}")


def safe_name(s: str, limit: int = 80) -> str:
    s = re.sub(r"[/\\:*?\"<>|\n\r\t]", "_", s).strip()
    s = re.sub(r"\s+", " ", s)
    return s[:limit].rstrip(" .")


def cmd_dl(target: str, *, audio_only: bool, quality: str | None, codec: str,
           out_dir: str, keep_parts: bool, subs: bool, as_json: bool) -> int:
    t = parse_target(target)
    if t["type"] != "video":
        die("dl 需要视频链接或 BV 号（列投稿请用 list）", 2)
    d = view(t["bvid"])
    aid, cid = d.get("aid"), d.get("cid")
    title, pub = d.get("title") or t["bvid"], d.get("pubdate") or int(time.time())
    owner = (d.get("owner") or {}).get("name") or "unknown"
    want_qn = _qn_value(quality)
    creds = load_creds()

    chain = ["app", "tv"]
    ent0 = entitlement_of(d)
    data, used, failures = None, "", []
    for ch in chain:
        try:
            data = playurl(ch, aid, cid, want_qn, creds, bvid=t["bvid"])
            used = ch
            break
        except CliError as e:
            failures.append((ch, e))
    if data is None:
        print(f"\n🏷 内容类型：{ent0['label']}", file=sys.stderr)
        print("TV 与 APP 两个通道都失败：", file=sys.stderr)
        for ch, e in failures:
            report(e)
        return 7 if any(e.category == "rights" for _, e in failures) else 3

    dash = data.get("dash") or {}
    if not dash:
        raise CliError("rights", f"[{used}] playurl 未返回 dash 流：可能是版权/权益限制",
                       channel=used)
    video, audio = pick_streams(dash, want_qn, codec)

    qn_id = video.get("id") or 0
    qn_text = QN.get(qn_id, str(qn_id))
    ent = entitlement_of(d)
    state = "yes" if (qn_id >= 64 or quality is not None) else "limited"
    ent["my_access"] = {"state": state, "text": ACCESS_TEXT[state],
                        "best_quality": qn_text, "via": used}
    if qn_id < 64 and quality is None:
        print(f"⚠ 当前只拿到 {qn_text}（{qn_id}）：账号权益或登录态不足。"
              f"试试 `bilibili_cli login` / `login --tv`", file=sys.stderr)
    if ent["kind"] != "free":
        print(f"  🏷 {ent['label']} · {ent['my_access']['text']}")

    base = os.path.join(out_dir, safe_name(owner))
    os.makedirs(base, exist_ok=True)
    stem = f"{datetime.fromtimestamp(pub).strftime('%Y-%m-%d')}_{safe_name(title)}"
    referer = f"https://www.bilibili.com/video/{t['bvid']}"

    if audio_only:
        if not audio:
            raise CliError("rights", f"[{used}] 没有独立音频轨，无法只下音频", channel=used)
        out = os.path.join(base, stem + ".m4a")
        kbps = (audio.get("bandwidth") or 0) // 1000
        print(f"→ [{used}] 音频 · {kbps} kbps")
        _curl_dl(_url_of(audio), out, referer)
    else:
        vtmp = os.path.join(base, f".{stem}.video.m4s")
        out = os.path.join(base, stem + ".mp4")
        print(f"→ [{used}] 视频 {qn_text} · {video.get('width')}x{video.get('height')} "
              f"· {video.get('codecs')}")
        _curl_dl(_url_of(video), vtmp, referer)
        if audio:
            atmp = os.path.join(base, f".{stem}.audio.m4s")
            _curl_dl(_url_of(audio), atmp, referer)
            _merge(vtmp, atmp, out)
            if not keep_parts:
                for p in (vtmp, atmp):
                    os.path.exists(p) and os.unlink(p)
        else:
            os.replace(vtmp, out)

    meta = {
        "schema": SCHEMA,
        "id": t["bvid"], "aid": aid, "cid": cid,
        "title": title,
        "url": f"https://www.bilibili.com/video/{t['bvid']}",
        "author": {"uid": (d.get("owner") or {}).get("mid"),
                   "name": owner},
        "published_at": iso(pub),
        "published_date": datetime.fromtimestamp(pub).strftime("%Y-%m-%d"),
        "duration_sec": d.get("duration") or 0,
        "duration_text": fmt_dur(d.get("duration")),
        "quality": qn_text, "quality_id": qn_id,
        "codec": video.get("codecs"),
        "channel": used,
        "entitlement": ent,
        "audio_only": audio_only,
        "desc": (d.get("desc") or "")[:2000],
        "file": out,
        "downloaded_at": iso(int(time.time())),
    }
    side = os.path.splitext(out)[0] + ".info.json"
    with open(side, "w", encoding="utf-8") as f:
        json.dump(meta, f, ensure_ascii=False, indent=2)

    if subs:
        _save_subs(t["bvid"], cid, os.path.splitext(out)[0], creds, referer)

    size = os.path.getsize(out) / 1048576 if os.path.exists(out) else 0
    if as_json:
        print(json.dumps({**meta, "bytes": int(size * 1048576)}, ensure_ascii=False, indent=2))
    else:
        print(f"✓ {out}  ({size:.1f} MB)")
        print(f"  元数据 {os.path.basename(side)}")
    return 0


def _save_subs(bvid: str, cid: int, stem: str, creds: dict, referer: str) -> None:
    try:
        j = fetch_json(f"https://api.bilibili.com/x/player/v2?bvid={bvid}&cid={cid}",
                       referer=referer, cookie=cookie_header(creds))
        items = ((j.get("data") or {}).get("subtitle") or {}).get("subtitles") or []
    except CliError:
        return
    for i, s in enumerate(items[:2]):
        lang = s.get("lan") or f"sub{i}"
        url = s.get("subtitle_url") or ""
        if url.startswith("//"):
            url = "https:" + url
        if not url:
            continue
        try:
            body, _ = curl(url, referer=referer)
            data = json.loads(body)
            lines = []
            for idx, seg in enumerate(data.get("body") or [], 1):
                start = seg.get("from", 0)
                end = seg.get("to", 0)

                def ts(x):
                    h, m, sec = int(x // 3600), int(x % 3600 // 60), x % 60
                    return f"{h:02d}:{m:02d}:{sec:06.3f}".replace(".", ",")

                lines += [str(idx), f"{ts(start)} --> {ts(end)}", seg.get("content", ""), ""]
            path = f"{stem}.{lang}.srt"
            with open(path, "w", encoding="utf-8") as f:
                f.write("\n".join(lines))
            print(f"  字幕 {os.path.basename(path)}")
        except Exception:
            continue


# ------------------------------------------------------------------ 查询命令

def cmd_list(target: str, n: int, as_json: bool, rights: bool = False) -> int:
    t = parse_target(target)
    if t["type"] != "user":
        die("list 需要 UP 主 uid 或空间链接", 2)
    uid = t["uid"]
    res = user_videos(uid, n)
    name = user_name(uid)
    items = []
    for it in res["items"]:
        row = {
            "id": it["bvid"],
            "title": it.get("title") or "",
            "url": f"https://www.bilibili.com/video/{it['bvid']}",
            "published_at": iso(it.get("ctime") or 0),
            "published_date": datetime.fromtimestamp(it.get("ctime") or 0).strftime("%Y-%m-%d"),
            "duration_sec": it.get("duration") or 0,
            "duration_text": fmt_dur(it.get("duration")),
            "views": it.get("play") or 0,
            "cover": it.get("cover") or "",
            "paid": bool(it.get("is_ugcpay")),
            "badges": [b.get("text") for b in (it.get("badges") or []) if b.get("text")],
        }
        if rights:  # 精确判定要不要钱（多一次 view 调用）
            try:
                ent = entitlement_of(view(it["bvid"]))
                row["entitlement"] = {k: ent[k] for k in ("kind", "label", "vip_only",
                                                          "charging_only", "paid")}
            except CliError as e:
                row["entitlement"] = {"kind": "unknown", "label": e.summary}
            time.sleep(0.15)
        items.append(row)
    if as_json:
        print(json.dumps({"schema": SCHEMA, "uid": uid,
                          "author": {"uid": uid, "name": name},
                          "total": res["total"], "items": items},
                         ensure_ascii=False, indent=2))
        return 0
    print(f"{name or uid}（uid {uid}）共 {res['total']} 个投稿 · 最新 {len(items)} 条\n")
    for v in items:
        tag = ""
        if v.get("entitlement"):
            tag = f" [{v['entitlement']['label']}]" if v["entitlement"]["kind"] != "free" else ""
        elif v.get("paid"):
            tag = " [付费]"
        print(f"  {v['published_date']}  {v['duration_text']:>7}  {v['title'][:44]}{tag}")
        print(f"     {v['url']}")
    return 0


def cmd_info(target: str, as_json: bool) -> int:
    t = parse_target(target)
    if t["type"] != "video":
        die("info 需要视频链接或 BV 号", 2)
    d = view(t["bvid"])
    creds = load_creds()
    aid, cid = d.get("aid"), d.get("cid")
    pages = d.get("pages") or []
    rights = d.get("rights") or {}

    probes = []
    for ch in ("app", "tv"):
        try:
            pd = playurl(ch, aid, cid, 127, creds, bvid=t["bvid"])
            dash = pd.get("dash") or {}
            vids = dash.get("video") or []
            best = max([v.get("id") or 0 for v in vids], default=0)
            probes.append({"channel": ch, "ok": True, "max_quality": QN.get(best, str(best)),
                           "quality_id": best,
                           "tracks": len(vids),
                           "has_audio": bool(dash.get("audio")),
                           "available": [QN.get(q, str(q)) for q in (pd.get("accept_quality") or [])]})
        except CliError as e:
            probes.append({"channel": ch, "ok": False, "category": e.category,
                           "reason": e.summary})

    pay_flags = entitlement_of(d)
    access = access_of(probes, pay_flags)
    out = {
        "schema": SCHEMA,
        "id": t["bvid"], "aid": aid, "cid": cid,
        "title": d.get("title"),
        "url": f"https://www.bilibili.com/video/{t['bvid']}",
        "author": {"uid": (d.get("owner") or {}).get("mid"),
                   "name": (d.get("owner") or {}).get("name")},
        "published_at": iso(d.get("pubdate") or 0),
        "published_date": datetime.fromtimestamp(d.get("pubdate") or 0).strftime("%Y-%m-%d"),
        "duration_sec": d.get("duration") or 0,
        "duration_text": fmt_dur(d.get("duration")),
        "pages": [{"page": p.get("page"), "cid": p.get("cid"),
                   "title": p.get("part"), "duration": p.get("duration")} for p in pages],
        "entitlement": {**pay_flags, "my_access": access},
        "channels": probes,
    }
    if as_json:
        print(json.dumps(out, ensure_ascii=False, indent=2))
        return 0

    print(f"{out['title']}\n  {out['author']['name']} · {out['published_date']} · "
          f"{out['duration_text']} · {t['bvid']}")
    if pay_flags["kind"] != "free":
        print(f"  🏷 内容类型：{pay_flags['label']}")
    print(f"  🔑 当前账号：{access['text']}"
          + (f" · {access['reason']}" if access.get("reason") else "")
          + (f" · 最高 {access['best_quality']}" if access.get("best_quality") else ""))
    if len(pages) > 1:
        print(f"  分P：{len(pages)} 个")
        for p in pages[:10]:
            print(f"    P{p['page']}  {p['title'][:40]}  {fmt_dur(p['duration'])}")
    print("  通道探测：")
    for p in probes:
        if p["ok"]:
            if (p.get("tracks") or 0) > 0:
                print(f"    ✓ {p['channel']:<4} 最高 {p['max_quality']} · "
                      f"{p['tracks']} 条视频轨 · 音轨{'有' if p['has_audio'] else '无'}")
            else:
                print(f"    ✓ {p['channel']:<4} 接口通，但未返回可用视频流（权益/风控限制）")
        else:
            print(f"    ✗ {p['channel']:<4} [{p['category']}] {p['reason']}")
    return 0


HELP_DETAIL = {
    "login": """bcli login              网页扫码登录 → cookie（决定能否 1080P / 字幕）
bcli login --tv         云视听小电视扫码 → access_token（仅作为下载的降级通道）

扫码位置：真终端里直接画 ASCII 二维码；非终端（脚本 / agent）生成 PNG 并自动打开。
有效期：cookie 约数月，TV token 约数月；过期会明确提示重新登录。
存储：~/.bilibili_cli/creds.json（权限 600）。
兜底：环境变量 BILI_SESSDATA 可直接提供 cookie。""",

    "list": """bcli list <uid|链接> [-n 20] [--json] [--rights]

  target    纯数字 uid，或 space.bilibili.com 链接（带 spm 参数也行）
  -n        条数，默认 20
  --json    机器可读（schema: bilibili_cli.v1）
  --rights  逐条判定是否会员/充电专属（每条多一次请求，慢）

例：bcli list 23947287 -n 5
    bcli list "https://space.bilibili.com/23947287?spm_id_from=..." --json

字段：id / title / url / published_at(ISO8601) / published_date
      duration_sec / duration_text / views / cover / paid / badges""",

    "search": """bcli search <关键词> [-n 20] [--sort X] [--users] [--desc] [--json]

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
    bcli search "露营" -n 3 --desc          # 带简介，适合直接看要不要点开""",

    "info": """bcli info <BV|链接> [--json]

用途：下载前探路 —— 这条视频要不要钱、我能不能播、有哪些清晰度。

输出：标题 / UP / 发布日期 / 时长 / 分P
      🏷 内容类型：free(免费) | vip(会员专享) | charging(充电专属) | paid(付费)
      🔑 账号权限：yes(可播放) | limited(仅低清晰度) | no(无权限) | unknown(需登录判断)
      通道探测：app / tv 各自最高清晰度与轨道数

例：bcli info BV1ocsd6jErz --json""",

    "dl": """bcli dl <BV|链接> [--audio] [--quality Q] [--codec C] [--subs] [--keep-parts] [-o DIR] [--json]

  --audio        只下音频 m4a（做笔记最快，体积约 1/10）
  --quality      最高清晰度：1080P / 720P / 480P / 4K，默认最高可用
  --codec        视频编码偏好 avc | hevc | av1（默认 avc，兼容性最好）
  --subs         同时保存字幕 srt
  --keep-parts   保留下载的临时分轨（排查用）
  -o             输出根目录，默认 ~/Documents/bilibili_content

通道：app 首选 → tv 降级 → 都失败则报错（不碰 web 端）
产物：<DIR>/<UP主>/<YYYY-MM-DD>_<标题>.mp4（或 .m4a）
      + 同名 .info.json（含 channel、权益快照 my_access）
断流：自动续传最多 3 次；最终失败会删除半成品，绝不留下“看起来正常”的文件

例：bcli dl BV1Vbnn6iEfG --audio
    bcli dl BV1Vbnn6iEfG --quality 720P --subs
    bcli dl BV1Vbnn6iEfG -o /tmp/test""",

    "doctor": """bcli doctor

检查：curl / ffmpeg / qrencode / uv 是否就绪
      网页 cookie 与 TV token 的登录态
      投稿接口连通性（用样例 UP 实测）
      gRPC 搜索接口连通性（用关键词「露营」实测 1 条）

报错会拆到“网络 / TLS / 超时 / 风控 / 密钥 / 权益 / 接口改版 / 频率 / 服务端”。""",
}

HELP_OVERVIEW = """bilibili_cli — B 站取用工具（无状态：只登录、只列、只看、只下）

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
        6 缺依赖 / 7 权益不足（大会员/充电）"""


def cmd_help(topic: str | None) -> int:
    if not topic:
        print(HELP_OVERVIEW)
        return 0
    key = topic.strip().lower()
    if key in HELP_DETAIL:
        print(HELP_DETAIL[key])
        return 0
    print(f"没有这个命令的帮助: {topic}\n", file=sys.stderr)
    print("可用：" + ", ".join(HELP_DETAIL), file=sys.stderr)
    return 2


def cmd_doctor() -> int:
    ok = True
    print("环境检查：")
    for name in ("curl", "ffmpeg", "qrencode", "uv"):
        found = shutil.which(name)
        print(f"  {'✓' if found else '·'} {name}")
    print(f"  {'✓' if shutil.which('ffmpeg') else '✗'} ffmpeg（合并音视频必需）")
    ok = ok and bool(shutil.which("ffmpeg"))

    c = load_creds()
    print("登录态：")
    if c.get("cookie"):
        who = _nav_name(cookie_header(c))
        print(f"  ✓ 网页 cookie{'（' + who + '）' if who else ''} · 保存于 "
              f"{datetime.fromtimestamp(c.get('saved_at', 0)).strftime('%Y-%m-%d') if c.get('saved_at') else '未知'}")
    else:
        print("  · 未登录网页（只能拿 480P）")
    if c.get("tv_access_token"):
        print("  ✓ TV access_token（TV 通道可用）")
    else:
        print("  · 未登录 TV（TV 通道会跳过）")

    try:
        v = user_videos("23947287", 3)
        print(f"环境接口：✓ 投稿接口连通（样例 {v['total']} 条）")
    except CliError as e:
        print("环境接口：✗")
        report(e)
        ok = False

    try:
        t0 = time.time()
        hits = grpc_search("露营", 1)
        dt = time.time() - t0
        if hits:
            print(f"搜索接口：✓ gRPC 搜索连通（{dt:.1f}s，样例「{hits[0]['title'][:24]}」）")
        else:
            print("搜索接口：✗ gRPC 返回空（接口可能改版）")
            ok = False
    except CliError as e:
        print("搜索接口：✗")
        report(e)
        ok = False
    return 0 if ok else 3



# ------------------------------------------------------------------ protobuf / gRPC
#
# App 端搜索是 gRPC（不是 REST）：bilibili.polymer.app.search.v1.Search/SearchAll
# 实测无需登录、无需 access_key、无频率限制（连打 10 次全通）。
# 这里只用标准库手写最小 protobuf，不引入 grpcio/protobuf 依赖。

GRPC_HOST = "https://grpc.biliapi.net"
SVC_SEARCH = "/bilibili.polymer.app.search.v1.Search/SearchAll"
SVC_SEARCH_TYPE = "/bilibili.polymer.app.search.v1.Search/SearchByType"
SVC_VIEW = "/bilibili.app.view.v1.View/View"
UA_GRPC = "grpc-java-okhttp/1.0"

# SearchAll.order：0 综合 / 1 最多播放 / 2 最新 / 3 最多弹幕 / 4 最多收藏
SORT_LABEL = {"totalrank": "综合", "play": "最多播放", "new": "最新",
              "dm": "最多弹幕", "stow": "最多收藏"}
SORT_APP = {"totalrank": 0, "default": 0, "play": 1, "click": 1, "new": 2,
            "pubdate": 2, "dm": 3, "danmaku": 3, "stow": 4, "favorite": 4}
SEARCH_TYPE_USER = 2      # SearchByType.type：2 = UP 主 / 10 = 视频


def _pv(n: int) -> bytes:
    """varint 编码"""
    n = int(n)
    out = bytearray()
    while True:
        b = n & 0x7F
        n >>= 7
        out.append(b | (0x80 if n else 0))
        if not n:
            return bytes(out)


def _ps(num: int, v: str) -> bytes:
    """写入 string 字段"""
    b = (v or "").encode()
    return _pv(num << 3 | 2) + _pv(len(b)) + b


def _pi(num: int, n: int) -> bytes:
    """写入 int 字段"""
    return _pv(num << 3) + _pv(int(n))


def _pm(num: int, body: bytes) -> bytes:
    """写入嵌套 message 字段"""
    return _pv(num << 3 | 2) + _pv(len(body)) + body


def _pread(buf: bytes) -> dict:
    """严格按 wire 格式读，返回 {字段号: [(wiretype, 值)]}。截断即抛错，绝不静默丢数据。"""
    out: dict = {}
    i, n = 0, len(buf)

    def rv(pos):
        r = shift = 0
        while True:
            c = buf[pos]
            pos += 1
            r |= (c & 0x7F) << shift
            if not c & 0x80:
                return r, pos
            shift += 7
            if shift > 70:
                raise ValueError("varint 过长")

    while i < n:
        key, i = rv(i)
        f, wt = key >> 3, key & 7
        if wt == 0:
            v, i = rv(i)
        elif wt == 2:
            ln, i = rv(i)
            if i + ln > n:
                raise ValueError(f"字段 {f} 越界")
            v = buf[i:i + ln]
            i += ln
        elif wt == 5:
            v = buf[i:i + 4]
            i += 4
        elif wt == 1:
            v = buf[i:i + 8]
            i += 8
        else:
            raise ValueError(f"未知 wiretype {wt}")
        out.setdefault(f, []).append((wt, v))
    return out


def _ptext(fields: dict, num: int, default: str = "") -> str:
    lst = fields.get(num)
    if not lst:
        return default
    wt, v = lst[0]
    return v.decode("utf-8", "replace") if wt == 2 else default


def _pint(fields: dict, num: int, default: int = 0) -> int:
    lst = fields.get(num)
    if not lst:
        return default
    wt, v = lst[0]
    return v if wt == 0 else default


def _pmsg(fields: dict, num: int) -> dict:
    lst = fields.get(num)
    if not lst or lst[0][0] != 2:
        return {}
    return _pread(lst[0][1])


def _prep(fields: dict, num: int) -> list:
    return [v for wt, v in fields.get(num, []) if wt == 2]


def grpc_call(path: str, payload: bytes, timeout: int = 20) -> bytes:
    """发一个 gRPC unary 请求，返回去帧后的原始 message 字节。"""
    frame = b"\x00" + struct.pack(">I", len(payload)) + payload
    with tempfile.TemporaryDirectory() as td:
        req_path = os.path.join(td, "req.bin")
        out_path = os.path.join(td, "resp.bin")
        with open(req_path, "wb") as f:
            f.write(frame)
        p = subprocess.run(
            ["curl", "-sS", "--http2", "--max-time", str(timeout),
             "-X", "POST", "--data-binary", "@" + req_path,
             "-H", "Content-Type: application/grpc", "-H", "TE: trailers",
             "-H", f"User-Agent: {UA_GRPC}",
             "-D", "-", "-o", out_path, GRPC_HOST + path],
            capture_output=True, text=True)
        if p.returncode != 0:
            cat, msg = CURL_EXIT.get(p.returncode, ("network", f"curl 退出码 {p.returncode}"))
            raise CliError(cat, f"gRPC 请求失败：{msg}", endpoint=path, channel="app")
        head = p.stdout
        body = b""
        if os.path.exists(out_path):
            with open(out_path, "rb") as f:
                body = f.read()

    status, message = "", ""
    for line in head.splitlines():
        low = line.lower()
        if low.startswith("grpc-status:"):
            status = line.split(":", 1)[1].strip()
        elif low.startswith("grpc-message:"):
            message = urllib.parse.unquote(line.split(":", 1)[1].strip())
    if status and status != "0":
        code = int(status)
        cat = {3: "interface", 8: "rate_limit", 14: "server",
               12: "interface", 13: "server"}.get(code, "unknown")
        raise CliError(cat, f"gRPC {code}：{message or '无说明'}", endpoint=path,
                       code=code, channel="app")

    # 去掉 5 字节帧头（可能有多帧，取第一帧的数据）
    if len(body) < 5:
        raise CliError("empty", "gRPC 返回空响应", endpoint=path, channel="app")
    flag, ln = body[0], struct.unpack(">I", body[1:5])[0]
    data = body[5:5 + ln]
    if flag:  # 压缩帧：正常不会遇到
        raise CliError("unknown", "gRPC 返回了压缩帧", endpoint=path, channel="app")
    return data


def _strip_em(t: str) -> str:
    return t.replace('<em class="keyword">', "").replace("</em>", "").strip()


def _dur_seconds(text: str) -> int:
    """把 app 的时长文本转秒：15:49 / 1:02:33 / 共25集 742:44 → 取最后一个时间部分。"""
    m = re.findall(r"(\d+):(\d+)(?::(\d+))?", text or "")
    if not m:
        return 0
    h, mnt, sec = m[-1]
    if sec:
        return int(h) * 3600 + int(mnt) * 60 + int(sec)
    return int(h) * 60 + int(mnt)


def _video_from_item(item: dict) -> dict | None:
    """Item → 视频卡"""
    card = _pmsg(item, 37)
    if not card:
        return None
    share = _pmsg(card, 6)
    video = _pmsg(share, 2)
    bvid = _ptext(video, 1)
    # Item.param 是 aid（string 类型！已与 web-interface/view 的 aid 逐条对齐）
    _aid = _ptext(item, 2)
    aid = int(_aid) if _aid.isdigit() else 0
    pub = _pint(card, 23)
    dur_text = _ptext(card, 12)
    return {
        "kind": "video",
        "bvid": bvid,
        "aid": aid,
        "title": _strip_em(_ptext(card, 1)),
        "author": _ptext(card, 10),
        "mid": _pint(card, 14),
        "play": _pint(card, 8),
        "danmaku": _pint(card, 9),
        "duration_text": dur_text,
        "duration_sec": _dur_seconds(dur_text),
        "pubdate": pub,
        "url": f"https://www.bilibili.com/video/{bvid}" if bvid else "",
        "cover": _ptext(card, 2),
    }


def _user_from_item(item: dict) -> dict | None:
    """Item → UP 主卡（mid 在 Item.param，不在卡片里）"""
    card = _pmsg(item, 36)
    if not card:
        return None
    ov = _pmsg(card, 8)
    mid = _ptext(item, 2)
    return {
        "kind": "user",
        "mid": int(mid) if mid.isdigit() else 0,
        "name": _ptext(card, 1),
        "fans": _pint(card, 4),
        "archives": _pint(card, 5),
        "level": _pint(card, 14),
        "sign": _ptext(card, 3),
        "official": _ptext(ov, 2),
        "url": f"https://space.bilibili.com/{mid}" if mid else "",
        "avatar": _ptext(card, 2),
    }


def grpc_view(bvid: str) -> dict:
    """详情卡：简介 + 完整统计（搜索卡里没有简介，服务端从不填 desc=11）。"""
    top = _pread(grpc_call(SVC_VIEW, _ps(2, bvid)))
    arc = _pmsg(top, 1)
    stat = _pmsg(arc, 23)
    return {
        "desc": _ptext(arc, 10),
        "duration_sec": _pint(arc, 16),
        "pubdate": _pint(arc, 8),
        "stats": {
            "play": _pint(stat, 2),
            "danmaku": _pint(stat, 3),
            "reply": _pint(stat, 4),
            "favorite": _pint(stat, 5),
            "coin": _pint(stat, 6),
            "share": _pint(stat, 7),
            "like": _pint(stat, 10),
        },
    }


def grpc_search(keyword: str, want: int, sort: str = "totalrank",
                users: bool = False, with_desc: bool = False) -> list:
    """App 端搜索：SearchAll（视频）/ SearchByType(type=2)（UP 主）。"""
    out: list = []
    nxt = ""
    per = 20
    pages = 0
    while len(out) < want and pages < 5:
        pages += 1
        pag = _pi(1, per) + (_ps(2, nxt) if nxt else b"")
        if users:
            req = _pi(1, SEARCH_TYPE_USER) + _ps(2, keyword) + _pm(7, pag)
            fields = _pread(grpc_call(SVC_SEARCH_TYPE, req))
            cards, pag_field = _prep(fields, 6), 7
        else:
            req = _ps(1, keyword) + _pi(2, SORT_APP.get(sort, 0)) + _pm(10, pag)
            fields = _pread(grpc_call(SVC_SEARCH, req))
            cards, pag_field = _prep(fields, 4), 11

        got = 0
        for raw in cards:
            item = _pread(raw)
            one = _user_from_item(item) if users else _video_from_item(item)
            if not one:
                continue
            out.append(one)
            got += 1
        nxt = _ptext(_pmsg(fields, pag_field), 1)
        if not nxt or got == 0:
            break
        time.sleep(0.3)

    out = out[:want]
    if with_desc and not users:
        for one in out:
            if not one.get("bvid"):
                continue
            try:
                d = grpc_view(one["bvid"])
                one["desc"] = d["desc"]
                one["stats"] = d["stats"]
                if not one.get("duration_sec"):
                    one["duration_sec"] = d["duration_sec"]
            except CliError:
                one["desc"] = ""
    return out


def _fmt_count(n: int) -> str:
    n = int(n or 0)
    if n >= 100000000:
        return f"{n / 100000000:.1f}亿"
    if n >= 10000:
        return f"{n / 10000:.1f}万"
    return str(n)


def cmd_search(keyword: str, n: int, sort: str, users: bool, desc: bool,
               as_json: bool) -> int:
    items = grpc_search(keyword, n, sort=sort, users=users, with_desc=desc)
    if not items:
        if as_json:
            print(json.dumps({"schema": SCHEMA, "type": "search", "keyword": keyword,
                              "count": 0, "items": []}, ensure_ascii=False, indent=2))
        else:
            print(f"没有结果：{keyword}", file=sys.stderr)
        return 4 if not items else 0

    if as_json:
        for one in items:
            if one.get("pubdate"):
                one["published_at"] = iso(one["pubdate"])
                one["published_date"] = datetime.fromtimestamp(
                    one["pubdate"]).strftime("%Y-%m-%d")
        print(json.dumps({"schema": SCHEMA, "type": "search", "keyword": keyword,
                          "sort": sort, "users": users, "count": len(items),
                          "items": items}, ensure_ascii=False, indent=2))
        return 0

    head = "UP 主" if users else "视频"
    suffix = "" if users or sort == "totalrank" else f"（排序：{SORT_LABEL.get(sort, sort)}）"
    print(f"「{keyword}」{head} {len(items)} 条{suffix}")
    for i, one in enumerate(items, 1):
        if users:
            print(f"\n{i:2d}. {one['name']}   <mid={one['mid']}>")
            line = [f"粉丝 {_fmt_count(one['fans'])}", f"投稿 {one['archives']}",
                    f"Lv{one['level']}"]
            print("    " + " · ".join(line))
            if one["official"]:
                print(f"    认证：{one['official']}")
            if one["sign"]:
                print(f"    签名：{one['sign'][:60]}")
        else:
            date = (datetime.fromtimestamp(one["pubdate"]).strftime("%Y-%m-%d")
                    if one["pubdate"] else "-")
            print(f"\n{i:2d}. {one['title']}   <{one['bvid']}>")
            dur = fmt_dur(one["duration_sec"]) if one.get("duration_sec") else one["duration_text"]
            if "集" in (one.get("duration_text") or ""):
                dur = one["duration_text"]
            print(f"    {one['author']} · {_fmt_count(one['play'])}播放 · {dur} · {date}")
            if desc and one.get("desc"):
                st = one.get("stats") or {}
                print(f"    简介：{one['desc'][:120].replace(chr(10), ' / ')}")
                print(f"    收藏 {_fmt_count(st.get('favorite', 0))} · "
                      f"点赞 {_fmt_count(st.get('like', 0))} · "
                      f"投币 {_fmt_count(st.get('coin', 0))}")
    return 0


# ------------------------------------------------------------------ CLI

def main() -> int:
    ap = argparse.ArgumentParser(
        prog="bilibili_cli",
        description="B 站取用工具：登录 / 列投稿 / 看详情 / 下载（无状态，不含订阅逻辑）")
    sub = ap.add_subparsers(dest="cmd")

    p = sub.add_parser("login", help="扫码登录（默认网页；--tv 为云视听小电视）")
    p.add_argument("--tv", action="store_true", help="走 TV 端登录，供 TV 通道使用")

    p = sub.add_parser("list", help="列出某 UP 主近 N 条投稿")
    p.add_argument("target")
    p.add_argument("-n", "--limit", type=int, default=20)
    p.add_argument("--json", action="store_true")
    p.add_argument("--rights", action="store_true",
                   help="逐条判定是否会员/充电专属（每条多一次请求，慢）")

    p = sub.add_parser("search", help="按关键词搜视频（--users 搜 UP 主）")
    p.add_argument("keyword")
    p.add_argument("-n", "--limit", type=int, default=20, help="条数，默认 20，上限 100")
    p.add_argument("--sort", default="totalrank",
                   choices=["totalrank", "play", "new", "dm", "stow"],
                   help="totalrank 综合(默认) / play 最多播放 / new 最新 / dm 弹幕 / stow 收藏")
    p.add_argument("--users", action="store_true", help="搜 UP 主而不是视频")
    p.add_argument("--desc", action="store_true",
                   help="补每条简介与收藏/点赞/投币（每条多一次请求，约 +130ms）")
    p.add_argument("--json", action="store_true")

    p = sub.add_parser("info", help="单视频详情：分P / 清晰度 / 各通道可用性")
    p.add_argument("target")
    p.add_argument("--json", action="store_true")

    p = sub.add_parser("dl", help="下载视频或音频（app 首选，tv 降级备用）")
    p.add_argument("target")
    p.add_argument("--audio", action="store_true", help="只下音频（m4a）")
    p.add_argument("--quality", help="最高清晰度，如 1080P / 4K / 720P（默认最高可用）")
    p.add_argument("--codec", default="avc", choices=list(CODECS), help="视频编码偏好")
    p.add_argument("-o", "--out", default=DEFAULT_OUT, help=f"输出根目录（默认 {DEFAULT_OUT}）")
    p.add_argument("--subs", action="store_true", help="同时保存字幕（srt）")
    p.add_argument("--keep-parts", action="store_true", help="保留下载的临时分轨")
    p.add_argument("--json", action="store_true")

    sub.add_parser("doctor", help="环境与登录态自检")

    p = sub.add_parser("help", help="查看帮助（bcli help [command]）")
    p.add_argument("topic", nargs="?", help="命令名，如 dl；留空看总览")

    args = ap.parse_args()
    if not args.cmd:
        ap.print_help()
        return 2

    try:
        if args.cmd == "login":
            return login_tv() if args.tv else login_web()
        if args.cmd == "search":
            n = max(1, min(args.limit, 100))
            return cmd_search(args.keyword, n, args.sort, args.users, args.desc, args.json)
        if args.cmd == "list":
            return cmd_list(args.target, args.limit, args.json, args.rights)
        if args.cmd == "info":
            return cmd_info(args.target, args.json)
        if args.cmd == "dl":
            return cmd_dl(args.target, audio_only=args.audio, quality=args.quality,
                          codec=args.codec, out_dir=args.out, keep_parts=args.keep_parts,
                          subs=args.subs, as_json=args.json)
        if args.cmd == "doctor":
            return cmd_doctor()
        if args.cmd == "help":
            return cmd_help(args.topic)
    except KeyboardInterrupt:
        return 130
    except CliError as e:
        report(e)
        return {"rights": 7, "auth": 5, "missing": 6}.get(e.category, 3)
    except Exception as e:
        die(f"未预期错误: {type(e).__name__}: {e}", 3)
    return 2


if __name__ == "__main__":
    sys.exit(main())
