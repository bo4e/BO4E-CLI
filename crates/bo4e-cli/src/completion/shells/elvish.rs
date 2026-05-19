use clap::Command;
use std::path::Path;

pub fn script(cmd: &mut Command) -> String {
    let mut buf = Vec::new();
    clap_complete::generate(clap_complete::Shell::Elvish, cmd, "bo4e", &mut buf);
    String::from_utf8(buf).expect("clap_complete output is valid UTF-8")
}

pub fn rc_body(script: &Path) -> String {
    let stem = script
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("bo4e");
    format!("use {}", stem)
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::CommandFactory;

    #[test]
    fn script_contains_edit_completion() {
        let mut cmd = crate::cli::base::Cli::command();
        let s = script(&mut cmd);
        assert!(s.contains("edit:completion:arg-completer"));
    }

    #[test]
    fn rc_body_uses_module() {
        assert_eq!(rc_body(Path::new("/tmp/bo4e.elv")), "use bo4e");
    }
}
