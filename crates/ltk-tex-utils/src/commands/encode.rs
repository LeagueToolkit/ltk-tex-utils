use std::{
    fs::{self, File},
    io::{BufReader, BufWriter},
    path::Path,
};

use ltk_texture::{
    Dds, Tex,
    tex::{EncodeOptions, MipmapFilter},
};
use tracing::info;

use crate::batch::{gather_inputs, run_batch, sibling_with_extension, single_output};
use crate::utils::{ValidFormat, parse_format, parse_mipmap_filter};

/// File extensions picked up when a directory is passed to `encode`.
pub const DIR_EXTENSIONS: &[&str] = &["png", "dds"];

#[derive(clap::Args, Debug)]
pub struct EncodeArgs {
    /// Images (.png/.dds/...) or folders to encode; folders are searched
    /// recursively for .png/.dds files
    #[arg(value_name = "INPUTS", required_unless_present = "input")]
    pub inputs: Vec<String>,

    /// Texture (image) to encode (alternative to positional INPUTS)
    #[arg(short, long, value_name = "INPUT")]
    pub input: Option<String>,

    /// Output file path (only valid with a single input file)
    /// Defaults to writing next to each input with a `.tex` extension.
    #[arg(short, long, value_name = "OUTPUT")]
    pub output: Option<String>,

    /// Texture format to encode to
    /// (bc1, bc3, bc7, bgra8, rgba16f, rgba32f)
    #[arg(short, long, value_parser = parse_format, default_value = "bc3")]
    pub format: ValidFormat,

    /// Weigh color by alpha during the BC1/BC3 cluster fit.
    /// Improves perceived quality for alpha-blended textures at the cost of
    /// color accuracy in transparent regions. Ignored for other formats.
    #[arg(long, default_value = "false")]
    pub weigh_color_by_alpha: bool,

    /// Whether to generate mipmaps
    #[arg(short = 'm', long, default_value = "true")]
    pub generate_mipmaps: bool,

    /// Filter type to use for mipmap generation
    #[arg(long, default_value = "catmullrom", value_parser = parse_mipmap_filter)]
    pub mipmap_filter: MipmapFilter,
}

pub fn run(args: EncodeArgs) -> eyre::Result<()> {
    let files = gather_inputs(args.input, args.inputs, DIR_EXTENSIONS)?;
    let output = single_output(args.output, &files)?;
    run_batch(&files, |file| {
        let out = output
            .clone()
            .unwrap_or_else(|| sibling_with_extension(file, "tex"));
        info!("encoding {} -> {}", file.display(), out);
        encode(EncodeCommandOptions {
            input: file.to_string_lossy().into_owned(),
            output: out,
            format: args.format,
            weigh_color_by_alpha: args.weigh_color_by_alpha,
            generate_mipmaps: args.generate_mipmaps,
            mipmap_filter: args.mipmap_filter,
        })
    })
}

pub struct EncodeCommandOptions {
    pub input: String,
    pub output: String,
    pub format: ValidFormat,
    pub weigh_color_by_alpha: bool,
    pub generate_mipmaps: bool,
    pub mipmap_filter: MipmapFilter,
}

pub fn encode(options: EncodeCommandOptions) -> eyre::Result<()> {
    let image = load_input_image(&options.input)?;

    let tex = Tex::encode_rgba_image(
        &image,
        EncodeOptions {
            format: options
                .format
                .to_encode_format(options.weigh_color_by_alpha),
            generate_mipmaps: options.generate_mipmaps,
            mipmap_filter: options.mipmap_filter,
        },
    )?;

    let output_path = Path::new(&options.output);
    if let Some(parent) = output_path.parent()
        && !parent.as_os_str().is_empty()
    {
        fs::create_dir_all(parent)?;
    }
    let file = File::create(&options.output)?;
    let mut writer = BufWriter::new(file);

    tex.write(&mut writer)?;

    Ok(())
}

/// Load the input as an RGBA8 image. DDS inputs are decoded through `ltk_texture`
/// (top mip only), since the `image` crate cannot read block-compressed DDS.
fn load_input_image(input: &str) -> eyre::Result<image::RgbaImage> {
    let ext = Path::new(input)
        .extension()
        .and_then(|e| e.to_str())
        .map(str::to_lowercase);

    match ext.as_deref() {
        Some("tex") => Err(eyre::eyre!("input is already a .tex texture: {input}")),
        Some("dds") => {
            let file = File::open(input)?;
            let mut reader = BufReader::new(file);
            let dds = Dds::from_reader(&mut reader)?;
            Ok(dds.decode_mipmap(0)?.into_image()?)
        }
        _ => Ok(image::open(input)?.to_rgba8()),
    }
}
