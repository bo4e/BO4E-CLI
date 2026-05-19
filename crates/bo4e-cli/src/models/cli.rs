use crate::cprint_normal;
use crate::io::github::{get_token_from_github_cli, is_valid_github_token};
use serde::{Deserialize, Serialize};
use std::fmt::Display;
use std::str::FromStr;

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct Token {
    pub token: String,
}

impl FromStr for Token {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Token::new(s.to_string())
    }
}

impl Display for Token {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.token)
    }
}

impl From<Token> for String {
    fn from(token: Token) -> Self {
        token.token
    }
}

impl From<&Token> for String {
    fn from(token: &Token) -> Self {
        token.token.clone()
    }
}

impl Token {
    pub fn new(token: String) -> Result<Self, String> {
        is_valid_github_token(&token)
            .then_some(Token { token })
            .ok_or_else(|| "Invalid GitHub token.".to_string())
    }
    pub fn from_github_cli() -> Result<Self, String> {
        get_token_from_github_cli()
            .map(|token| Token { token })
            .ok_or_else(|| "Could not retrieve GitHub token from GitHub CLI.".to_string())
    }
}

/// Same fallback chain as `get_token_as_string` but emits no console
/// output. Safe to call from completion mode where stdout is reserved.
pub fn resolve_token_silent(token: &Option<Token>) -> Option<String> {
    if let Some(t) = token {
        return Some(t.into());
    }
    Token::from_github_cli().ok().map(|t| t.into())
}

pub fn get_token_as_string(token: &Option<Token>) -> Option<String> {
    if let Some(t) = token {
        cprint_normal!("Using GitHub Access Token for authentication.");
        return Some(t.into());
    }
    if let Ok(t) = Token::from_github_cli() {
        cprint_normal!("Using GitHub Access Token from GitHub CLI for authentication.");
        return Some(t.into());
    }
    cprint_normal!(
        "No GitHub Access Token provided. \
         This may lead to rate limiting issues if you run this command multiple times."
    );
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::console::console::{CONSOLE, Console, Level};

    fn ensure_console() {
        let _ = CONSOLE.set(Console::new(Level::Quiet));
    }

    #[test]
    fn resolve_token_silent_uses_arg_when_present() {
        let arg = Token { token: "ghp_aaa".to_string() };
        assert_eq!(
            resolve_token_silent(&Some(arg)),
            Some("ghp_aaa".to_string()),
        );
    }

    #[test]
    fn resolve_token_silent_returns_none_when_arg_absent_and_no_gh() {
        ensure_console();
        // Test relies on `gh auth token` failing or not being installed.
        // If `gh` is logged in on the dev machine this test is skipped — guard
        // by checking `Token::from_github_cli()` first.
        if Token::from_github_cli().is_ok() {
            eprintln!("skipping: gh auth token is logged in on this machine");
            return;
        }
        assert_eq!(resolve_token_silent(&None), None);
    }
}
