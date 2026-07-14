use colored::Colorize;
use ltk_texture::tex::Tex;
use std::fs::File;
use std::io::BufReader;

#[derive(clap::Args, Debug)]
pub struct InfoArgs {
    /// Texture to get info from
    #[arg(short, long)]
    pub input: String,
}

pub fn run(args: InfoArgs) {
    info(InfoCommandOptions { input: args.input });
}

pub struct InfoCommandOptions {
    pub input: String,
}

pub fn info(options: InfoCommandOptions) {
    let path = &options.input;
    let file = match File::open(path) {
        Ok(f) => f,
        Err(err) => {
            eprintln!(
                "{} failed to open '{}': {}",
                "error:".bold().red(),
                path,
                err
            );
            return;
        }
    };
    let mut reader = BufReader::new(file);

    let tex = match Tex::from_reader(&mut reader) {
        Ok(tex) => tex,
        Err(err) => {
            eprintln!(
                "{} failed to read TEX from '{}': {:?}",
                "error:".bold().red(),
                path,
                err
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
        format!(": {:?}", tex.resource_type).green()
    );
}
