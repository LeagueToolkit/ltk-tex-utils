use clap::{
    ColorChoice, CommandFactory, FromArgMatches, Parser, Subcommand,
    builder::{Styles, styling::AnsiColor},
};
use league_toolkit::texture::tex::MipmapFilter;
use std::ops::ControlFlow;
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
        /// Texture (image) to encode
        #[arg(
            short,
            long,
            value_name = "INPUT",
            required_unless_present = "input_pos",
            conflicts_with = "input_pos"
        )]
        input: Option<String>,

        /// Texture (image) to encode (positional alternative to -i/--input)
        #[arg(value_name = "INPUT", index = 1, required_unless_present = "input")]
        input_pos: Option<String>,

        /// Output file path
        /// Defaults to writing next to INPUT with a `.tex` extension.
        #[arg(short, long, value_name = "OUTPUT")]
        output: Option<String>,

        /// Texture format to encode to (e.g., BC1, BC3, BGRA8)
        #[arg(short, long, value_parser = parse_format, default_value = "bc3")]
        format: ValidFormat,

        /// Whether to generate mipmaps
        #[arg(short = 'm', long, default_value = "true")]
        generate_mipmaps: bool,

        /// Filter type to use for mipmap generation
        #[arg(long, default_value = "triangle", value_parser = parse_mipmap_filter)]
        mipmap_filter: MipmapFilter,
    },
    Decode {
        /// Texture (.tex) to decode
        #[arg(
            short,
            long,
            value_name = "INPUT",
            required_unless_present = "input_pos",
            conflicts_with = "input_pos"
        )]
        input: Option<String>,

        /// Texture (.tex) to decode (positional alternative to -i/--input)
        #[arg(value_name = "INPUT", index = 1, required_unless_present = "input")]
        input_pos: Option<String>,

        /// Output file path
        /// The output directory will be created if it doesn't exist
        /// The output format will be determined by the file extension
        /// Defaults to writing next to INPUT with a `.png` extension.
        #[arg(short, long, value_name = "OUTPUT")]
        output: Option<String>,

        /// Mipmap to decode
        #[arg(short, long, default_value = "0")]
        mipmap: u32,
    },
}

fn main() -> eyre::Result<()> {
    initialize_tracing().unwrap();

    // Drag-and-drop auto mode (Windows): if invoked with a single file path argument,
    // auto-route to decode/encode based on extension and derive the output by changing the extension.
    if let ControlFlow::Break(result) = try_handle_auto_mode() {
        return result;
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
            input_pos,
            output,
            format,
            generate_mipmaps,
            mipmap_filter,
        } => {
            let input = resolve_input(input, input_pos)?;
            let output = resolve_output(output, &input, "tex")?;
            encode(EncodeCommandOptions {
                input,
                output,
                format,
                generate_mipmaps,
                mipmap_filter,
            })?
        }
        Commands::Decode {
            input,
            input_pos,
            output,
            mipmap,
        } => {
            let input = resolve_input(input, input_pos)?;
            let output = resolve_output(output, &input, "png")?;
            decode(DecodeCommandOptions {
                input,
                output,
                mipmap,
            })?
        }
    }

    Ok(())
}

fn resolve_input(flag: Option<String>, positional: Option<String>) -> eyre::Result<String> {
    match (flag, positional) {
        (Some(v), None) | (None, Some(v)) => Ok(v),
        (None, None) => Err(eyre::eyre!(
            "missing input; pass -i/--input or provide INPUT positionally"
        )),
        (Some(_), Some(_)) => Err(eyre::eyre!(
            "provide input either via -i/--input or positionally, not both"
        )),
    }
}

fn resolve_output(output: Option<String>, input: &str, default_ext: &str) -> eyre::Result<String> {
    if let Some(o) = output {
        return Ok(o);
    }

    let input_path = Path::new(input);
    let parent = input_path.parent().unwrap_or_else(|| Path::new("."));

    let stem = input_path
        .file_stem()
        .and_then(|s| s.to_str())
        .ok_or_else(|| eyre::eyre!("failed to derive output path from INPUT"))?;

    let mut out_path = parent.join(stem);
    out_path.set_extension(default_ext);
    Ok(out_path.to_string_lossy().to_string())
}

/// Attempts to handle a single positional argument invocation (drag-and-drop style).
/// Returns `Break(result)` if handled, `Continue(())` to proceed with normal clap parsing.
fn try_handle_auto_mode() -> ControlFlow<eyre::Result<()>> {
    let mut args = std::env::args_os();
    let _exe = args.next();
    let first = match args.next() {
        Some(a) => a,
        None => return ControlFlow::Continue(()),
    };
    // Ensure exactly one argument
    if args.next().is_some() {
        return ControlFlow::Continue(());
    }

    // If the single argument looks like a flag (e.g. --help, -h, /?),
    // defer to the normal CLI parser so help/version work as expected.
    let first_str_lossy = first.to_string_lossy();
    if first_str_lossy.starts_with('-') || first_str_lossy.starts_with('/') {
        return ControlFlow::Continue(());
    }

    let input_os = first;
    let input_path = Path::new(&input_os);

    // Only trigger auto-mode if the argument resolves to an existing path.
    // This prevents accidental activation on random single tokens.
    if !input_path.exists() {
        return ControlFlow::Continue(());
    }

    let input_str = input_path.to_string_lossy().to_string();

    let ext = input_path
        .extension()
        .and_then(|s| s.to_str())
        .map(|s| s.to_lowercase());

    let result = match ext.as_deref() {
        Some("dds") => Err(eyre::eyre!(".dds files are not supported for decoding")),
        Some("tex") => {
            let mut out_path = input_path.to_path_buf();
            out_path.set_extension("png");
            let output = out_path.to_string_lossy().to_string();
            info!(
                input = %input_str,
                output = %output,
                "auto mode: decoding .tex to .png"
            );
            crate::commands::decode(crate::commands::DecodeCommandOptions {
                input: input_str,
                output,
                mipmap: 0,
            })
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

            crate::commands::encode(crate::commands::EncodeCommandOptions {
                input: input_str,
                output,
                format,
                generate_mipmaps,
                mipmap_filter,
            })
        }
    };

    ControlFlow::Break(result)
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
