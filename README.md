<div align="center">
  <a href="https://github.com/LeagueToolkit">
    <img src="https://avatars.githubusercontent.com/u/28510182?s=200&v=4" alt="LeagueToolkit logo" width="96" height="96">
  </a>
  <h1>ltk-tex-utils</h1>
</div>

Small CLI utilities for working with League of Legends TEX textures, powered by `league-toolkit`.

### Features
- **Inspect TEX**: print format, dimensions, mipmaps, and resource type
- **Encode**: convert standard images (PNG/JPG/TGA/BMP/…) into `.tex`
- **Decode**: convert `.tex` back to common image formats (driven by output file extension)
- **Mipmaps**: optional generation with selectable filters

### Install

On Windows (recommended):

PowerShell (user scope):

```powershell
iwr -useb https://raw.githubusercontent.com/LeagueToolkit/ltk-tex-utils/main/scripts/install-windows.ps1 | iex
```

This downloads the latest release, installs it to `%LOCALAPPDATA%\LeagueToolkit\ltk-tex-utils`, adds a stable `bin` shim to your user `PATH`, and makes `ltk-tex-utils` available in new terminals.

From source (all platforms):

Prerequisites: Rust (stable) with `cargo`.

- From a local clone:

```bash
cargo install --path .
```

Or build locally and use the binary from `target/release`:

```bash
cargo build --release
# Binary: target/release/ltk-tex-utils(.exe)
```

### Usage

Top-level help:

```bash
ltk-tex-utils --help
```

Subcommands:

#### info
Prints basic metadata about a TEX file.

```bash
ltk-tex-utils info -i path/to/texture.tex
```

Example output:

```
info: path/to/texture.tex
    format : Bc3
    dimensions : 1024x1024
    mipmaps : 10 (has_mipmaps: true)
    resource : Texture2D
```

#### encode
Encode an image into `.tex`.

```bash
ltk-tex-utils encode \
  -i path/to/input.png \
  -o path/to/output.tex \
  -f <bc1|bc3|bgra8> \
  -m <true|false> \
  --mipmap-filter <nearest|triangle|catmullrom|lanczos3>
```

Notes:
- `-m/--generate-mipmaps` defaults to `true`. Pass `-m false` to disable mipmap generation.
- The input image is read via the `image` crate and supports common formats like PNG, JPEG, BMP, TIFF, TGA, etc.

Examples:

```bash
# BC3 with default mipmaps (triangle)
ltk-tex-utils encode -i albedo.png -o albedo.tex -f bc3

# Disable mipmaps
ltk-tex-utils encode -i icon.png -o icon.tex -f bgra8 -m false

# BC1 with a different mipmap filter
ltk-tex-utils encode -i mask.png -o mask.tex -f bc1 --mipmap-filter lanczos3
```

#### decode
Decode a `.tex` file into a standard image.

```bash
ltk-tex-utils decode -i path/to/input.tex -o path/to/output.png
```

Notes:
- The output image format is inferred from the file extension (e.g., `.png`, `.jpg`, `.tiff`).
- Currently decodes the top-level mip (mip 0).

### Supported formats and filters

- **Texture formats**: `bc1`, `bc3`, `bgra8`  
  (ETC1/ETC2 are not supported.)
- **Mipmap filters**: `nearest`, `triangle` (default), `catmullrom`, `lanczos3`

### Logging

The tool emits human-friendly logs. Informational, debug, and trace logs go to stdout; warnings and errors go to stderr.

### Development

Run from source:

```bash
cargo run -- <subcommand> [options]
```

Examples:

```bash
cargo run -- info -i samples/texture.tex
cargo run -- encode -i samples/albedo.png -o out/albedo.tex -f bc3
cargo run -- decode -i samples/texture.tex -o out/texture.png
```

### Acknowledgements

Built on top of `league-toolkit`'s texture APIs.


