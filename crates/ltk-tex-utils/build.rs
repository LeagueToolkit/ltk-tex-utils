use std::path::Path;

fn main() {
    // Ensure C++ standard library is linked when pulling in crates
    // that use C++/ISPC (e.g., intel_tex_2 via league_toolkit).
    let target_os = std::env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();

    match target_os.as_str() {
        // On Linux (GNU/Musl), libstdc++ provides __gxx_personality_v0 and others
        "linux" => println!("cargo:rustc-link-lib=dylib=stdc++"),
        // On macOS, the C++ runtime is libc++
        "macos" => println!("cargo:rustc-link-lib=dylib=c++"),
        _ => {}
    }

    embed_windows_icon(&target_os);
}

/// Embeds the application icon into the Windows executable so the binary (and the
/// Explorer context-menu entries that reference it) carry LeagueToolkit branding.
///
/// The icon is optional - if `assets/ltk-tex-utils.ico` is not present the embed is
/// skipped so source builds keep working without the asset. Non-Windows targets do
/// nothing.
fn embed_windows_icon(target_os: &str) {
    let icon = "assets/ltk-tex-utils.ico";
    println!("cargo:rerun-if-changed={icon}");

    if target_os == "windows" && Path::new(icon).exists() {
        #[cfg(windows)]
        {
            let mut res = winresource::WindowsResource::new();
            res.set_icon(icon);
            if let Err(e) = res.compile() {
                println!("cargo:warning=failed to embed Windows icon: {e}");
            }
        }
    }
}
