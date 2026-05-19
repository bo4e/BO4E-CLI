use clap::Command;
use std::path::Path;

pub fn script(cmd: &mut Command) -> String {
    let mut buf = Vec::new();
    clap_complete::generate(clap_complete_nushell::Nushell, cmd, "bo4e", &mut buf);
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
    fn script_is_non_empty() {
        let mut cmd = crate::cli::base::Cli::command();
        let s = script(&mut cmd);
        assert!(!s.is_empty());
    }

    #[test]
    fn rc_body_sources_script() {
        assert_eq!(rc_body(Path::new("/tmp/bo4e.nu")), "source '/tmp/bo4e.nu'");
    }
}
