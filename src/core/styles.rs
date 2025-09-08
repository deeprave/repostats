//! Style roles expressed as an enum + macro mapping logical names to `colored::Color`.
//! This eliminates palette structs / instances while keeping a single source of truth.
//!
//! # Overview
//! Each logical style (Header, Key, etc.) is a variant of `StyleRole`. Coloring is applied
//! only when the `enabled` flag passed to `paint()` is true, avoiding global mutable state.
//!
//! # Basic Usage
//! ```
//! use repostats::core::styles::StyleRole;
//! let plain = StyleRole::Header.paint("Title", false);
//! assert_eq!(plain, "Title");
//! let colored = StyleRole::Header.paint("Title", true);
//! assert!(colored.starts_with("\x1b["));
//! assert!(colored.ends_with("\x1b[0m"));
//! ```
//!
//! # Extending (example of defining a local mini role set with the same macro pattern)
//! This is illustrative only; the crate already defines `StyleRole` globally. You normally
//! just add a variant to the existing invocation below.
//! ```
//! use colored::Color;
//! macro_rules! local_style { ( $( $v:ident => $c:expr ),+ $(,)? ) => {
//!     #[derive(Copy, Clone, Debug)] enum Local { $( $v ),+ }
//!     impl Local { fn color(self) -> Option<Color> { match self { $( Local::$v => $c ),+ } } }
//! } }
//! local_style! { Header => Some(Color::Yellow), Value => None }
//! assert!(Local::Header.color().is_some());
//! assert!(Local::Value.color().is_none());
//! ```

use clap::builder::styling::AnsiColor;
use colored::Color; // for clap help mapping

// Macro defines the enum variants and their associated colour Option.
macro_rules! style {
    ( $( $variant:ident => $color:expr ),+ $(,)? ) => {
        #[derive(Copy, Clone, Debug)]
        pub enum StyleRole { $( $variant ),+ }

        impl StyleRole {
            pub fn color(self) -> Option<Color> {
                match self { $( StyleRole::$variant => $color ),+ }
            }

            pub fn ansi_code(self) -> Option<String> {
                map_color_code(self.color()?)
            }

            pub fn paint(self, text: &str, enabled: bool) -> String {
                if !enabled { return text.to_string(); }
                if let Some(code) = self.ansi_code() { return format!("\x1b[{}m{}\x1b[0m", code, text); }
                text.to_string()
            }

            /// Convert StyleRole to prettytable style_spec format
            pub fn to_prettytable_spec(self) -> Option<String> {
                let color = self.color()?;

                let spec_char = match color {
                    Color::Black => "k",
                    Color::Red => "r",
                    Color::Green => "g",
                    Color::Yellow => "y",
                    Color::Blue => "b",
                    Color::Magenta => "m",
                    Color::Cyan => "c",
                    Color::White => "w",
                    Color::BrightBlack => "K",
                    Color::BrightRed => "R",
                    Color::BrightGreen => "G",
                    Color::BrightYellow => "Y",
                    Color::BrightBlue => "B",
                    Color::BrightMagenta => "M",
                    Color::BrightCyan => "C",
                    Color::BrightWhite => "W",
                    _ => return None,
                };

                Some(format!("F{}", spec_char)) // Foreground color
            }
        }
    }
}

// Define all logical roles. Value => None (uncoloured)
style! {
    Header      => Some(Color::Yellow),
    Literal     => Some(Color::Cyan),
    Placeholder => Some(Color::Green),
    Valid       => Some(Color::Green),
    Invalid     => Some(Color::Red),
    Error       => Some(Color::BrightRed),
    Key         => Some(Color::BrightGreen),
    Value       => None,
    Accent      => Some(Color::Blue),
    Dim         => Some(Color::BrightBlack)
}

fn map_color_code(c: Color) -> Option<String> {
    use Color::*;
    match c {
        Black => Some("30".to_string()),
        Red => Some("31".to_string()),
        Green => Some("32".to_string()),
        Yellow => Some("33".to_string()),
        Blue => Some("34".to_string()),
        Magenta => Some("35".to_string()),
        Cyan => Some("36".to_string()),
        White => Some("37".to_string()),
        BrightBlack => Some("90".to_string()),
        BrightRed => Some("91".to_string()),
        BrightGreen => Some("92".to_string()),
        BrightYellow => Some("93".to_string()),
        BrightBlue => Some("94".to_string()),
        BrightMagenta => Some("95".to_string()),
        BrightCyan => Some("96".to_string()),
        BrightWhite => Some("97".to_string()),
        TrueColor { r, g, b } => {
            // ANSI TrueColor format: 38;2;R;G;B for foreground text
            Some(format!("38;2;{};{};{}", r, g, b))
        }
    }
}

fn color_to_ansi(c: Color) -> Option<AnsiColor> {
    use AnsiColor as A;
    use Color::*;
    Some(match c {
        Black => A::Black,
        Red => A::Red,
        Green => A::Green,
        Yellow => A::Yellow,
        Blue => A::Blue,
        Magenta => A::Magenta,
        Cyan => A::Cyan,
        White => A::White,
        BrightBlack => A::BrightBlack,
        BrightRed => A::BrightRed,
        BrightGreen => A::BrightGreen,
        BrightYellow => A::BrightYellow,
        BrightBlue => A::BrightBlue,
        BrightMagenta => A::BrightMagenta,
        BrightCyan => A::BrightCyan,
        BrightWhite => A::BrightWhite,
        _ => return None,
    })
}

/// Build clap Styles for help output using enum roles (no external palette arg).
pub fn palette_to_clap(enabled: bool) -> clap::builder::Styles {
    use clap::builder::styling::{Color as ClapColor, Style};
    if !enabled {
        return clap::builder::Styles::plain();
    }

    let style = |role: StyleRole, bold: bool| {
        let mut s = Style::new();
        if let Some(col) = role.color().and_then(color_to_ansi) {
            s = s.fg_color(Some(ClapColor::Ansi(col)));
        }
        if bold {
            s = s.bold();
        }
        s
    };

    clap::builder::Styles::styled()
        .header(style(StyleRole::Header, true))
        .literal(style(StyleRole::Literal, false))
        .placeholder(style(StyleRole::Placeholder, false))
        .valid(style(StyleRole::Valid, false))
        .invalid(style(StyleRole::Invalid, false))
        .error(style(StyleRole::Error, false))
}

/// Apply palette to table header (basic usage).
// (Pruned auxiliary helpers; will reintroduce minimal ones as needed later.)

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ansi_code_header() {
        assert_eq!(StyleRole::Header.ansi_code(), Some("33".to_string()));
    }

    #[test]
    fn paint_enabled_disabled() {
        let txt = "Hello";
        let colored = StyleRole::Header.paint(txt, true);
        assert!(colored.starts_with("\x1b[33m") && colored.ends_with("\x1b[0m"));
        assert_eq!(StyleRole::Header.paint(txt, false), txt);
    }

    #[test]
    fn truecolor_support() {
        use colored::Color::TrueColor;

        // Test TrueColor ANSI code generation
        let truecolor = TrueColor {
            r: 255,
            g: 128,
            b: 64,
        };
        let code = map_color_code(truecolor);

        assert_eq!(code, Some("38;2;255;128;64".to_string()));

        // Test that it formats properly in full ANSI escape sequence
        let expected_format = "\x1b[38;2;255;128;64mHello\x1b[0m";
        // We can't test this directly with StyleRole since that uses predefined colors,
        // but we can verify the ANSI code is correct
        let full_escape = format!("\x1b[{}mHello\x1b[0m", code.unwrap());
        assert_eq!(full_escape, expected_format);
    }

    #[test]
    fn palette_to_clap_differs_when_enabled() {
        let plain_dbg = format!("{:?}", palette_to_clap(false));
        let styled_dbg = format!("{:?}", palette_to_clap(true));
        assert_ne!(
            plain_dbg, styled_dbg,
            "Expected styled vs plain clap styles to differ"
        );
    }

    #[test]
    fn touch_all_variants() {
        // Ensure seldom-used variants are constructed at least once to avoid dead_code warnings.
        let _ = [StyleRole::Value, StyleRole::Accent, StyleRole::Dim];
    }
}
