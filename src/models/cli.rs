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
        Token::new(s.to_string()).or_else(|_| Token::from_github_cli())
    }
}

impl Display for Token {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.token)
    }
}

impl Token {
    pub fn new(token: String) -> Result<Self, String> {
        is_valid_github_token(&token)
            .then(|| Token { token })
            .ok_or_else(|| "Invalid GitHub token.".to_string())
    }
    pub fn from_github_cli() -> Result<Self, String> {
        get_token_from_github_cli()
            .map(|token| Token { token })
            .ok_or_else(|| "Could not retrieve GitHub token from GitHub CLI.".to_string())
    }
}
