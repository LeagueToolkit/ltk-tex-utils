use std::{
    fs::{self, File},
    io::BufWriter,
    path::Path,
};

use ltk_texture::{
    Tex,
    tex::{EncodeFormat, EncodeOptions, Format, MipmapFilter},
};

use crate::utils::ValidFormat;

pub struct EncodeCommandOptions {
    pub input: String,
    pub output: String,
    pub format: ValidFormat,
    pub generate_mipmaps: bool,
    pub mipmap_filter: MipmapFilter,
}

pub fn encode(options: EncodeCommandOptions) -> eyre::Result<()> {
    let image = image::open(&options.input)?;
    let image = image.to_rgba8();

    let tex = Tex::encode_rgba_image(
        &image,
        EncodeOptions {
            format: EncodeFormat::try_from(Format::from(options.format))?,
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
