//! play 模块移植桩：权益判定 + 通道可播性 + 流挑选。
//! 实现前必读 rust/tasks/00-shared-rules.md 与 rust/tasks/play.md。
#![forbid(unsafe_code)]

/// Port of `entitlement_of` (`bilibili_cli.py:502`). view 数据 → 免费/会员/充电/付费判定。
pub fn entitlement_of(d: serde_json::Value) -> Result<serde_json::Value, String> {
    let _ = &d;
    unimplemented!("port `entitlement_of` from bilibili_cli.py")
}

/// Port of `access_of` (`bilibili_cli.py:524`).
///
/// `ent` 在源码中默认 `None`。
pub fn access_of(
    probes: Vec<serde_json::Value>,
    ent: Option<serde_json::Value>,
) -> Result<serde_json::Value, String> {
    let _ = (&probes, &ent);
    unimplemented!("port `access_of` from bilibili_cli.py")
}

/// Port of `pick_streams` (`bilibili_cli.py:590`).
///
/// `codec` 在源码中默认 `"avc"`。无视频流时源码抛 `CliError("rights", ...)`。
pub fn pick_streams(
    dash: serde_json::Value,
    want_qn: i64,
    codec: String,
) -> Result<serde_json::Value, String> {
    let _ = (&dash, &want_qn, &codec);
    unimplemented!("port `pick_streams` from bilibili_cli.py")
}

/// Port of `_url_of` (`bilibili_cli.py:605`). 流 dict → 主 URL（baseUrl/base_url）。
pub fn _url_of(s: serde_json::Value) -> Result<String, String> {
    let _ = &s;
    unimplemented!("port `_url_of` from bilibili_cli.py")
}

/// Port of `_backup_of` (`bilibili_cli.py:609`). 流 dict → 备用 URL 列表。
pub fn _backup_of(s: serde_json::Value) -> Result<Vec<serde_json::Value>, String> {
    let _ = &s;
    unimplemented!("port `_backup_of` from bilibili_cli.py")
}
