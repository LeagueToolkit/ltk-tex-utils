use league_toolkit::texture::tex::MipmapFilter;

use crate::utils::ValidFormat;

pub struct EncodeCommandOptions {
    pub input: String,
    pub output: String,
    pub format: ValidFormat,
    pub generate_mipmaps: bool,
    pub mipmap_filter: MipmapFilter,
}

pub fn encode(options: EncodeCommandOptions) {
    println!("Encode command");
}
