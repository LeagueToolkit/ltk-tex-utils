use std::{
    fs::{self, File},
    io::BufWriter,
    path::Path,
};

use league_toolkit::texture::{
    Tex,
    tex::{EncodeOptions, MipmapFilter},
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

    let tex = Tex::from_rgba_image(
        &image,
        EncodeOptions {
            format: options.format.into(),
            generate_mipmaps: options.generate_mipmaps,
            mipmap_filter: options.mipmap_filter,
        },
    )?;

    fs::create_dir_all(Path::new(&options.output).parent().unwrap())?;
    let file = File::create(&options.output)?;
    let mut writer = BufWriter::new(file);

    tex.write(&mut writer)?;

    Ok(())
}
