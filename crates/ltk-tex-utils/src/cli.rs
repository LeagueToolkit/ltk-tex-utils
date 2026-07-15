use clap::{
    ColorChoice, CommandFactory, FromArgMatches, Parser, Subcommand, ValueEnum,
    builder::{Styles, styling::AnsiColor},
};

use crate::commands::{self, DecodeArgs, EncodeArgs, InfoArgs};
use crate::handler;
use crate::shell;

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
pub struct Args {
    /// When to keep the console window open before exiting
    /// (useful when invoked from Explorer)
    #[arg(long, global = true, value_enum, default_value_t = PauseMode::Never)]
    pub pause: PauseMode,

    #[command(subcommand)]
    pub command: Commands,
}

#[derive(ValueEnum, Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PauseMode {
    /// Exit immediately
    #[default]
    Never,
    /// Wait for Enter if an error occurred
    OnError,
    /// Always wait for Enter
    Always,
}

#[derive(Subcommand, Debug)]
pub enum Commands {
    Info(InfoArgs),
    Encode(EncodeArgs),
    Decode(DecodeArgs),
    /// Manage Windows Explorer context-menu integration
    Shell {
        #[command(subcommand)]
        action: shell::ShellAction,
    },
    /// Install/uninstall the .tex thumbnail & preview handler (elevates via UAC when needed)
    Handler {
        #[command(subcommand)]
        action: handler::HandlerAction,
    },
}

/// Parse the command line with styled help/usage output.
pub fn parse() -> Args {
    let styles = Styles::styled()
        .header(AnsiColor::Yellow.on_default().bold())
        .usage(AnsiColor::Green.on_default().bold())
        .literal(AnsiColor::Cyan.on_default())
        .placeholder(AnsiColor::Blue.on_default());

    let matches = Args::command()
        .styles(styles)
        .color(ColorChoice::Auto)
        .get_matches();

    Args::from_arg_matches(&matches).expect("failed to parse arguments")
}

pub fn run(command: Commands) -> eyre::Result<()> {
    match command {
        Commands::Info(args) => {
            commands::info::run(args);
            Ok(())
        }
        Commands::Encode(args) => commands::encode::run(args),
        Commands::Decode(args) => commands::decode::run(args),
        Commands::Shell { action } => shell::run(&action),
        Commands::Handler { action } => handler::run(&action),
    }
}

/// Apply the `--pause` behavior, printing the error before pausing so it stays
/// visible in a console window spawned by Explorer.
pub fn finish(result: eyre::Result<()>, pause: PauseMode) -> eyre::Result<()> {
    match result {
        Ok(()) => {
            if pause == PauseMode::Always {
                pause_prompt();
            }
            Ok(())
        }
        Err(err) => {
            if pause != PauseMode::Never {
                eprintln!("error: {err:?}");
                pause_prompt();
                std::process::exit(1);
            }
            Err(err)
        }
    }
}

fn pause_prompt() {
    use std::io::Write;
    print!("\nPress Enter to close...");
    let _ = std::io::stdout().flush();
    let _ = std::io::stdin().read_line(&mut String::new());
}
