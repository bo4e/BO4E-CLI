use clap::Command;
use std::path::Path;

pub fn script(cmd: &mut Command) -> String {
    let mut buf = Vec::new();
    clap_complete::generate(clap_complete::Shell::PowerShell, cmd, "bo4e", &mut buf);
    String::from_utf8(buf).expect("clap_complete output is valid UTF-8")
}

/// PowerShell embeds the entire script in $PROFILE rather than sourcing an
/// external file. The "script" arg is ignored here; the install flow embeds
/// `script(cmd)` directly inside the marker block.
pub fn rc_body(_script: &Path) -> String {
    String::new()
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::CommandFactory;

    #[test]
    fn script_contains_register_argumentcompleter() {
        let mut cmd = crate::cli::base::Cli::command();
        let s = script(&mut cmd);
        assert!(s.contains("Register-ArgumentCompleter"));
    }
}
