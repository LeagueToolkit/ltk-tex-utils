use std::{fs::File, io::BufReader, path::Path};

use league_toolkit::texture::{Dds, Tex};

pub struct DecodeCommandOptions {
    pub input: String,
    pub output: String,
}

pub fn decode(options: DecodeCommandOptions) -> eyre::Result<()> {
    let file = File::open(&options.input)?;
    let mut reader = BufReader::new(file);

    // Detect file type from extension
    let input_path = Path::new(&options.input);
    let ext = input_path
        .extension()
        .and_then(|s| s.to_str())
        .map(|s| s.to_lowercase());

    let image = match ext.as_deref() {
        Some("dds") => {
            let dds = Dds::from_reader(&mut reader)?;
            let surface = dds.decode_mipmap(0)?;
            surface.into_image()?
        }
        Some("tex") => {
            let tex = Tex::from_reader(&mut reader)?;
            let surface = tex.decode_mipmap(0)?;
            surface.into_rgba_image()?
        }
        _ => {
            let tex = Tex::from_reader(&mut reader)?;
            let surface = tex.decode_mipmap(0)?;
            surface.into_rgba_image()?
        }
    };

    image.save(&options.output)?;

    Ok(())
}
