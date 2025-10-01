use clap::{
    ColorChoice, CommandFactory, FromArgMatches, Parser, Subcommand,
    builder::{Styles, styling::AnsiColor},
};
use league_toolkit::texture::tex::MipmapFilter;
use std::path::Path;
use tracing::info;
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
        /// The output directory will be created if it doesn't exist
        /// The output format will be determined by the file extension
        #[arg(short, long)]
        output: String,
    },
}

fn main() -> eyre::Result<()> {
    initialize_tracing().unwrap();

    // Drag-and-drop auto mode (Windows): if invoked with a single file path argument,
    // auto-route to decode/encode based on extension and derive the output by changing the extension.
    if let Some(result) = try_handle_auto_mode()? {
        result?;
        return Ok(());
    }

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
        })?,
        Commands::Decode { input, output } => decode(DecodeCommandOptions { input, output })?,
    }

    Ok(())
}

/// Attempts to handle a single positional argument invocation (drag-and-drop style).
/// Returns Ok(Some(result)) if handled, Ok(None) to continue with normal clap parsing.
fn try_handle_auto_mode() -> eyre::Result<Option<eyre::Result<()>>> {
    let mut args = std::env::args_os();
    let _exe = args.next();
    let first = match args.next() {
        Some(a) => a,
        None => return Ok(None),
    };
    // Ensure exactly one argument
    if args.next().is_some() {
        return Ok(None);
    }

    // If the single argument looks like a flag (e.g. --help, -h, /?),
    // defer to the normal CLI parser so help/version work as expected.
    let first_str_lossy = first.to_string_lossy();
    if first_str_lossy.starts_with('-') || first_str_lossy.starts_with('/') {
        return Ok(None);
    }

    let input_os = first;
    let input_path = Path::new(&input_os);

    // Only trigger auto-mode if the argument resolves to an existing path.
    // This prevents accidental activation on random single tokens.
    if !input_path.exists() {
        return Ok(None);
    }

    let input_str = input_path.to_string_lossy().to_string();

    let ext = input_path
        .extension()
        .and_then(|s| s.to_str())
        .map(|s| s.to_lowercase());

    match ext.as_deref() {
        Some("dds") => Ok(Some(Err(eyre::eyre!(
            ".dds files are not supported for decoding"
        )))),
        Some("tex") => {
            let mut out_path = input_path.to_path_buf();
            out_path.set_extension("png");
            let output = out_path.to_string_lossy().to_string();
            info!(
                input = %input_str,
                output = %output,
                "auto mode: decoding .tex to .png"
            );
            let res = crate::commands::decode(crate::commands::DecodeCommandOptions {
                input: input_str,
                output,
            });
            Ok(Some(res))
        }
        _ => {
            let mut out_path = input_path.to_path_buf();
            out_path.set_extension("tex");
            let output = out_path.to_string_lossy().to_string();

            let format = ValidFormat::Bc3;
            let generate_mipmaps = true;
            let mipmap_filter = MipmapFilter::Lanczos3;

            info!(
                input = %input_str,
                output = %output,
                format = ?format,
                generate_mipmaps = generate_mipmaps,
                mipmap_filter = ?mipmap_filter,
                "auto mode: encoding image to .tex"
            );

            let res = crate::commands::encode(crate::commands::EncodeCommandOptions {
                input: input_str,
                output,
                format,
                generate_mipmaps,
                mipmap_filter,
            });
            Ok(Some(res))
        }
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
