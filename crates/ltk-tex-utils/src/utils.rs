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
