use crate::completion::shells::Selected;
use clap::Command;
use std::io::{self, Write};

pub fn show(cmd: &mut Command, shell: Selected, out: &mut dyn Write) -> io::Result<()> {
    let script = shell.script(cmd);
    out.write_all(script.as_bytes())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cli::base::Cli;
    use clap::CommandFactory;

    fn render(shell: Selected) -> String {
        let mut cmd = Cli::command();
        let mut buf = Vec::new();
        show(&mut cmd, shell, &mut buf).unwrap();
        String::from_utf8(buf).unwrap()
    }

    #[test]
    fn show_renders_non_empty_for_all_six_shells() {
        for sh in [
            Selected::Bash,
            Selected::Zsh,
            Selected::Fish,
            Selected::Powershell,
            Selected::Elvish,
            Selected::Nushell,
        ] {
            let s = render(sh);
            assert!(!s.is_empty(), "shell {:?} returned empty script", sh);
        }
    }
}
