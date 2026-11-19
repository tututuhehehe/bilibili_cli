//! 生成于 lift：所有命令由 Rust 应答，无回落、无 kill switch。

use std::process::ExitCode;

use clap::ArgMatches;

mod parse;
mod play;
mod proto;
mod sign;

pub fn dispatch(command: &str, matches: &ArgMatches) -> Option<ExitCode> {
    match command {
        "search" | "doctor" => Some(proto::run(command, matches)),
        "list" | "login" => Some(sign::run(command, matches)),
        "info" | "dl" => Some(play::run(command, matches)),
        _ => None,
    }
}
