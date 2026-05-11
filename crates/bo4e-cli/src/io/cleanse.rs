use crate::console::spinner;
use crate::{cprint_normal, cprint_verbose};
use std::path::Path;

/// Clear (and delete) the directory if `clear_output` is true and the directory exists.
/// If the path points to a file instead of a directory, an error is returned.
/// If `clear_output` is false, the function does nothing and returns Ok(()).
/// If the directory does not exist, it is also considered a success (no error).
pub fn clear_dir_if_needed(output_dir: &Path, clear_output: bool) -> std::io::Result<()> {
    if !clear_output {
        return Ok(());
    }
    if !output_dir.try_exists()? {
        cprint_verbose!(
            "Directory {} does not exist, nothing to clear.",
            output_dir.display()
        );
        return Ok(());
    }
    if !output_dir.is_dir() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::NotADirectory,
            "Tried to clear a directory, but the path points to a file.",
        ));
    }
    let _entries_removed = std::fs::read_dir(output_dir)?.count();
    {
        let _spin = spinner::grenade(format!("Clearing directory {}", output_dir.display()));
        std::fs::remove_dir_all(output_dir)?;
    }
    cprint_normal!("Cleared directory {}", output_dir.display());
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::console::console::{CONSOLE, Console, Level};

    fn ensure_console() {
        let _ = CONSOLE.set(Console::new(Level::Verbose));
    }

    #[test]
    fn clears_existing_directory() {
        ensure_console();
        let dir = tempfile::tempdir().unwrap();
        let nested = dir.path().join("nested");
        std::fs::create_dir(&nested).unwrap();
        std::fs::write(nested.join("a.txt"), b"x").unwrap();
        clear_dir_if_needed(&nested, true).unwrap();
        assert!(!nested.exists());
    }

    #[test]
    fn nonexistent_directory_is_ok() {
        ensure_console();
        let dir = tempfile::tempdir().unwrap();
        let missing = dir.path().join("missing");
        clear_dir_if_needed(&missing, true).unwrap();
    }

    #[test]
    fn clear_output_false_is_noop() {
        ensure_console();
        let dir = tempfile::tempdir().unwrap();
        let nested = dir.path().join("nested");
        std::fs::create_dir(&nested).unwrap();
        std::fs::write(nested.join("a.txt"), b"x").unwrap();
        clear_dir_if_needed(&nested, false).unwrap();
        assert!(nested.exists());
    }

    #[test]
    fn file_path_is_error() {
        ensure_console();
        let dir = tempfile::tempdir().unwrap();
        let f = dir.path().join("file");
        std::fs::write(&f, b"x").unwrap();
        let err = clear_dir_if_needed(&f, true).unwrap_err();
        assert_eq!(err.kind(), std::io::ErrorKind::NotADirectory);
    }
}
