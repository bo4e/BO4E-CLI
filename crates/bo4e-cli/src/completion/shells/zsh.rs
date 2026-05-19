use clap::Command;
use std::path::Path;

pub fn script(cmd: &mut Command) -> String {
    let mut buf = Vec::new();
    clap_complete::generate(clap_complete::Shell::Zsh, cmd, "bo4e", &mut buf);
    String::from_utf8(buf).expect("clap_complete output is valid UTF-8")
}

pub fn rc_body(_script: &Path) -> String {
    "fpath+=~/.zfunc; autoload -Uz compinit; compinit".to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::CommandFactory;

    #[test]
    fn script_starts_with_zsh_compdef() {
        let mut cmd = crate::cli::base::Cli::command();
        let s = script(&mut cmd);
        assert!(s.starts_with("#compdef") || s.contains("#compdef bo4e"));
    }

    #[test]
    fn rc_body_configures_fpath() {
        let s = rc_body(Path::new("/tmp/_bo4e"));
        assert!(s.contains("fpath+=~/.zfunc"));
        assert!(s.contains("compinit"));
    }
}
