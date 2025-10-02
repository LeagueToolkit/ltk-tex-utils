use colored::Colorize;
use league_toolkit::texture::{Dds, Tex};
use std::fs::File;
use std::io::BufReader;
use std::path::Path;

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

    // Detect file type from extension
    let input_path = Path::new(path);
    let ext = input_path
        .extension()
        .and_then(|s| s.to_str())
        .map(|s| s.to_lowercase());

    match ext.as_deref() {
        Some("dds") => {
            let dds = match Dds::from_reader(&mut reader) {
                Ok(dds) => dds,
                Err(err) => {
                    eprintln!(
                        "{} failed to read DDS from '{}': {:?}",
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
                ": DDS".green()
            );
            crate::println_pad!(
                "{} {}",
                "dimensions".bold().cyan(),
                format!(": {}x{}", dds.width(), dds.height()).green()
            );
            crate::println_pad!(
                "{} {}",
                "mipmaps".bold().cyan(),
                format!(": {}", dds.mip_count()).green()
            );
        }
        Some("tex") | _ => {
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
                format!(": {}", tex.resource_type).green()
            );
        }
    }
}
