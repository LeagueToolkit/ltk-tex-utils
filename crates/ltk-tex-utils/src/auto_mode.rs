//! Drag-and-drop auto mode (Windows): if invoked only with existing file/folder
//! paths, auto-route each to decode/encode based on extension and derive outputs
//! by changing the extension.

use std::ops::ControlFlow;
use std::path::Path;

use ltk_texture::tex::MipmapFilter;
use tracing::info;

use crate::batch::{run_batch, sibling_with_extension};
use crate::commands::{DecodeCommandOptions, EncodeCommandOptions, decode, encode};
use crate::utils::{ValidFormat, collect_input_files};

/// Attempts to handle an invocation whose arguments are all existing file/folder paths
/// (drag-and-drop style). Returns `Break(result)` if handled, `Continue(())` to proceed
/// with normal clap parsing.
pub fn try_handle() -> ControlFlow<eyre::Result<()>> {
    let raw: Vec<std::ffi::OsString> = std::env::args_os().skip(1).collect();
    if raw.is_empty() {
        return ControlFlow::Continue(());
    }

    let mut inputs = Vec::with_capacity(raw.len());
    for arg in &raw {
        // If an argument looks like a flag (e.g. --help, -h, /?),
        // defer to the normal CLI parser so help/version work as expected.
        let lossy = arg.to_string_lossy();
        if lossy.starts_with('-') || lossy.starts_with('/') {
            return ControlFlow::Continue(());
        }
        // Only trigger auto-mode if every argument resolves to an existing path.
        // This prevents accidental activation on subcommand names and typos.
        if !Path::new(arg).exists() {
            return ControlFlow::Continue(());
        }
        inputs.push(lossy.into_owned());
    }

    ControlFlow::Break(run_auto_mode(&inputs))
}

fn run_auto_mode(inputs: &[String]) -> eyre::Result<()> {
    // Folders expand to the .tex files they contain (decoded to .png).
    let files = collect_input_files(inputs, crate::commands::decode::DIR_EXTENSIONS)?;
    if files.is_empty() {
        eyre::bail!("no convertible files found");
    }
    run_batch(&files, auto_convert_file)
}

fn auto_convert_file(input: &Path) -> eyre::Result<()> {
    let ext = input
        .extension()
        .and_then(|s| s.to_str())
        .map(|s| s.to_lowercase());

    match ext.as_deref() {
        Some("tex") => {
            let output = sibling_with_extension(input, "png");
            info!(
                input = %input.display(),
                output = %output,
                "auto mode: decoding .tex to .png"
            );
            decode(DecodeCommandOptions {
                input: input.to_string_lossy().into_owned(),
                output,
                mipmap: 0,
            })
        }
        _ => {
            let output = sibling_with_extension(input, "tex");

            let format = ValidFormat::Bc3;
            let weigh_color_by_alpha = false;
            let generate_mipmaps = true;
            let mipmap_filter = MipmapFilter::CatmullRom;

            info!(
                input = %input.display(),
                output = %output,
                format = ?format,
                generate_mipmaps = generate_mipmaps,
                mipmap_filter = ?mipmap_filter,
                "auto mode: encoding image to .tex"
            );

            encode(EncodeCommandOptions {
                input: input.to_string_lossy().into_owned(),
                output,
                format,
                weigh_color_by_alpha,
                generate_mipmaps,
                mipmap_filter,
            })
        }
    }
}
