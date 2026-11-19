// CliError/Failure 为本 CLI 错误载体：一次错误即打印退出，无跨调用传递的装箱收益（lift 决策）。
#![allow(clippy::result_large_err)]

mod cli;
mod front_texts;
mod frontspec;
mod glue;
mod units;

use std::ffi::OsString;
use std::process::ExitCode;

fn main() -> ExitCode {
    // 识别标记：rust-anything app replay 靠它认出本构建。
    // 纯化后已无回落路径，这里只开一下日志文件，不再写任何内容。
    if let Ok(p) = std::env::var("RUST_ANYTHING_FALLBACK_LOG") {
        let _ = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(p);
    }
    let argv: Vec<OsString> = std::env::args_os().collect();
    match frontspec::classify(&argv) {
        frontspec::Handled::Exit {
            code,
            stdout,
            stderr,
        } => {
            if !stdout.is_empty() {
                print!("{stdout}");
            }
            if !stderr.is_empty() {
                eprint!("{stderr}");
            }
            use std::io::Write;
            let _ = std::io::stdout().flush();
            ExitCode::from(code)
        }
        frontspec::Handled::Pass => match cli::command().try_get_matches_from(&argv) {
            Ok(matches) => {
                let (command, sub) = matches.subcommand().unwrap_or((cli::ROOT, &matches));
                if command == "help" {
                    let topic = sub
                        .get_many::<OsString>("args")
                        .map(|it| it.cloned().collect::<Vec<_>>())
                        .and_then(|v| v.into_iter().next())
                        .map(|s| s.to_string_lossy().into_owned());
                    return ExitCode::from(frontspec::cmd_help(topic.as_deref()));
                }
                if let Some(code) = units::dispatch(command, sub) {
                    return code;
                }
                // frontspec 已接受、clap 已解析，但没有单元认领：不应发生（replay 守护）
                eprintln!("错误: 内部路由缺失，请上报");
                ExitCode::from(3)
            }
            Err(_) => {
                // frontspec 接受但 clap 拒绝：两解析器失配（不应发生；replay 全绿守护）
                eprintln!("错误: 内部解析不一致（frontspec/clap），请上报");
                ExitCode::from(3)
            }
        },
    }
}
