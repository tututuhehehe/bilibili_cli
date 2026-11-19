//! `bilibili_cli.CliError` 的 bridge 侧异常类。
//!
//! 原版在 playurl 无视频流时抛 `CliError("rights", ...)`（bilibili_cli.py:592）。
//! 生成模板的 `error()` 只认 Python 内建异常名，会把 `Err("CliError: ...")`
//! 降级成 `ValueError`；而 diff/gate 按「异常类型名: 消息」对比录制基线，
//! `CliError` 这个类型名是对比的一部分（实测该用例必红），故用
//! `create_exception!` 定义同名异常类，供 `lib.rs` 的 `error()` 分发。
#![forbid(unsafe_code)]

pyo3::create_exception!(bilibili_cli, CliError, pyo3::exceptions::PyException);
