use std::{
    fs::{self, File},
    io::{BufReader, BufWriter},
    path::Path,
};

use image_dds::{ImageFormat, Mipmaps, Quality};
use ltk_texture::Tex;
use tracing::info;

use crate::batch::{gather_inputs, run_batch, sibling_with_extension, single_output};
use crate::utils::DecodeOutputFormat;

/// File extensions picked up when a directory is passed to `decode`.
pub const DIR_EXTENSIONS: &[&str] = &["tex"];

#[derive(clap::Args, Debug)]
pub struct DecodeArgs {
    /// Textures (.tex) or folders to decode; folders are searched
    /// recursively for .tex files
    #[arg(value_name = "INPUTS", required_unless_present = "input")]
    pub inputs: Vec<String>,

    /// Texture (.tex) to decode (alternative to positional INPUTS)
    #[arg(short, long, value_name = "INPUT")]
    pub input: Option<String>,

    /// Output file path (only valid with a single input file)
    /// The output directory will be created if it doesn't exist
    /// The output format will be determined by the file extension
    /// Defaults to writing next to each input with the `--format` extension.
    #[arg(short, long, value_name = "OUTPUT")]
    pub output: Option<String>,

    /// Output image format used when --output is not given
    #[arg(short, long, value_enum, default_value_t = DecodeOutputFormat::Png)]
    pub format: DecodeOutputFormat,

    /// Mipmap to decode (0 = largest)
    #[arg(short, long, default_value = "0")]
    pub mipmap: u32,
}

pub fn run(args: DecodeArgs) -> eyre::Result<()> {
    let files = gather_inputs(args.input, args.inputs, DIR_EXTENSIONS)?;
    let output = single_output(args.output, &files)?;
    run_batch(&files, |file| {
        let out = output
            .clone()
            .unwrap_or_else(|| sibling_with_extension(file, args.format.extension()));
        info!("decoding {} -> {}", file.display(), out);
        decode(DecodeCommandOptions {
            input: file.to_string_lossy().into_owned(),
            output: out,
            mipmap: args.mipmap,
        })
    })
}

pub struct DecodeCommandOptions {
    pub input: String,
    pub output: String,
    pub mipmap: u32,
}

pub fn decode(options: DecodeCommandOptions) -> eyre::Result<()> {
    let file = File::open(&options.input)?;
    let mut reader = BufReader::new(file);

    let tex = Tex::from_reader(&mut reader)?;

    let image = tex.decode_mipmap(options.mipmap)?;
    let image = image.into_rgba_image()?;

    let output_path = Path::new(&options.output);
    if let Some(parent) = output_path.parent()
        && !parent.as_os_str().is_empty()
    {
        fs::create_dir_all(parent)?;
    }

    let is_dds = output_path
        .extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| e.eq_ignore_ascii_case("dds"));

    if is_dds {
        // The `image` crate cannot write DDS; emit an uncompressed RGBA8 DDS
        // holding the single decoded mip.
        let dds = image_dds::dds_from_image(
            &image,
            ImageFormat::Rgba8Unorm,
            Quality::Fast,
            Mipmaps::Disabled,
        )?;
        let file = File::create(output_path)?;
        let mut writer = BufWriter::new(file);
        dds.write(&mut writer)?;
    } else {
        image.save(&options.output)?;
    }

    Ok(())
}
