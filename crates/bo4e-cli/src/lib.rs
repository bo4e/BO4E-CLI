//! Library facade exposing the CLI's modules to integration tests.
//! The binary entrypoint remains in `main.rs`.

pub mod cli;
pub mod console;
pub mod diff;
pub mod edit;
pub mod io;
pub mod models;
pub mod repo;
pub mod utils;

/// Process-global CWD mutex for tests that mutate `std::env::set_current_dir`. Only available to the library's own unit tests — integration tests compile against the lib without `--cfg test` and cannot reach this module.
///
/// A process-global mutex used by tests in multiple modules that call
/// `std::env::set_current_dir`. Cargo runs tests in parallel by default;
/// any test that mutates the process cwd must hold this lock for the
/// duration of the test.
#[cfg(test)]
pub(crate) mod test_lock {
    use std::sync::Mutex;
    pub(crate) static CWD_LOCK: Mutex<()> = Mutex::new(());
}
