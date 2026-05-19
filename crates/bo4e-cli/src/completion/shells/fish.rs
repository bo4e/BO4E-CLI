use clap::Command;
use clap_complete::env::EnvCompleter as _;
use clap_complete::env::Fish;

pub fn script(_cmd: &mut Command) -> String {
    let mut buf = Vec::new();
    Fish.write_registration("COMPLETE", "bo4e", "bo4e", "bo4e", &mut buf)
        .expect("write_registration");
    String::from_utf8(buf).expect("clap_complete output is valid UTF-8")
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::CommandFactory;

    #[test]
    fn script_calls_complete() {
        let mut cmd = crate::cli::base::Cli::command();
        let s = script(&mut cmd);
        assert!(s.contains("complete"), "expected `complete` invocation");
        assert!(s.contains("bo4e"));
        assert!(s.contains("COMPLETE"));
    }
}
