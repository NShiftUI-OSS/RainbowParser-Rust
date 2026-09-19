use std::env;
use std::io::{self, IsTerminal};

#[derive(Clone, Copy, Debug)]
pub struct Style {
    color: bool,
}

impl Style {
    pub fn stdout() -> Self {
        Self {
            color: colors_enabled(io::stdout()),
        }
    }

    pub fn stderr() -> Self {
        Self {
            color: colors_enabled(io::stderr()),
        }
    }

    pub fn paint(self, code: &str, text: &str) -> String {
        if self.color {
            format!("\x1b[{code}m{text}\x1b[0m")
        } else {
            text.to_string()
        }
    }

    pub fn bold(self, text: &str) -> String {
        self.paint("1", text)
    }

    pub fn dim(self, text: &str) -> String {
        self.paint("2", text)
    }

    pub fn green(self, text: &str) -> String {
        self.paint("32", text)
    }

    pub fn red(self, text: &str) -> String {
        self.paint("31", text)
    }

    pub fn yellow(self, text: &str) -> String {
        self.paint("33", text)
    }

    pub fn cyan(self, text: &str) -> String {
        self.paint("36", text)
    }

    pub fn ok_icon(self) -> String {
        self.green("✓")
    }

    pub fn fail_icon(self) -> String {
        self.red("✗")
    }

    pub fn skip_icon(self) -> String {
        self.dim("○")
    }

    pub fn warn_icon(self) -> String {
        self.yellow("!")
    }

    pub fn arrow(self) -> String {
        self.cyan("→")
    }

    pub fn header(self, title: &str) -> String {
        self.bold(title)
    }

    pub fn ok_line(self, message: &str) -> String {
        format!("{}  {message}", self.ok_icon())
    }

    pub fn fail_line(self, message: &str) -> String {
        format!("{}  {message}", self.fail_icon())
    }

    pub fn warn_line(self, message: &str) -> String {
        format!("{}  {message}", self.warn_icon())
    }

    pub fn skip_line(self, message: &str) -> String {
        format!("{}  {message}", self.skip_icon())
    }

    pub fn step(self, current: usize, total: usize, message: &str) -> String {
        let label = self.dim(&format!("[{current}/{total}]"));
        format!("{label} {message}")
    }

    pub fn step_done(self, current: usize, total: usize, message: &str) -> String {
        format!(
            "{} {} {}",
            self.dim(&format!("[{current}/{total}]")),
            self.ok_icon(),
            message
        )
    }

    pub fn indent(self, message: &str) -> String {
        format!("   {message}")
    }

    pub fn path(self, path: &str) -> String {
        self.cyan(path)
    }
}

fn colors_enabled(stream: impl IsTerminal) -> bool {
    if env::var_os("NO_COLOR").is_some() {
        return false;
    }
    if env::var_os("RAINBOW_FORCE_COLOR").is_some() || env::var_os("FORCE_COLOR").is_some() {
        return true;
    }
    stream.is_terminal()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn paint_without_color_is_plain() {
        let style = Style { color: false };
        assert_eq!(style.green("ok"), "ok");
        assert_eq!(style.ok_line("ready"), "✓  ready");
    }

    #[test]
    fn paint_with_color_wraps_ansi() {
        let style = Style { color: true };
        assert_eq!(style.green("ok"), "\x1b[32mok\x1b[0m");
    }
}
