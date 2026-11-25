//! Human-readable output. Commands build typed data (also what `--json` prints); the
//! printers here only format it.

pub mod human;
pub mod state;

/// ANSI styling, off unless colors are enabled (`--color`, NO_COLOR, tty).
#[derive(Debug, Clone, Copy)]
pub struct Style {
    on: bool,
}

impl Style {
    pub fn new(on: bool) -> Self {
        Self { on }
    }

    fn paint(&self, code: &str, s: &str) -> String {
        if self.on {
            format!("\x1b[{code}m{s}\x1b[0m")
        } else {
            s.to_string()
        }
    }

    pub fn bold(&self, s: &str) -> String {
        self.paint("1", s)
    }
    pub fn dim(&self, s: &str) -> String {
        self.paint("2", s)
    }
    pub fn green(&self, s: &str) -> String {
        self.paint("32", s)
    }
    pub fn yellow(&self, s: &str) -> String {
        self.paint("33", s)
    }
    pub fn red(&self, s: &str) -> String {
        self.paint("31", s)
    }
    pub fn cyan(&self, s: &str) -> String {
        self.paint("36", s)
    }
}

/// Left-aligned columns sized to the widest cell (ANSI-free cells only).
pub fn table(header: &[&str], rows: &[Vec<String>], st: &Style) {
    let n = header.len();
    let mut w: Vec<usize> = header.iter().map(|h| h.chars().count()).collect();
    for r in rows {
        for (i, c) in r.iter().enumerate().take(n) {
            w[i] = w[i].max(c.chars().count());
        }
    }
    let line = |cells: Vec<String>| {
        let mut s = String::new();
        for (i, c) in cells.iter().enumerate() {
            if i + 1 == cells.len() {
                s.push_str(c);
            } else {
                s.push_str(&format!("{c:<width$}  ", width = w[i]));
            }
        }
        s.trim_end().to_string()
    };
    println!(
        "{}",
        st.bold(&line(header.iter().map(|h| h.to_string()).collect()))
    );
    for r in rows {
        println!("{}", line(r.clone()));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn style_off_is_plain() {
        assert_eq!(Style::new(false).red("x"), "x");
        assert_eq!(Style::new(true).red("x"), "\x1b[31mx\x1b[0m");
    }
}
