use std::fs;
use std::path::{Path, PathBuf};

use ltk_texture::tex::{EncodeFormat, MipmapFilter};

#[macro_export]
macro_rules! println_pad {
    ($($arg:tt)*) => {{
        let __s = format!($($arg)*);
        for __line in __s.lines() {
            println!("    {}", __line);
        }
    }};
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ValidFormat {
    Bc1,
    Bc3,
    Bc7,
    Bgra8,
    Rgba16Float,
    Rgba32Float,
}

impl ValidFormat {
    /// Build the `ltk_texture` encode format, applying any format-specific options.
    ///
    /// `weigh_color_by_alpha` only affects the BC1/BC3 cluster fit; it is ignored
    /// for the other formats.
    pub fn to_encode_format(self, weigh_color_by_alpha: bool) -> EncodeFormat {
        match self {
            ValidFormat::Bc1 => EncodeFormat::Bc1 {
                weigh_colour_by_alpha: weigh_color_by_alpha,
            },
            ValidFormat::Bc3 => EncodeFormat::Bc3 {
                weigh_colour_by_alpha: weigh_color_by_alpha,
            },
            ValidFormat::Bc7 => EncodeFormat::Bc7,
            ValidFormat::Bgra8 => EncodeFormat::Bgra8,
            ValidFormat::Rgba16Float => EncodeFormat::Rgba16Float,
            ValidFormat::Rgba32Float => EncodeFormat::Rgba32Float,
        }
    }
}

/// Output image format for `decode` when no explicit output path is given.
#[derive(clap::ValueEnum, Debug, Clone, Copy, PartialEq, Eq)]
pub enum DecodeOutputFormat {
    /// PNG image
    Png,
    /// Uncompressed RGBA8 DDS (top mip only)
    Dds,
}

impl DecodeOutputFormat {
    pub fn extension(self) -> &'static str {
        match self {
            DecodeOutputFormat::Png => "png",
            DecodeOutputFormat::Dds => "dds",
        }
    }
}

/// Expand a mix of file and directory inputs into a flat list of files.
///
/// Directories are walked recursively, keeping files whose extension matches
/// `dir_extensions` (case-insensitive). Explicitly listed files are kept as-is.
pub fn collect_input_files(
    inputs: &[String],
    dir_extensions: &[&str],
) -> eyre::Result<Vec<PathBuf>> {
    let mut files = Vec::new();
    for input in inputs {
        let path = PathBuf::from(input);
        if path.is_dir() {
            collect_dir_files(&path, dir_extensions, &mut files)?;
        } else if path.is_file() {
            files.push(path);
        } else {
            eyre::bail!("input does not exist: {input}");
        }
    }
    Ok(files)
}

fn collect_dir_files(
    dir: &Path,
    extensions: &[&str],
    files: &mut Vec<PathBuf>,
) -> eyre::Result<()> {
    for entry in fs::read_dir(dir)? {
        let path = entry?.path();
        if path.is_dir() {
            collect_dir_files(&path, extensions, files)?;
        } else if path
            .extension()
            .and_then(|e| e.to_str())
            .is_some_and(|e| extensions.iter().any(|x| x.eq_ignore_ascii_case(e)))
        {
            files.push(path);
        }
    }
    Ok(())
}

pub fn parse_mipmap_filter(s: &str) -> Result<MipmapFilter, String> {
    match s.to_lowercase().as_str() {
        "nearest" => Ok(MipmapFilter::Nearest),
        "triangle" => Ok(MipmapFilter::Triangle),
        "catmullrom" => Ok(MipmapFilter::CatmullRom),
        "lanczos3" => Ok(MipmapFilter::Lanczos3),
        _ => Err(format!(
            "Invalid mipmap filter: {}. Valid options: nearest, triangle, catmullrom, lanczos3",
            s
        )),
    }
}

pub fn parse_format(s: &str) -> Result<ValidFormat, String> {
    match s.to_lowercase().as_str() {
        "bc1" => Ok(ValidFormat::Bc1),
        "bc3" => Ok(ValidFormat::Bc3),
        "bc7" => Ok(ValidFormat::Bc7),
        "bgra8" => Ok(ValidFormat::Bgra8),
        "rgba16f" | "rgba16float" => Ok(ValidFormat::Rgba16Float),
        "rgba32f" | "rgba32float" => Ok(ValidFormat::Rgba32Float),
        _ => Err(format!(
            "Invalid format: {}. Valid options: bc1, bc3, bc7, bgra8, rgba16f, rgba32f \
             (ETC1, ETC2 and BC5 are not supported for encoding)",
            s
        )),
    }
}
