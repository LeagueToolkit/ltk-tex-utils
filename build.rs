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
}
