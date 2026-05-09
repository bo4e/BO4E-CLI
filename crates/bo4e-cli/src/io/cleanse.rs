use std::path::Path;

/// Clear (and delete) the directory if `clear_output` is true and the directory exists.
/// If the path points to a file instead of a directory, an error is returned.
/// If `clear_output` is false, the function does nothing and returns Ok(()).
/// If the directory does not exist, it is also considered a success (no error).
pub fn clear_dir_if_needed(output_dir: &Path, clear_output: bool) -> std::io::Result<()> {
    if clear_output && output_dir.try_exists()? {
        if !output_dir.is_dir() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::NotADirectory,
                "Tried to clear a directory, but the path points to a file.",
            ));
        }
        return std::fs::remove_dir_all(&output_dir);
    }
    Ok(())
}
