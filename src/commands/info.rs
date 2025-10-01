use colored::Colorize;
use league_toolkit::texture::tex::Tex;
use std::fs::File;
use std::io::BufReader;

pub struct InfoCommandOptions {
    pub input: String,
}

pub fn info(options: InfoCommandOptions) {
    let path = &options.input;
    let file = match File::open(path) {
        Ok(f) => f,
        Err(err) => {
            eprintln!(
                "{} {}",
                "error:".bold().red(),
                format!("failed to open '{}': {}", path, err)
            );
            return;
        }
    };
    let mut reader = BufReader::new(file);

    let tex = match Tex::from_reader(&mut reader) {
        Ok(tex) => tex,
        Err(err) => {
            eprintln!(
                "{} {}",
                "error:".bold().red(),
                format!("failed to read TEX from '{}': {:?}", path, err)
            );
            return;
        }
    };

    println!("{} {}", "info:".bold().blue(), path.bold());
    crate::println_pad!(
        "{} {}",
        "format".bold().cyan(),
        format!(": {}", format!("{:?}", tex.format).green())
    );
    crate::println_pad!(
        "{} {}",
        "dimensions".bold().cyan(),
        format!(": {}x{}", tex.width, tex.height).green()
    );
    crate::println_pad!(
        "{} {}",
        "mipmaps".bold().cyan(),
        format!(": {} (has_mipmaps: {})", tex.mip_count, tex.has_mipmaps()).green()
    );
    crate::println_pad!(
        "{} {}",
        "resource".bold().cyan(),
        format!(": {}", tex.resource_type).green()
    );
}
