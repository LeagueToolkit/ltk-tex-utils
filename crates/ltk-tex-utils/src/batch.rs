use std::path::{Path, PathBuf};

use crate::utils::collect_input_files;

/// Merge `-i/--input` with the positional inputs and expand folders into files.
pub fn gather_inputs(
    flag: Option<String>,
    mut inputs: Vec<String>,
    dir_extensions: &[&str],
) -> eyre::Result<Vec<PathBuf>> {
    if let Some(flag) = flag {
        inputs.insert(0, flag);
    }
    if inputs.is_empty() {
        eyre::bail!("missing input; pass -i/--input or provide INPUTS positionally");
    }

    let files = collect_input_files(&inputs, dir_extensions)?;
    if files.is_empty() {
        eyre::bail!(
            "no matching input files found (looked for {} in folders)",
            dir_extensions.join(", ")
        );
    }
    Ok(files)
}

/// `-o/--output` is only meaningful when converting a single file.
pub fn single_output(output: Option<String>, files: &[PathBuf]) -> eyre::Result<Option<String>> {
    if output.is_some() && files.len() > 1 {
        eyre::bail!(
            "-o/--output cannot be used with multiple inputs ({} files); \
             outputs are written next to each input",
            files.len()
        );
    }
    Ok(output)
}

/// Convert each file, logging failures but continuing; errors out at the end if any failed.
pub fn run_batch(
    files: &[PathBuf],
    per_file: impl Fn(&Path) -> eyre::Result<()>,
) -> eyre::Result<()> {
    let mut failed = 0usize;
    for file in files {
        if let Err(err) = per_file(file) {
            failed += 1;
            tracing::error!("failed to convert {}: {err:#}", file.display());
        }
    }
    if failed > 0 {
        eyre::bail!("{failed} of {} file(s) failed to convert", files.len());
    }
    Ok(())
}

pub fn sibling_with_extension(input: &Path, extension: &str) -> String {
    let mut out = input.to_path_buf();
    out.set_extension(extension);
    out.to_string_lossy().into_owned()
}
