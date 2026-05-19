use clap::Command;
use clap_complete::env::EnvCompleter as _;
use clap_complete::env::Powershell;
use std::path::Path;

pub fn script(_cmd: &mut Command) -> String {
    let mut buf = Vec::new();
    Powershell
        .write_registration("COMPLETE", "bo4e", "bo4e", "bo4e", &mut buf)
        .expect("write_registration");
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
    fn script_emits_register_argumentcompleter() {
        let mut cmd = crate::cli::base::Cli::command();
        let s = script(&mut cmd);
        assert!(
            s.contains("Register-ArgumentCompleter"),
            "expected Register-ArgumentCompleter: {}",
            &s[..200.min(s.len())]
        );
        assert!(s.contains("bo4e"));
    }
}
