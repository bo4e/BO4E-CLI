use color_eyre::owo_colors::OwoColorize;
use console::Style;
use regex::Regex;
use std::collections::HashMap;

pub trait Highlight {
    fn highlight(&self, highlighter: Highlighter, styles: HashMap<&'static str, Style>) -> String;
}

impl Highlight for String {
    fn highlight(&self, highlighter: Highlighter, styles: HashMap<&'static str, Style>) -> String {
        format!("\x1b[1;34m{}\x1b[0m", self) // Example: Blue bold text
    }
}

pub struct Highlighter {
    regexes: Vec<Regex>,
}

impl Highlighter {
    pub fn new(regexes: Vec<Regex>) -> Self {
        Highlighter { regexes }
    }

    pub fn highlight_text(&self, text: &str, styles: &HashMap<&'static str, Style>) -> String {
        let mut highlighted = String::with_capacity(3 * text.len());
        let mut start_idx = 0;
        for regex in &self.regexes {
            for caps in regex.captures_iter(text) {
                let mut caps_iter = caps.iter().zip(regex.capture_names());
                let root_match = caps_iter.next().unwrap().0.unwrap();
                let (root_match_start, root_match_end) = (root_match.start(), root_match.end());
                highlighted.push_str(&text[start_idx..root_match_start]);
                for (cap_match, cap) in caps_iter {}
                start_idx = root_match_end;
            }
        }
        highlighted
    }
}

impl TryFrom<Vec<&str>> for Highlighter {
    type Error = regex::Error;

    fn try_from(patterns: Vec<&str>) -> Result<Self, Self::Error> {
        let regexes = patterns
            .into_iter()
            .map(|p| Regex::new(p))
            .collect::<Result<Vec<Regex>, regex::Error>>()?;
        Ok(Highlighter::new(regexes))
    }
}
