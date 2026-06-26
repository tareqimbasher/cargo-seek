//! Configuration — defaults embedded from `.config/config.json5` at build time, merged with the
//! user's config directory. Keybindings and styles deserialize from the merged config.

#![allow(dead_code)]

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use derive_deref::{Deref, DerefMut};
use directories::ProjectDirs;
use ratatui::style::{Color, Modifier, Style};
use serde::{Deserialize, de::Deserializer};
use std::sync::LazyLock;
use std::{collections::HashMap, env, path::PathBuf};
use tracing::error;

use crate::{action::Action, app::Mode};

const CONFIG: &str = include_str!("../.config/config.json5");

#[derive(Clone, Debug, Deserialize, Default)]
pub struct AppConfig {
    #[serde(default)]
    pub data_dir: PathBuf,
    #[serde(default)]
    pub config_dir: PathBuf,
}

#[derive(Clone, Debug, Default, Deref, DerefMut)]
pub struct KeyBindings(pub HashMap<Mode, HashMap<Vec<KeyEvent>, Action>>);

impl<'de> Deserialize<'de> for KeyBindings {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let parsed_map = HashMap::<Mode, HashMap<String, Action>>::deserialize(deserializer)?;

        let mut keybindings = HashMap::new();
        for (mode, inner_map) in parsed_map {
            let mut converted_inner_map = HashMap::new();
            for (key_str, cmd) in inner_map {
                // Surface a malformed keybinding as a config error instead of panicking.
                let keys = parse_key_sequence(&key_str).map_err(serde::de::Error::custom)?;
                converted_inner_map.insert(keys, cmd);
            }
            keybindings.insert(mode, converted_inner_map);
        }

        Ok(KeyBindings(keybindings))
    }
}

#[derive(Clone, Debug, Default)]
pub struct Config {
    pub config: AppConfig,
    pub keybindings: KeyBindings,
    pub theme: Theme,
}

/// Config exactly as parsed from disk, before the embedded defaults are merged in.
#[derive(Default, Deserialize)]
struct RawConfig {
    #[serde(default, flatten)]
    config: AppConfig,
    #[serde(default)]
    keybindings: KeyBindings,
    #[serde(default)]
    styles: ThemeConfig,
}

pub static PROJECT_NAME: LazyLock<String> =
    LazyLock::new(|| env!("CARGO_CRATE_NAME").to_uppercase().to_string());
static DATA_FOLDER: LazyLock<Option<PathBuf>> = LazyLock::new(|| {
    env::var(format!("{}_DATA", PROJECT_NAME.clone()))
        .ok()
        .map(PathBuf::from)
});
static CONFIG_FOLDER: LazyLock<Option<PathBuf>> = LazyLock::new(|| {
    env::var(format!("{}_CONFIG", PROJECT_NAME.clone()))
        .ok()
        .map(PathBuf::from)
});

impl Config {
    pub fn new() -> Result<Self, config::ConfigError> {
        let default_config: RawConfig = json5::from_str(CONFIG)
            .expect("the embedded default config (.config/config.json5) must be valid JSON5");
        let data_dir = get_data_dir();
        let config_dir = get_config_dir();
        let mut builder = config::Config::builder()
            .set_default("data_dir", data_dir.to_string_lossy().to_string())?
            .set_default("config_dir", config_dir.to_string_lossy().to_string())?;

        let config_files = [
            ("config.json5", config::FileFormat::Json5),
            ("config.json", config::FileFormat::Json),
            ("config.yaml", config::FileFormat::Yaml),
            ("config.toml", config::FileFormat::Toml),
            ("config.ini", config::FileFormat::Ini),
        ];
        let mut found_config = false;
        for (file, format) in &config_files {
            let source = config::File::from(config_dir.join(file))
                .format(*format)
                .required(false);
            builder = builder.add_source(source);
            if config_dir.join(file).exists() {
                found_config = true
            }
        }
        if !found_config {
            error!("No configuration file found. Application may not behave as expected");
        }

        let mut cfg: RawConfig = builder.build()?.try_deserialize()?;

        for (mode, default_bindings) in default_config.keybindings.iter() {
            let user_bindings = cfg.keybindings.entry(*mode).or_default();
            for (key, cmd) in default_bindings.iter() {
                user_bindings
                    .entry(key.clone())
                    .or_insert_with(|| cmd.clone());
            }
        }

        Ok(Config {
            config: cfg.config,
            theme: cfg.styles.resolve(&default_config.styles),
            keybindings: cfg.keybindings,
        })
    }
}

pub fn get_data_dir() -> PathBuf {
    if let Some(s) = DATA_FOLDER.clone() {
        s
    } else if let Some(proj_dirs) = project_directory() {
        proj_dirs.data_local_dir().to_path_buf()
    } else {
        PathBuf::from(".").join(".data")
    }
}

pub fn get_config_dir() -> PathBuf {
    if let Some(s) = CONFIG_FOLDER.clone() {
        s
    } else if let Some(proj_dirs) = project_directory() {
        proj_dirs.config_local_dir().to_path_buf()
    } else {
        PathBuf::from(".").join(".config")
    }
}

fn project_directory() -> Option<ProjectDirs> {
    ProjectDirs::from("com", "cargo-seek", env!("CARGO_PKG_NAME"))
}

fn parse_key_event(raw: &str) -> Result<KeyEvent, String> {
    let raw_lower = raw.to_ascii_lowercase();
    let (remaining, modifiers) = extract_modifiers(&raw_lower);
    parse_key_code_with_modifiers(remaining, modifiers)
}

fn extract_modifiers(raw: &str) -> (&str, KeyModifiers) {
    let mut modifiers = KeyModifiers::empty();
    let mut current = raw;

    loop {
        match current {
            rest if rest.starts_with("ctrl-") => {
                modifiers.insert(KeyModifiers::CONTROL);
                current = &rest[5..];
            }
            rest if rest.starts_with("alt-") => {
                modifiers.insert(KeyModifiers::ALT);
                current = &rest[4..];
            }
            rest if rest.starts_with("shift-") => {
                modifiers.insert(KeyModifiers::SHIFT);
                current = &rest[6..];
            }
            _ => break, // break out of the loop if no known prefix is detected
        };
    }

    (current, modifiers)
}

fn parse_key_code_with_modifiers(
    raw: &str,
    mut modifiers: KeyModifiers,
) -> Result<KeyEvent, String> {
    let c = match raw {
        "esc" => KeyCode::Esc,
        "enter" => KeyCode::Enter,
        "left" => KeyCode::Left,
        "right" => KeyCode::Right,
        "up" => KeyCode::Up,
        "down" => KeyCode::Down,
        "home" => KeyCode::Home,
        "end" => KeyCode::End,
        "pageup" => KeyCode::PageUp,
        "pagedown" => KeyCode::PageDown,
        "backtab" => {
            modifiers.insert(KeyModifiers::SHIFT);
            KeyCode::BackTab
        }
        "backspace" => KeyCode::Backspace,
        "delete" => KeyCode::Delete,
        "insert" => KeyCode::Insert,
        "f1" => KeyCode::F(1),
        "f2" => KeyCode::F(2),
        "f3" => KeyCode::F(3),
        "f4" => KeyCode::F(4),
        "f5" => KeyCode::F(5),
        "f6" => KeyCode::F(6),
        "f7" => KeyCode::F(7),
        "f8" => KeyCode::F(8),
        "f9" => KeyCode::F(9),
        "f10" => KeyCode::F(10),
        "f11" => KeyCode::F(11),
        "f12" => KeyCode::F(12),
        "space" => KeyCode::Char(' '),
        "hyphen" => KeyCode::Char('-'),
        "minus" => KeyCode::Char('-'),
        "tab" => KeyCode::Tab,
        c if c.len() == 1 => {
            let mut c = c.chars().next().unwrap();
            if modifiers.contains(KeyModifiers::SHIFT) {
                c = c.to_ascii_uppercase();
            }
            KeyCode::Char(c)
        }
        _ => return Err(format!("Unable to parse {raw}")),
    };
    Ok(KeyEvent::new(c, modifiers))
}

fn key_event_to_string(key_event: &KeyEvent) -> String {
    let char;
    let key_code = match key_event.code {
        KeyCode::Backspace => "backspace",
        KeyCode::Enter => "enter",
        KeyCode::Left => "left",
        KeyCode::Right => "right",
        KeyCode::Up => "up",
        KeyCode::Down => "down",
        KeyCode::Home => "home",
        KeyCode::End => "end",
        KeyCode::PageUp => "pageup",
        KeyCode::PageDown => "pagedown",
        KeyCode::Tab => "tab",
        KeyCode::BackTab => "backtab",
        KeyCode::Delete => "delete",
        KeyCode::Insert => "insert",
        KeyCode::F(c) => {
            char = format!("f({c})");
            &char
        }
        KeyCode::Char(' ') => "space",
        KeyCode::Char(c) => {
            char = c.to_string();
            &char
        }
        KeyCode::Esc => "esc",
        KeyCode::Null => "",
        KeyCode::CapsLock => "",
        KeyCode::Menu => "",
        KeyCode::ScrollLock => "",
        KeyCode::Media(_) => "",
        KeyCode::NumLock => "",
        KeyCode::PrintScreen => "",
        KeyCode::Pause => "",
        KeyCode::KeypadBegin => "",
        KeyCode::Modifier(_) => "",
    };

    let mut modifiers = Vec::with_capacity(3);

    if key_event.modifiers.intersects(KeyModifiers::CONTROL) {
        modifiers.push("ctrl");
    }

    if key_event.modifiers.intersects(KeyModifiers::SHIFT) {
        modifiers.push("shift");
    }

    if key_event.modifiers.intersects(KeyModifiers::ALT) {
        modifiers.push("alt");
    }

    let mut key = modifiers.join("-");

    if !key.is_empty() {
        key.push('-');
    }
    key.push_str(key_code);

    key
}

fn parse_key_sequence(raw: &str) -> Result<Vec<KeyEvent>, String> {
    if raw.chars().filter(|c| *c == '>').count() != raw.chars().filter(|c| *c == '<').count() {
        return Err(format!("Unable to parse `{raw}`"));
    }
    let raw = if !raw.contains("><") {
        let raw = raw.strip_prefix('<').unwrap_or(raw);
        raw.strip_prefix('>').unwrap_or(raw)
    } else {
        raw
    };
    let sequences = raw
        .split("><")
        .map(|seq| {
            if let Some(s) = seq.strip_prefix('<') {
                s
            } else if let Some(s) = seq.strip_suffix('>') {
                s
            } else {
                seq
            }
        })
        .collect::<Vec<_>>();

    sequences.into_iter().map(parse_key_event).collect()
}

/// The effective theme used by render code: the user's configured styles layered over the embedded
/// defaults (see [`ThemeConfig::resolve`]).
#[derive(Clone, Copy, Debug, Default)]
pub struct Theme {
    pub accent: Style,
    pub accent_active: Style,
    pub title: Style,
    pub throbber: Style,
}

/// A theme as written in a config file: each field is an optional style string (e.g. `"bold
/// lightyellow"`). Unset fields fall back to the embedded defaults when resolved into a [`Theme`].
#[derive(Default, Deserialize)]
#[serde(default)]
struct ThemeConfig {
    accent: Option<String>,
    accent_active: Option<String>,
    title: Option<String>,
    throbber: Option<String>,
}

impl ThemeConfig {
    /// Resolve into a [`Theme`]: each field is the user's value if set, otherwise `fallback`'s.
    fn resolve(self, fallback: &ThemeConfig) -> Theme {
        let pick = |user: Option<String>, default: &Option<String>| {
            parse_style(user.as_deref().or(default.as_deref()).unwrap_or_default())
        };
        Theme {
            accent: pick(self.accent, &fallback.accent),
            accent_active: pick(self.accent_active, &fallback.accent_active),
            title: pick(self.title, &fallback.title),
            throbber: pick(self.throbber, &fallback.throbber),
        }
    }
}

fn parse_style(line: &str) -> Style {
    // Find and split on the same string: an index taken from one string can fall on a non-char
    // boundary of another when `to_lowercase()` changes byte length (e.g. a leading `İ`) and panic.
    let line = line.to_lowercase();
    let (foreground, background) = line.split_at(line.find("on ").unwrap_or(line.len()));
    let foreground = process_color_string(foreground);
    let background = process_color_string(&background.replace("on ", ""));

    let mut style = Style::default();
    if let Some(fg) = parse_color(&foreground.0) {
        style = style.fg(fg);
    }
    if let Some(bg) = parse_color(&background.0) {
        style = style.bg(bg);
    }
    style = style.add_modifier(foreground.1 | background.1);
    style
}

fn process_color_string(color_str: &str) -> (String, Modifier) {
    let color = color_str
        .replace("grey", "gray")
        .replace("bright ", "")
        .replace("bold ", "")
        .replace("underline ", "")
        .replace("inverse ", "");

    let mut modifiers = Modifier::empty();
    if color_str.contains("underline") {
        modifiers |= Modifier::UNDERLINED;
    }
    if color_str.contains("bold") {
        modifiers |= Modifier::BOLD;
    }
    if color_str.contains("inverse") {
        modifiers |= Modifier::REVERSED;
    }

    (color, modifiers)
}

fn parse_color(s: &str) -> Option<Color> {
    let s = s.trim_start();
    let s = s.trim_end();
    if s.contains("bright color") {
        let s = s.trim_start_matches("bright ");
        let c = s
            .trim_start_matches("color")
            .parse::<u8>()
            .unwrap_or_default();
        Some(Color::Indexed(c.wrapping_shl(8)))
    } else if s.contains("color") {
        let c = s
            .trim_start_matches("color")
            .parse::<u8>()
            .unwrap_or_default();
        Some(Color::Indexed(c))
    } else if s.contains("gray") {
        // The 256-color grayscale ramp is palette indices 232..=255, so the offset must stay in
        // 0..=23 or `232 + offset` overflows `u8`.
        let offset = s
            .trim_start_matches("gray")
            .parse::<u16>()
            .unwrap_or_default()
            .min(23);
        Some(Color::Indexed((232 + offset) as u8))
    } else if s.contains("rgb") {
        // "rgb" must be followed by three digits; `.get(3..6)` bails to `None` rather than indexing
        // out of bounds on a short value like "rgb1". Each channel indexes the 6x6x6 color cube, so
        // clamp to 0..=5 to keep `16 + r*36 + g*6 + b` within 16..=231.
        let mut channels = s
            .get(3..6)?
            .chars()
            .map(|ch| ch.to_digit(10).unwrap_or_default().min(5) as u16);
        let (red, green, blue) = (channels.next()?, channels.next()?, channels.next()?);
        Some(Color::Indexed((16 + red * 36 + green * 6 + blue) as u8))
    } else if s == "bold black" {
        Some(Color::Indexed(8))
    } else if s == "bold red" {
        Some(Color::Indexed(9))
    } else if s == "bold green" {
        Some(Color::Indexed(10))
    } else if s == "bold yellow" {
        Some(Color::Indexed(11))
    } else if s == "bold blue" {
        Some(Color::Indexed(12))
    } else if s == "bold magenta" {
        Some(Color::Indexed(13))
    } else if s == "bold cyan" {
        Some(Color::Indexed(14))
    } else if s == "bold white" {
        Some(Color::Indexed(15))
    } else if s == "lightred" {
        Some(Color::LightRed)
    } else if s == "lightgreen" {
        Some(Color::LightGreen)
    } else if s == "lightyellow" {
        Some(Color::LightYellow)
    } else if s == "lightblue" {
        Some(Color::LightBlue)
    } else if s == "lightmagenta" {
        Some(Color::LightMagenta)
    } else if s == "lightcyan" {
        Some(Color::LightCyan)
    } else if s == "black" {
        Some(Color::Indexed(0))
    } else if s == "red" {
        Some(Color::Indexed(1))
    } else if s == "green" {
        Some(Color::Indexed(2))
    } else if s == "yellow" {
        Some(Color::Indexed(3))
    } else if s == "blue" {
        Some(Color::Indexed(4))
    } else if s == "magenta" {
        Some(Color::Indexed(5))
    } else if s == "cyan" {
        Some(Color::Indexed(6))
    } else if s == "white" {
        Some(Color::Indexed(7))
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::errors::AppResult;
    use pretty_assertions::{assert_eq, assert_matches};

    #[test]
    fn test_parse_style_default() {
        let style = parse_style("");
        assert_eq!(style, Style::default());
    }

    #[test]
    fn test_parse_style_foreground() {
        let style = parse_style("red");
        assert_eq!(style.fg, Some(Color::Indexed(1)));
    }

    #[test]
    fn test_parse_style_background() {
        let style = parse_style("on blue");
        assert_eq!(style.bg, Some(Color::Indexed(4)));
    }

    #[test]
    fn test_parse_style_modifiers() {
        let style = parse_style("underline red on blue");
        assert_eq!(style.fg, Some(Color::Indexed(1)));
        assert_eq!(style.bg, Some(Color::Indexed(4)));
    }

    #[test]
    fn test_process_color_string() {
        let (color, modifiers) = process_color_string("underline bold inverse gray");
        assert_eq!(color, "gray");
        assert!(modifiers.contains(Modifier::UNDERLINED));
        assert!(modifiers.contains(Modifier::BOLD));
        assert!(modifiers.contains(Modifier::REVERSED));
    }

    #[test]
    fn test_parse_color_rgb() {
        let color = parse_color("rgb123");
        let expected = 16 + 36 + 2 * 6 + 3;
        assert_eq!(color, Some(Color::Indexed(expected)));
    }

    #[test]
    fn test_parse_color_unknown() {
        let color = parse_color("unknown");
        assert_eq!(color, None);
    }

    #[test]
    fn parse_color_rgb_too_short_is_none_not_panic() {
        assert_eq!(parse_color("rgb"), None);
        assert_eq!(parse_color("rgb1"), None);
        assert_eq!(parse_color("rgb12"), None);
    }

    #[test]
    fn parse_color_rgb_clamps_out_of_range_digits() {
        // Channel digits past the 0..=5 cube axis clamp instead of overflowing `u8` (9*36 = 324).
        assert_eq!(parse_color("rgb900"), Some(Color::Indexed(16 + 5 * 36)));
    }

    #[test]
    fn parse_color_gray_clamps_to_palette_range() {
        // Offsets past 23 clamp instead of overflowing `u8` (232 + 24 = 256).
        assert_eq!(parse_color("gray0"), Some(Color::Indexed(232)));
        assert_eq!(parse_color("gray23"), Some(Color::Indexed(255)));
        assert_eq!(parse_color("gray24"), Some(Color::Indexed(255)));
        assert_eq!(parse_color("gray999"), Some(Color::Indexed(255)));
    }

    #[test]
    fn parse_style_non_ascii_does_not_panic() {
        // `İ`.to_lowercase() is two bytes ("i̇"); finding and splitting on different strings would
        // land off a char boundary here and panic.
        assert_eq!(parse_style("İ on red").bg, Some(Color::Indexed(1)));
    }

    #[test]
    fn theme_resolve_prefers_user_then_falls_back() {
        let fallback = ThemeConfig {
            accent: Some("yellow".into()),
            accent_active: Some("lightyellow".into()),
            title: Some("bold lightyellow".into()),
            throbber: Some("lightyellow".into()),
        };
        let user = ThemeConfig {
            accent: Some("red".into()),
            ..Default::default()
        };

        let theme = user.resolve(&fallback);

        assert_eq!(theme.accent, parse_style("red"));
        assert_eq!(theme.accent_active, parse_style("lightyellow"));
        assert_eq!(theme.title, parse_style("bold lightyellow"));
    }

    #[test]
    fn test_config() -> AppResult<()> {
        let c = Config::new()?;
        assert_matches!(
            c.keybindings
                .get(&Mode::App)
                .unwrap()
                .get(&parse_key_sequence("<Ctrl-c>").unwrap_or_default())
                .unwrap(),
            &Action::Quit
        );
        Ok(())
    }

    #[test]
    fn malformed_keybinding_is_an_error_not_a_panic() {
        let json = r#"{ "Home": { "<not-a-real-key>": "Quit" } }"#;
        let result: Result<KeyBindings, _> = json5::from_str(json);
        assert!(result.is_err());
    }

    #[test]
    fn test_simple_keys() {
        assert_eq!(
            parse_key_event("a").unwrap(),
            KeyEvent::new(KeyCode::Char('a'), KeyModifiers::empty())
        );

        assert_eq!(
            parse_key_event("enter").unwrap(),
            KeyEvent::new(KeyCode::Enter, KeyModifiers::empty())
        );

        assert_eq!(
            parse_key_event("esc").unwrap(),
            KeyEvent::new(KeyCode::Esc, KeyModifiers::empty())
        );
    }

    #[test]
    fn test_with_modifiers() {
        assert_eq!(
            parse_key_event("ctrl-a").unwrap(),
            KeyEvent::new(KeyCode::Char('a'), KeyModifiers::CONTROL)
        );

        assert_eq!(
            parse_key_event("alt-enter").unwrap(),
            KeyEvent::new(KeyCode::Enter, KeyModifiers::ALT)
        );

        assert_eq!(
            parse_key_event("shift-esc").unwrap(),
            KeyEvent::new(KeyCode::Esc, KeyModifiers::SHIFT)
        );
    }

    #[test]
    fn test_multiple_modifiers() {
        assert_eq!(
            parse_key_event("ctrl-alt-a").unwrap(),
            KeyEvent::new(
                KeyCode::Char('a'),
                KeyModifiers::CONTROL | KeyModifiers::ALT
            )
        );

        assert_eq!(
            parse_key_event("ctrl-shift-enter").unwrap(),
            KeyEvent::new(KeyCode::Enter, KeyModifiers::CONTROL | KeyModifiers::SHIFT)
        );
    }

    #[test]
    fn test_reverse_multiple_modifiers() {
        assert_eq!(
            key_event_to_string(&KeyEvent::new(
                KeyCode::Char('a'),
                KeyModifiers::CONTROL | KeyModifiers::ALT
            )),
            "ctrl-alt-a".to_string()
        );
    }

    #[test]
    fn test_invalid_keys() {
        assert!(parse_key_event("invalid-key").is_err());
        assert!(parse_key_event("ctrl-invalid-key").is_err());
    }

    #[test]
    fn test_case_insensitivity() {
        assert_eq!(
            parse_key_event("CTRL-a").unwrap(),
            KeyEvent::new(KeyCode::Char('a'), KeyModifiers::CONTROL)
        );

        assert_eq!(
            parse_key_event("AlT-eNtEr").unwrap(),
            KeyEvent::new(KeyCode::Enter, KeyModifiers::ALT)
        );
    }
}
