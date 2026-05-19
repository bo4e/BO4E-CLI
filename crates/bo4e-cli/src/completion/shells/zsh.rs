use clap::Command;
use clap_complete::env::EnvCompleter as _;
use clap_complete::env::Zsh;
use std::path::Path;

pub fn script(_cmd: &mut Command) -> String {
    let mut buf = Vec::new();
    Zsh.write_registration("COMPLETE", "bo4e", "bo4e", "bo4e", &mut buf)
        .expect("write_registration");
    String::from_utf8(buf).expect("clap_complete output is valid UTF-8")
}

pub fn rc_body(script: &Path) -> String {
    format!("source '{}'", script.display())
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::CommandFactory;

    #[test]
    fn script_emits_compdef_registering_bo4e() {
        let mut cmd = crate::cli::base::Cli::command();
        let s = script(&mut cmd);
        assert!(
            s.contains("compdef") || s.contains("compctl"),
            "expected zsh registration: {}",
            &s[..200.min(s.len())]
        );
        assert!(s.contains("bo4e"), "expected `bo4e` in script");
        assert!(s.contains("COMPLETE"), "expected COMPLETE env-var wiring");
    }

    #[test]
    fn rc_body_uses_source_line() {
        assert_eq!(rc_body(Path::new("/tmp/_bo4e")), "source '/tmp/_bo4e'");
    }
}
