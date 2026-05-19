use clap::Command;

pub fn script(cmd: &mut Command) -> String {
    let mut buf = Vec::new();
    clap_complete::generate(clap_complete::Shell::Fish, cmd, "bo4e", &mut buf);
    String::from_utf8(buf).expect("clap_complete output is valid UTF-8")
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::CommandFactory;

    #[test]
    fn script_contains_complete_command() {
        let mut cmd = crate::cli::base::Cli::command();
        let s = script(&mut cmd);
        assert!(s.contains("complete -c bo4e"));
    }
}
