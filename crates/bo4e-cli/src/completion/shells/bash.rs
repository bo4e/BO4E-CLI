use clap::Command;
use std::path::Path;

pub fn script(cmd: &mut Command) -> String {
    let mut buf = Vec::new();
    clap_complete::generate(clap_complete::Shell::Bash, cmd, "bo4e", &mut buf);
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
    fn script_includes_completion_function() {
        let mut cmd = crate::cli::base::Cli::command();
        let s = script(&mut cmd);
        assert!(
            s.contains("_bo4e()"),
            "expected bash completion function: {}",
            &s[..200.min(s.len())]
        );
    }

    #[test]
    fn rc_body_uses_source_line() {
        assert_eq!(rc_body(Path::new("/tmp/x.sh")), "source '/tmp/x.sh'");
    }
}
