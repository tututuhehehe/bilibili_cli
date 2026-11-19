#!/usr/bin/env python3
"""proto 模块的用例与参照数据生成（离线，一次性；重跑幂等——先截断再追加）。

- 从 rust/fixtures/searchall_1.resp.bin 的真实 Item 生成 `_video_from_item` 用例，
  并补边界（无 37 字段的非视频卡 / 巨大 aid / No 类字符 aid）。
- `_user_from_item`：仓库内没有 SearchByType 抓帧，用原版 `_pv/_ps/_pi/_pm`
  编码器按 README 字段表构造真实 wire 形状的 Item（param=2、卡=36、认证=8.2），
  值取自 fixtures 里的真实 UP 数据；None 路径用真实 SearchAll Item 覆盖。
- `_pread/_ptext/_pint/_pmsg/_prep/_strip_em/_dur_seconds` 的陷阱用例：
  wire 截断/越界/未知 wiretype/超长 varint/重复字段/u64 域、utf-8 replace、
  全角与阿拉伯-印度数字、C0 分隔符 strip、多匹配取末位。
- tests/expected_*.json：原版对两帧 resp.bin 的一次性运行参照（driver enc 形式），
  cargo 测试逐字段比对。

用法：cd rust/proto && python3 gen_cases.py
"""

import json
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]   # bilibili_cli/
CRATE = Path(__file__).resolve().parent      # rust/proto/
sys.path.insert(0, str(ROOT))

import bilibili_cli as bc  # noqa: E402  原版：录制与参照的唯一真值来源

BIG = 2 ** 53


def enc(v):  # 与 rust-anything driver 的 enc 完全同构
    if v is None or isinstance(v, (bool, str)):
        return v
    if isinstance(v, int):
        return v if -BIG <= v <= BIG else {"$int": str(v)}
    if isinstance(v, list):
        return [enc(x) for x in v]
    if isinstance(v, tuple):
        return {"$tuple": [enc(x) for x in v]}
    if isinstance(v, (bytes, bytearray)):
        return {"$bytes": list(v)}
    if isinstance(v, dict):
        if all(isinstance(k, str) for k in v):
            return {k: enc(x) for k, x in v.items()}
        return {"$dict": [[enc(k), enc(x)] for k, x in v.items()]}
    raise TypeError(repr(v))


def case(fn, *args):
    return {"fn": fn, "args": [enc(a) for a in args]}


searchall = (ROOT / "rust/fixtures/searchall_1.resp.bin").read_bytes()
view = (ROOT / "rust/fixtures/view_1.resp.bin").read_bytes()
items = bc._prep(bc._pread(searchall), 4)

new_inputs = []

# ── _video_from_item：真实 Item（时长文本 / em 高亮 / emoji 全覆盖）───────────
for idx in [1, 2, 3, 5, 10, 15, 18]:
    new_inputs.append(case("_video_from_item", bc._pread(items[idx])))

# 合成边界（真实 wire 形状）：无 37 卡的非视频 Item → None
new_inputs.append(case("_video_from_item", bc._pread(
    bc._ps(1, "bilibili://clip") + bc._ps(2, "9") + bc._ps(3, "tint"))))
# aid 超过 i64：Item.param.isdigit() 真，int() 出 2^63（bridge $int 精确承载）
new_inputs.append(case("_video_from_item", bc._pread(
    bc._ps(2, "9223372036854775808") + bc._pm(37, bc._ps(1, "大aid")))))
# No 类字符（上标 ²）：isdigit() 真、int() 抛 ValueError——原版同抛，对齐错误类
new_inputs.append(case("_video_from_item", bc._pread(
    bc._ps(2, "²") + bc._pm(37, bc._ps(1, "坏aid")))))

# ── _user_from_item：SearchByType 形状（仓库无抓帧，用原版编码器构造）──────
AVATAR = "https://i0.hdslb.com/bfs/face/76e6fc5a0ec3784fa276"
FULL_CARD = (bc._ps(1, "神秘的小白QAQ") + bc._ps(2, AVATAR)
             + bc._ps(3, "户外露营、野营车探险，周五晚更新")
             + bc._pi(4, 1234567) + bc._pi(5, 234) + bc._pi(14, 6)
             + bc._pm(8, bc._ps(2, "bilibili 优质UP主认证")))
PLAIN_CARD = (bc._ps(1, "尼玛很快乐") + bc._ps(2, AVATAR)
              + bc._ps(3, "4K超清露营合集") + bc._pi(4, 88) + bc._pi(5, 1202)
              + bc._pi(14, 5))
MINI_CARD = bc._ps(1, "吃铁丝儿")

new_inputs.append(case("_user_from_item", bc._pread(   # 全字段：fans/sign/认证
    bc._ps(2, "235733713") + bc._pm(36, FULL_CARD))))
new_inputs.append(case("_user_from_item", bc._pread(   # 无认证消息 → official ""
    bc._ps(2, "3546726365399456") + bc._pm(36, PLAIN_CARD))))
new_inputs.append(case("_user_from_item", bc._pread(   # 最小卡 → fans 等 0
    bc._ps(2, "19035216") + bc._pm(36, MINI_CARD))))
new_inputs.append(case("_user_from_item", bc._pread(   # 非数字 param → mid 0，url 原文
    bc._ps(2, "BV1xx411c7mD") + bc._pm(36, FULL_CARD))))
new_inputs.append(case("_user_from_item", bc._pread(   # 空 param → mid 0，url ""
    bc._ps(2, "") + bc._pm(36, PLAIN_CARD))))
new_inputs.append(case("_user_from_item", bc._pread(   # No 字符 → ValueError
    bc._ps(2, "²") + bc._pm(36, MINI_CARD))))
new_inputs.append(case("_user_from_item", bc._pread(items[0])))  # 真实 Item 无 36 → None

# ── _pread：wire 陷阱 ────────────────────────────────────────────────────────
new_inputs.append(case("_pread", bytes([0x0D, 0xAA])))          # wt5 截断不报错
new_inputs.append(case("_pread", bytes([0x09, 1, 2, 3])))       # wt1 截断不报错
new_inputs.append(case("_pread", bytes([0x0A, 0x05, 0x61])))   # 越界 → ValueError
new_inputs.append(case("_pread", bytes([0x1B])))                # 未知 wiretype 3
new_inputs.append(case("_pread", bytes([255] * 10 + [129])))    # varint 过长
new_inputs.append(case("_pread", bytes([8, 1, 8, 2])))          # 重复字段合并
new_inputs.append(case("_pread", bytes([8] + [255] * 9 + [1])))  # varint = 2^64-1
new_inputs.append(case("_pread", bytes([0xF0] + [255] * 9 + [1, 0x2A])))  # 键 > u64

# ── _ptext / _pint / _pmsg / _prep：首条目、wt 不符、值透传 ──────────────────
new_inputs.append(case("_ptext", {2: [(2, b"abc\xff\xed\xa0\x80\xe0\xa0def")]}, 2, ""))
new_inputs.append(case("_ptext", {7: [(0, 5)]}, 7, "缺省"))
new_inputs.append(case("_ptext", {2: [(2, b"\xe4\xb8\xad"), (2, b"\xe5\x9b\xbd")]}, 2, ""))
new_inputs.append(case("_pint", {5: [(2, b"\x01")]}, 5, 42))
new_inputs.append(case("_pint", {1: [(0, 42), (0, 99)]}, 1, 0))
new_inputs.append(case("_pmsg", {2: [(0, 5)]}, 2))
new_inputs.append(case("_pmsg", {3: [(2, b"\x08\x01")]}, 7))
new_inputs.append(case("_prep", {4: [(2, b"a"), (0, 7), (2, b"b")]}, 4))
new_inputs.append(case("_prep", {4: [(2, 42)]}, 4))              # int 值原样透传
new_inputs.append(case("_prep", {4: [(0, 9)]}, 7))              # 键缺失 → []

# ── _strip_em：全角空格 strip、多重 em、C0 分隔符 ────────────────────────────
new_inputs.append(case("_strip_em", "　<em class=\"keyword\">露营</em>　"))
new_inputs.append(case("_strip_em", "  多重<em class=\"keyword\">a</em><em class=\"keyword\">b</em>  "))
new_inputs.append(case("_strip_em", "\x1c边缘\x1f"))

# ── _dur_seconds：全角/阿数位、多匹配取末位、双冒号、共N集 ─────────────────────
for t in ["共25集 742:44", "１:２", "٣:٤", "10:20:30:40", "68::44",
          "1:2 3:4:5", "🦀68:44🦀", ":68:44"]:
    new_inputs.append(case("_dur_seconds", t))

# ── inputs.jsonl：追加在基线之后（幂等：先剔掉旧追加段再补齐） ─────────────────────────────────────────────
inputs_path = CRATE / "inputs.jsonl"
old_lines = [l for l in inputs_path.read_text().splitlines() if l.strip()]
new_lines = [json.dumps(c, ensure_ascii=False) for c in new_inputs]
baseline_lines = [l for l in old_lines if l not in set(new_lines)]
fresh = baseline_lines + new_lines
inputs_path.write_text("".join(l + "\n" for l in fresh))
print(f"inputs.jsonl: {len(fresh)} 条（基线 {len(baseline_lines)} + 新增 {len(new_inputs)}）")

# ── 参照 JSON：原版对两帧的真实输出（cargo 测试逐字段比对）───────────────────
top = bc._pread(searchall)
expected_search = {
    "pread": enc(top),
    "videos": [enc(bc._video_from_item(bc._pread(raw))) for raw in items],
    "users": [enc(bc._user_from_item(bc._pread(raw))) for raw in items],
}
(CRATE / "tests/expected_searchall_1.json").write_text(
    json.dumps(expected_search, ensure_ascii=False), encoding="utf-8")

vtop = bc._pread(view)
varc = bc._pmsg(vtop, 1)
vstat = bc._pmsg(varc, 23)
expected_view = {
    "pread": enc(vtop),
    "walk": enc({                     # grpc_view 的解析走法（arc/23/…）
        "desc": bc._ptext(varc, 10),
        "duration_sec": bc._pint(varc, 16),
        "pubdate": bc._pint(varc, 8),
        "stats": {
            "play": bc._pint(vstat, 2), "danmaku": bc._pint(vstat, 3),
            "reply": bc._pint(vstat, 4), "favorite": bc._pint(vstat, 5),
            "coin": bc._pint(vstat, 6), "share": bc._pint(vstat, 7),
            "like": bc._pint(vstat, 10),
        },
    }),
}
(CRATE / "tests/expected_view_1.json").write_text(
    json.dumps(expected_view, ensure_ascii=False), encoding="utf-8")
print("tests/expected_*.json 已生成")
for name, obj in [("expected_searchall_1.json", expected_search),
                  ("expected_view_1.json", expected_view)]:
    size = (CRATE / "tests" / name).stat().st_size
    print(f"  {name}: {size/1024:.0f} KB")
