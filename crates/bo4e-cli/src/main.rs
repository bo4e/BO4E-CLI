use bo4e_cli::cli;
use bo4e_cli::cli::base::Executable;
use bo4e_cli::console::console::{CONSOLE, Console, Level};
use bo4e_cli::console::highlighter::Highlighter;

#[cfg(feature = "dynamic-completion")]
use clap::CommandFactory as _;
use clap::Parser;
use clap::error::ErrorKind;

fn main() -> Result<(), String> {
    #[cfg(feature = "dynamic-completion")]
    clap_complete::CompleteEnv::with_factory(cli::base::Cli::command).complete();

    match cli::base::Cli::try_parse() {
        Ok(args) => {
            if args.show_version {
                println!("v{}", env!("CARGO_PKG_VERSION"));
                return Ok(());
            }
            let level = match (args.verbose, args.quiet) {
                (true, _) => Level::Verbose,
                (_, true) => Level::Quiet,
                _ => Level::Normal,
            };
            CONSOLE
                .set(Console::new(level))
                .map_err(|_| "CONSOLE already initialized".to_string())?;
            args.run()
        }
        Err(e)
            if matches!(
                e.kind(),
                ErrorKind::DisplayHelp | ErrorKind::DisplayHelpOnMissingArgumentOrSubcommand
            ) =>
        {
            // Run clap's rendered help text through the BO4E highlighter so
            // schema names, BO4E, versions, etc. get the same colouring as
            // every other line of CLI output. `StyledStr::to_string()` is the
            // plain-text form (no ANSI); `add_help_rules` then adds back
            // structural styling (headers, flags, placeholders, URLs).
            let plain = e.render().to_string();
            let mut h = Highlighter::default();
            h.add_help_rules();
            print!("{}", h.apply(&plain));
            std::process::exit(0);
        }
        Err(e) => e.exit(),
    }
}
