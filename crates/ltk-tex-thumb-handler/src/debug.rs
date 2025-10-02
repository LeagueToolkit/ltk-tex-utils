// =============================================================================
// DEBUG LOGGING
// =============================================================================

use std::io::Write;

#[allow(dead_code)]
pub fn debug_log(msg: &str) {
    if let Ok(mut file) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open("C:\\temp\\ltk_tex_thumb_debug.log")
    {
        let _ = writeln!(
            file,
            "[{}] {}",
            chrono::Local::now().format("%Y-%m-%d %H:%M:%S"),
            msg
        );
    }
}
