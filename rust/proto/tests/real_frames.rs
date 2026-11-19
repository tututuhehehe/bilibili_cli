//! 真实帧回归：`rust/fixtures/` 的两帧 gRPC 响应在 port 下与「原版一次性运行产生的
//! 参照 JSON」（`tests/expected_*.json`，由 `gen_cases.py` 用原版 Python 生成）逐字段
//! 一致。比对走 `pb::enc_normalize`：> 2^53 的整数两侧都会变成 `$int` 标记（与
//! rust-anything driver 的 `enc` 同阈值），其余值要求完全相等、`$dict`/`$bytes` 数组
//! 顺序敏感（即插入序敏感）。

use std::path::Path;

use serde_json::{Value, json};

use proto::pb::enc_normalize;

fn fixture(name: &str) -> Vec<u8> {
    let p = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../fixtures")
        .join(name);
    std::fs::read(&p).unwrap_or_else(|e| panic!("读 {p:?}: {e}"))
}

fn expected(name: &str) -> Value {
    let p = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join(name);
    let text = std::fs::read_to_string(&p).unwrap_or_else(|e| panic!("读 {p:?}: {e}"));
    serde_json::from_str(&text).expect("参照 JSON 解析")
}

fn item_values(frame: &[u8]) -> Vec<Value> {
    // SearchAll：fields.4 是 repeated Item；走 port 的公开函数链
    let top = proto::port::_pread(json!({ "$bytes": frame })).expect("_pread");
    proto::port::_prep(top, 4)
        .expect("_prep")
        .into_iter()
        .map(|raw| proto::port::_pread(raw).expect("item _pread"))
        .collect()
}

#[test]
fn searchall_frame_matches_reference() {
    let frame = fixture("searchall_1.resp.bin");
    let got = proto::port::_pread(json!({ "$bytes": frame })).expect("_pread");
    let want = expected("expected_searchall_1.json")["pread"].clone();
    assert_eq!(enc_normalize(&got), enc_normalize(&want));
}

#[test]
fn searchall_videos_match_reference() {
    let frame = fixture("searchall_1.resp.bin");
    let want = expected("expected_searchall_1.json")["videos"]
        .as_array()
        .cloned()
        .unwrap();
    let items = item_values(&frame);
    assert_eq!(items.len(), want.len(), "Item 数量");
    for (i, item) in items.iter().enumerate() {
        let got = proto::port::_video_from_item(item.clone()).expect("_video_from_item");
        assert_eq!(
            enc_normalize(&got),
            enc_normalize(&want[i]),
            "第 {i} 条视频卡逐字段不一致"
        );
    }
}

#[test]
fn searchall_items_are_not_user_cards() {
    let frame = fixture("searchall_1.resp.bin");
    let want = expected("expected_searchall_1.json")["users"]
        .as_array()
        .cloned()
        .unwrap();
    let items = item_values(&frame);
    for (i, item) in items.iter().enumerate() {
        let got = proto::port::_user_from_item(item.clone()).expect("_user_from_item");
        assert_eq!(enc_normalize(&got), enc_normalize(&want[i]), "第 {i} 条");
    }
}

#[test]
fn view_frame_matches_reference() {
    let frame = fixture("view_1.resp.bin");
    let got = proto::port::_pread(json!({ "$bytes": frame })).expect("_pread");
    let want = expected("expected_view_1.json")["pread"].clone();
    assert_eq!(enc_normalize(&got), enc_normalize(&want));
}

#[test]
fn view_walk_matches_reference() {
    // grpc_view 的解析走法：arc = top.1，stat = arc.23，desc/duration/pubdate/stats
    let frame = fixture("view_1.resp.bin");
    let top = proto::port::_pread(json!({ "$bytes": frame })).expect("_pread");
    let arc = proto::port::_pmsg(top, 1).expect("_pmsg arc");
    let stat = proto::port::_pmsg(arc.clone(), 23).expect("_pmsg stat");

    let walk = json!({
        "desc": proto::port::_ptext(arc.clone(), 10, String::new()).expect("desc"),
        "duration_sec": proto::port::_pint(arc.clone(), 16, 0).expect("duration"),
        "pubdate": proto::port::_pint(arc, 8, 0).expect("pubdate"),
        "stats": {
            "play": proto::port::_pint(stat.clone(), 2, 0).expect("play"),
            "danmaku": proto::port::_pint(stat.clone(), 3, 0).expect("danmaku"),
            "reply": proto::port::_pint(stat.clone(), 4, 0).expect("reply"),
            "favorite": proto::port::_pint(stat.clone(), 5, 0).expect("favorite"),
            "coin": proto::port::_pint(stat.clone(), 6, 0).expect("coin"),
            "share": proto::port::_pint(stat.clone(), 7, 0).expect("share"),
            "like": proto::port::_pint(stat, 10, 0).expect("like"),
        },
    });
    let want = expected("expected_view_1.json")["walk"].clone();
    assert_eq!(enc_normalize(&walk), enc_normalize(&want));
}
