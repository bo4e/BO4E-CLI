use bo4e_cli::cli::base::Executable;
use bo4e_cli::console::console::{Console, Level, CONSOLE};
use bo4e_cli::cli;

use clap::Parser;

fn main() -> Result<(), String> {
    let args = cli::base::Cli::parse();
    let level = match (args.verbose, args.quiet) {
        (true, _) => Level::Verbose,
        (_, true) => Level::Quiet,
        _         => Level::Normal,
    };
    CONSOLE
        .set(Console::new(level))
        .map_err(|_| "CONSOLE already initialized".to_string())?;
    args.run()
}
