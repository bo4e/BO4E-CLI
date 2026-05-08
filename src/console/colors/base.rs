use crossterm::style::Color;

pub struct ColorWithFallback {
    color: Color,
    fallback: Color,
}

impl ColorWithFallback {
    pub fn new(color: Color, fallback: Color) -> Result<Self, String> {
        // Ensure that `color` is Color::Rgb and `fallback` is not
        if !matches!(color, Color::Rgb { .. }) {
            return Err("The `color` must be a true color (Color::Rgb).".to_string());
        }
        if matches!(fallback, Color::Rgb { .. }) {
            return Err("The `fallback` must not be a true color.".to_string());
        }
        Ok(Self { color, fallback })
    }

    pub fn supports_truecolor() -> bool {
        // Check the environment variable "COLORTERM" for "truecolor" or "24bit"
        std::env::var("COLORTERM").map_or(false, |colorterm| {
            colorterm.eq_ignore_ascii_case("truecolor") || colorterm.eq_ignore_ascii_case("24bit")
        })
    }

    pub fn get(&self) -> Option<&Color> {
        if atty::is(atty::Stream::Stdout) {
            if Self::supports_truecolor() {
                Some(&self.color)
            } else {
                Some(&self.fallback)
            }
        } else {
            None
        }
    }
}
