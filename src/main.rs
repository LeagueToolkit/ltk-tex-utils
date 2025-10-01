use clap::{
    ColorChoice, CommandFactory, FromArgMatches, Parser, Subcommand,
    builder::{Styles, styling::AnsiColor},
};
use league_toolkit::texture::tex::MipmapFilter;
use tracing::{Level, level_filters::LevelFilter};
use tracing_subscriber::prelude::*;

mod commands;
mod utils;

use commands::{
    DecodeCommandOptions, EncodeCommandOptions, InfoCommandOptions, decode, encode, info,
};
use utils::{parse_format, parse_mipmap_filter};

use crate::utils::ValidFormat;

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand, Debug)]
pub enum Commands {
    Info {
        /// Texture to get info from
        #[arg(short, long)]
        input: String,
    },
    Encode {
        /// Texture to encode
        #[arg(short, long)]
        input: String,

        /// Output file path
        #[arg(short, long)]
        output: String,

        /// Texture format to encode to (e.g., BC1, BC3, BGRA8)
        #[arg(short, long, value_parser = parse_format)]
        format: ValidFormat,

        /// Whether to generate mipmaps
        #[arg(short = 'm', long, default_value = "true")]
        generate_mipmaps: bool,

        /// Filter type to use for mipmap generation
        #[arg(long, default_value = "triangle", value_parser = parse_mipmap_filter)]
        mipmap_filter: MipmapFilter,
    },
    Decode {
        /// Texture to decode
        #[arg(short, long)]
        input: String,

        /// Output file path
        #[arg(short, long)]
        output: String,
    },
}

fn main() {
    initialize_tracing().unwrap();

    let styles = Styles::styled()
        .header(AnsiColor::Yellow.on_default().bold())
        .usage(AnsiColor::Green.on_default().bold())
        .literal(AnsiColor::Cyan.on_default())
        .placeholder(AnsiColor::Blue.on_default());

    let matches = Args::command()
        .styles(styles)
        .color(ColorChoice::Auto)
        .get_matches();

    let args = Args::from_arg_matches(&matches).expect("failed to parse arguments");

    // TODO: Handle commands
    match args.command {
        Commands::Info { input } => info(InfoCommandOptions { input }),
        Commands::Encode {
            input,
            output,
            format,
            generate_mipmaps,
            mipmap_filter,
        } => encode(EncodeCommandOptions {
            input,
            output,
            format,
            generate_mipmaps,
            mipmap_filter,
        }),
        Commands::Decode { input, output } => decode(DecodeCommandOptions { input, output }),
    }
}

fn initialize_tracing() -> eyre::Result<()> {
    let common_format = tracing_subscriber::fmt::format()
        .with_ansi(true)
        .with_level(true)
        .with_source_location(false)
        .with_line_number(false)
        .with_target(false)
        .with_timer(tracing_subscriber::fmt::time::time());

    // stdout: INFO/DEBUG/TRACE
    let stdout_layer = tracing_subscriber::fmt::layer()
        .with_writer(std::io::stdout)
        .event_format(common_format.clone())
        .with_filter(tracing_subscriber::filter::filter_fn(|metadata| {
            let level = *metadata.level();
            level == Level::INFO || level == Level::DEBUG || level == Level::TRACE
        }));

    // stderr: WARN/ERROR
    let stderr_layer = tracing_subscriber::fmt::layer()
        .with_writer(std::io::stderr)
        .event_format(common_format)
        .with_filter(tracing_subscriber::filter::filter_fn(|metadata| {
            let level = *metadata.level();
            level == Level::WARN || level == Level::ERROR
        }));

    tracing_subscriber::registry()
        .with(stdout_layer)
        .with(stderr_layer)
        .with(LevelFilter::TRACE)
        .init();
    Ok(())
}
