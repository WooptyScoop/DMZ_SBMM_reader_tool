//! Shared visual language for the UI.
//!
//! Quiet dark surfaces, hairline borders and one restrained accent, in the
//! spirit of Hex's settings panes. Kept separate from `app` so the view code
//! stays about layout and behaviour rather than colour literals.

use rune::{Color, Icon, col, el, oklch, oklcha, text};
use rune::tok::{TextStyle, type_};

/// All colours used by the app, in one place.
pub struct Palette {
    pub bg: Color,
    pub sidebar: Color,
    pub card: Color,
    pub well: Color,
    pub border: Color,
    pub border_strong: Color,
    pub text: Color,
    pub secondary: Color,
    pub muted: Color,
    pub hover: Color,
    pub selected: Color,
    pub transparent: Color,
    pub accent: Color,
    pub accent_soft: Color,
    pub accent_text: Color,
    pub green: Color,
    pub blue: Color,
    pub yellow: Color,
    pub red: Color,
}

pub fn dark() -> Palette {
    Palette {
        bg: oklch(0.145, 0.003, 265.0),
        sidebar: oklch(0.145, 0.003, 265.0),
        card: oklch(0.188, 0.003, 265.0),
        well: oklch(0.145, 0.003, 265.0),
        border: oklcha(1.0, 0.0, 0.0, 0.08),
        border_strong: oklcha(1.0, 0.0, 0.0, 0.14),
        text: oklch(0.955, 0.003, 265.0),
        secondary: oklch(0.73, 0.006, 265.0),
        muted: oklch(0.56, 0.006, 265.0),
        hover: oklcha(1.0, 0.0, 0.0, 0.045),
        selected: oklcha(1.0, 0.0, 0.0, 0.075),
        transparent: oklcha(1.0, 0.0, 0.0, 0.0),
        accent: oklch(0.84, 0.17, 126.0),
        accent_soft: oklcha(0.84, 0.17, 126.0, 0.13),
        accent_text: oklch(0.17, 0.02, 126.0),
        green: oklch(0.74, 0.13, 150.0),
        blue: oklch(0.72, 0.12, 250.0),
        yellow: oklch(0.80, 0.12, 90.0),
        red: oklch(0.69, 0.15, 25.0),
    }
}

pub const CAPTION: TextStyle = TextStyle::new(11.0, 500.0);
pub const SECTION: TextStyle = TextStyle::new(12.0, 600.0);
pub const PAGE_TITLE: TextStyle = TextStyle::new(20.0, 600.0);
pub const MONO: TextStyle = TextStyle::new(11.5, 400.0);

/// Small round status light.
pub fn dot(color: Color, size: f32) -> rune::El {
    el().key("dot").size(size, size).radius(size / 2.0).fill(color)
}

/// One-pixel hairline, used between panes and above footers.
pub fn hairline(p: &Palette) -> rune::El {
    el().key("hairline").h(1.0).w_full().fill(p.border)
}

/// Muted one-line hint beneath a control.
pub fn hint(s: &str, p: &Palette) -> rune::El {
    text(s.to_string()).font(CAPTION).color(p.muted).wrap()
}

/// Uppercase section label, e.g. `FOLDERS TO SCAN`.
pub fn eyebrow(s: &str, p: &Palette) -> rune::El {
    text(s.to_string()).font(CAPTION.tracking(0.8)).color(p.muted)
}

/// Title plus supporting line at the top of a page.
pub fn page_header(title: &str, subtitle: &str, p: &Palette) -> rune::El {
    col().key("page-header").gap(4.0)
        .child(text(title.to_string()).font(PAGE_TITLE).color(p.text))
        .child(text(subtitle.to_string()).font(type_::BODY).color(p.secondary).wrap())
}

/// A bordered, rounded surface. Callers add the content.
pub fn card(key: &str, p: &Palette) -> rune::El {
    el().key(key.to_string())
        .w_full()
        .radius(10.0)
        .fill(p.card)
        .border(1.0, p.border)
}

/// A recessed area for logs and text results.
pub fn well(key: &str, p: &Palette) -> rune::El {
    el().key(key.to_string())
        .radius(8.0)
        .fill(p.well)
        .border(1.0, p.border)
}

/// Icons. Bodies are SVG fragments inside a 16x16 viewBox; `currentColor`
/// picks up the colour of the element they are attached to.
pub mod icons {
    use super::Icon;

    pub static SCAN: Icon = Icon {
        name: "scan",
        view: 16.0,
        body: r#"<path d="M11.5 7C11.5 9.48528 9.48528 11.5 7 11.5C4.51472 11.5 2.5 9.48528 2.5 7C2.5 4.51472 4.51472 2.5 7 2.5C9.48528 2.5 11.5 4.51472 11.5 7Z" stroke="currentColor"/><path d="M13.5 13.5L10.5 10.5" stroke="currentColor" stroke-linecap="round"/>"#,
    };

    pub static PROBE: Icon = Icon {
        name: "probe",
        view: 16.0,
        body: r#"<path d="M1.5 8H4L6 3.5L9 12.5L11 8H14.5" stroke="currentColor" stroke-linecap="round" stroke-linejoin="round"/>"#,
    };

    pub static FOLDER: Icon = Icon {
        name: "folder",
        view: 16.0,
        body: r#"<path d="M1.5 13.5V2.5H5.5L8.5 5.5H14.5V13.5H1.5Z" stroke="currentColor" stroke-linecap="round" stroke-linejoin="round"/>"#,
    };

    pub static DETECT: Icon = Icon {
        name: "detect",
        view: 16.0,
        body: r#"<path d="M8 1.5V3.5M8 12.5V14.5M1.5 8H3.5M12.5 8H14.5" stroke="currentColor" stroke-linecap="round"/><circle cx="8" cy="8" r="4" stroke="currentColor"/>"#,
    };

    pub static PLAY: Icon = Icon {
        name: "play",
        view: 16.0,
        body: r#"<path d="M5 3.5L12 8L5 12.5V3.5Z" fill="currentColor"/>"#,
    };

    pub static STOP: Icon = Icon {
        name: "stop",
        view: 16.0,
        body: r#"<path d="M4 4H12V12H4V4Z" fill="currentColor"/>"#,
    };

    pub static EXPORT: Icon = Icon {
        name: "export",
        view: 16.0,
        body: r#"<path d="M8 2.5V10M5 7.5L8 10.5L11 7.5M2.5 13H13.5" stroke="currentColor" stroke-linecap="round" stroke-linejoin="round"/>"#,
    };

    pub static SHIELD: Icon = Icon {
        name: "shield",
        view: 16.0,
        body: r#"<path d="M8 1.5L13.5 3.5V8C13.5 11.2 11.1 13.4 8 14.5C4.9 13.4 2.5 11.2 2.5 8V3.5L8 1.5Z" stroke="currentColor" stroke-linejoin="round"/><path d="M5.8 8L7.3 9.5L10.3 6.5" stroke="currentColor" stroke-linecap="round" stroke-linejoin="round"/>"#,
    };

    pub static LIST: Icon = Icon {
        name: "list",
        view: 16.0,
        body: r#"<path d="M6 4.5H14M6 8H14M6 11.5H14" stroke="currentColor" stroke-linecap="round"/><circle cx="2.5" cy="4.5" r="0.8" fill="currentColor"/><circle cx="2.5" cy="8" r="0.8" fill="currentColor"/><circle cx="2.5" cy="11.5" r="0.8" fill="currentColor"/>"#,
    };

    pub static CHEVRON: Icon = Icon {
        name: "chevron",
        view: 16.0,
        body: r#"<path d="M6.5 4.5L10 8L6.5 11.5" stroke="currentColor" stroke-linecap="round" stroke-linejoin="round"/>"#,
    };
}
