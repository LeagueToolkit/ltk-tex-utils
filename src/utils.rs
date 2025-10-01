use league_toolkit::texture::tex::{Format, MipmapFilter};

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
    Bgra8,
}

impl From<ValidFormat> for Format {
    fn from(val: ValidFormat) -> Self {
        match val {
            ValidFormat::Bc1 => Format::Bc1,
            ValidFormat::Bc3 => Format::Bc3,
            ValidFormat::Bgra8 => Format::Bgra8,
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
        "bgra8" => Ok(ValidFormat::Bgra8),
        _ => Err(format!(
            "Invalid format: {}. Valid options: bc1, bc3, bgra8 (ETC1 and ETC2 are not supported)",
            s
        )),
    }
}
