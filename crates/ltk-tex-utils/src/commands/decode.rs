use std::{
    fs::{self, File},
    io::BufReader,
    path::Path,
};

use league_toolkit::texture::Tex;

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
    image.save(&options.output)?;

    Ok(())
}
