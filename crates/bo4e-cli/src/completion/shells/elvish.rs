use clap::Command;
use clap_complete::env::Elvish;
use clap_complete::env::EnvCompleter as _;
use std::path::Path;

pub fn script(_cmd: &mut Command) -> String {
    let mut buf = Vec::new();
    Elvish
        .write_registration("COMPLETE", "bo4e", "bo4e", "bo4e", &mut buf)
        .expect("write_registration");
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
    fn script_emits_edit_completion() {
        let mut cmd = crate::cli::base::Cli::command();
        let s = script(&mut cmd);
        assert!(
            s.contains("edit:completion") || s.contains("set @"),
            "expected elvish completion hook: {}",
            &s[..200.min(s.len())]
        );
        assert!(s.contains("bo4e"));
    }

    #[test]
    fn rc_body_uses_module() {
        assert_eq!(rc_body(Path::new("/tmp/bo4e.elv")), "use bo4e");
    }
}
