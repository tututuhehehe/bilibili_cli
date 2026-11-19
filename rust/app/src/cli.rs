use std::ffi::OsString;

use clap::{Arg, ArgAction, Command, value_parser};

/// The command `units::dispatch` gets when argv names no subcommand.
pub const ROOT: &str = "bilibili_cli";

fn rest() -> Arg {
    Arg::new("args")
        .num_args(0..)
        .trailing_var_arg(true)
        .allow_hyphen_values(true)
        .value_parser(value_parser!(OsString))
}

/// Routes argv to a command. Anything this parser rejects runs the original, so it never
/// has to match every way the original accepts its arguments.
pub fn command() -> Command {
    Command::new(ROOT)
        .disable_help_flag(true)
        .disable_version_flag(true)
        .disable_help_subcommand(true)
        .args_conflicts_with_subcommands(true)
        .arg(rest())
        .subcommand(Command::new("help").disable_help_flag(true).arg(rest()))
        .subcommand(
            Command::new("search")
                .disable_help_flag(true)
                .arg(
                    Arg::new("keyword")
                        .required(true)
                        .num_args(1)
                        .value_parser(value_parser!(OsString)),
                )
                .arg(
                    Arg::new("limit")
                        .short('n')
                        .long("limit")
                        .default_value("20")
                        .value_parser(value_parser!(i64)),
                )
                .arg(
                    Arg::new("sort")
                        .long("sort")
                        .default_value("totalrank")
                        .value_parser(["totalrank", "play", "new", "dm", "stow"]),
                )
                .arg(Arg::new("users").long("users").action(ArgAction::SetTrue))
                .arg(Arg::new("desc").long("desc").action(ArgAction::SetTrue))
                .arg(Arg::new("json").long("json").action(ArgAction::SetTrue)),
        )
        .subcommand(
            Command::new("list")
                .disable_help_flag(true)
                .arg(
                    Arg::new("target")
                        .required(true)
                        .num_args(1)
                        .value_parser(value_parser!(OsString)),
                )
                .arg(
                    Arg::new("limit")
                        .short('n')
                        .long("limit")
                        .default_value("20")
                        .value_parser(value_parser!(i64)),
                )
                .arg(Arg::new("json").long("json").action(ArgAction::SetTrue))
                .arg(Arg::new("rights").long("rights").action(ArgAction::SetTrue)),
        )
        .subcommand(
            Command::new("info")
                .disable_help_flag(true)
                .arg(
                    Arg::new("target")
                        .required(true)
                        .num_args(1)
                        .value_parser(value_parser!(OsString)),
                )
                .arg(Arg::new("json").long("json").action(ArgAction::SetTrue)),
        )
        .subcommand(
            Command::new("doctor")
                .disable_help_flag(true),
        )
        .subcommand(
            Command::new("dl")
                .disable_help_flag(true)
                .arg(
                    Arg::new("target")
                        .required(true)
                        .num_args(1)
                        .value_parser(value_parser!(OsString)),
                )
                .arg(Arg::new("audio").long("audio").action(ArgAction::SetTrue))
                .arg(
                    Arg::new("quality")
                        .long("quality")
                        .num_args(1)
                        .value_parser(value_parser!(OsString)),
                )
                .arg(
                    Arg::new("codec")
                        .long("codec")
                        .default_value("avc")
                        .value_parser(["avc", "hevc", "av1"]),
                )
                .arg(
                    Arg::new("out")
                        .short('o')
                        .long("out")
                        .num_args(1)
                        .value_parser(value_parser!(OsString)),
                )
                .arg(Arg::new("subs").long("subs").action(ArgAction::SetTrue))
                .arg(
                    Arg::new("keep_parts")
                        .long("keep-parts")
                        .action(ArgAction::SetTrue),
                )
                .arg(Arg::new("json").long("json").action(ArgAction::SetTrue)),
        )
}
