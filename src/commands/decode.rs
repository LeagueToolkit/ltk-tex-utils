use std::{fs::File, io::BufReader};

use league_toolkit::texture::Tex;

pub struct DecodeCommandOptions {
    pub input: String,
    pub output: String,
}

pub fn decode(options: DecodeCommandOptions) -> eyre::Result<()> {
    let file = File::open(&options.input)?;
    let mut reader = BufReader::new(file);

    let tex = Tex::from_reader(&mut reader)?;

    let image = tex.decode_mipmap(0)?;
    let image = image.into_rgba_image()?;
    image.save(&options.output)?;

    Ok(())
}
