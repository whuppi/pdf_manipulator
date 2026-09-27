use std::sync::OnceLock;

// Brand colors (oxide.fyi)
const RUST_ORANGE: &str = "\x1b[38;2;255;122;69m"; // #FF7A45
const RUST_DARK: &str = "\x1b[38;2;212;84;28m"; // #D4541C
const RUST_DEEP: &str = "\x1b[38;2;194;71;23m"; // #C24717
const WHITE: &str = "\x1b[38;2;245;245;247m"; // #f5f5f7
const RESET: &str = "\x1b[0m";
const DIM: &str = "\x1b[2m";
const BOLD: &str = "\x1b[1m";

// Basic ANSI fallbacks
const BASIC_ORANGE: &str = "\x1b[33m"; // yellow (closest to orange)
const BASIC_WHITE: &str = "\x1b[37m";
const BASIC_RED: &str = "\x1b[31m";

static USE_COLORS: OnceLock<bool> = OnceLock::new();
static USE_TRUECOLOR: OnceLock<bool> = OnceLock::new();

pub fn use_colors() -> bool {
    *USE_COLORS.get_or_init(|| {
        if std::env::var("NO_COLOR").is_ok() {
            return false;
        }
        if std::env::var("CI").is_ok() {
            return false;
        }
        is_terminal::is_terminal(std::io::stderr())
    })
}

fn use_truecolor() -> bool {
    *USE_TRUECOLOR.get_or_init(|| {
        if let Ok(colorterm) = std::env::var("COLORTERM") {
            colorterm == "truecolor" || colorterm == "24bit"
        } else {
            // Most modern terminals support truecolor even without COLORTERM
            true
        }
    })
}

pub fn rust_orange(text: &str) -> String {
    if !use_colors() {
        return text.to_string();
    }
    if use_truecolor() {
        format!("{RUST_ORANGE}{text}{RESET}")
    } else {
        format!("{BASIC_ORANGE}{text}{RESET}")
    }
}

pub fn rust_dark(text: &str) -> String {
    if !use_colors() {
        return text.to_string();
    }
    if use_truecolor() {
        format!("{RUST_DARK}{text}{RESET}")
    } else {
        format!("{BASIC_RED}{text}{RESET}")
    }
}

pub fn rust_deep(text: &str) -> String {
    if !use_colors() {
        return text.to_string();
    }
    if use_truecolor() {
        format!("{RUST_DEEP}{text}{RESET}")
    } else {
        format!("{BASIC_RED}{text}{RESET}")
    }
}

pub fn white(text: &str) -> String {
    if !use_colors() {
        return text.to_string();
    }
    if use_truecolor() {
        format!("{WHITE}{text}{RESET}")
    } else {
        format!("{BASIC_WHITE}{text}{RESET}")
    }
}

pub fn dim(text: &str) -> String {
    if !use_colors() {
        return text.to_string();
    }
    format!("{DIM}{text}{RESET}")
}

pub fn bold(text: &str) -> String {
    if !use_colors() {
        return text.to_string();
    }
    format!("{BOLD}{text}{RESET}")
}

pub fn error(text: &str) -> String {
    if !use_colors() {
        return text.to_string();
    }
    if use_truecolor() {
        format!("{RUST_DEEP}{BOLD}{text}{RESET}")
    } else {
        format!("{BASIC_RED}{BOLD}{text}{RESET}")
    }
}
