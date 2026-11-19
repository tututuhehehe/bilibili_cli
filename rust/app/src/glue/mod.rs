//! 共享胶水层：错误模型、curl 执行器、gRPC 编排。
//! 逐字对齐 bilibili_cli.py 的 CliError / report / curl / grpc_call。

pub mod creds;
pub mod curl;
pub mod download;
pub mod err;
pub mod grpc;
pub mod http;
pub mod login;
pub mod play;
