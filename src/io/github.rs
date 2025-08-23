use lazy_static::lazy_static;

lazy_static! {
    static ref REGEX_GITHUB_TOKEN: regex::Regex = regex::Regex::new(r"^(gh[pousr]_[A-Za-z0-9_]{36,251}|github_pat_[a-zA-Z0-9]{22}_[a-zA-Z0-9]{59}|v[0-9]\.[0-9a-f]{40})$").unwrap();
}

pub fn is_valid_github_token(token: &str) -> bool {
    REGEX_GITHUB_TOKEN.is_match(token)
}

pub fn get_token_from_github_cli() -> Option<String> {
    std::process::Command::new("gh")
        .arg("auth")
        .arg("token")
        .output()
        .ok()
        .and_then(|output| output.status.success().then(|| output))
        .and_then(|output| {
            let token_str = String::from_utf8_lossy(&output.stdout);
            let token_str = token_str.trim();
            is_valid_github_token(token_str).then(|| token_str.to_string())
        })
}
