use crate::api::schema::{ClientTheme, ClientThemeColors};
use crate::app::state::Palette;
use crate::protocol::{ClientHostDefaultColorKind, ClientHostThemeUpdate};
use crate::terminal_theme::{RgbColor, TerminalTheme};
use ratatui::style::Color;

pub(super) fn observed_theme(updates: &[ClientHostThemeUpdate]) -> TerminalTheme {
    let mut theme = TerminalTheme::default();
    for update in updates {
        match update {
            ClientHostThemeUpdate::DefaultColor { kind, color } => match kind {
                ClientHostDefaultColorKind::Foreground => theme.foreground = Some((*color).into()),
                ClientHostDefaultColorKind::Background => theme.background = Some((*color).into()),
            },
            ClientHostThemeUpdate::PaletteColors(colors) => {
                for (index, color) in colors {
                    theme.palette[usize::from(*index)] = Some((*color).into());
                }
            }
            ClientHostThemeUpdate::Appearance(_) => {}
        }
    }
    theme
}

pub(super) fn merge_palette_colors(
    current: &mut Vec<(u8, crate::protocol::ClientHostColor)>,
    incoming: &[(u8, crate::protocol::ClientHostColor)],
) {
    let mut colors = [None; 256];
    for (index, color) in current.iter().chain(incoming) {
        colors[usize::from(*index)] = Some(*color);
    }
    *current = colors
        .into_iter()
        .enumerate()
        .filter_map(|(index, color)| color.map(|color| (index as u8, color)))
        .collect();
}

fn hex(color: RgbColor) -> String {
    format!("#{:02x}{:02x}{:02x}", color.r, color.g, color.b)
}

fn resolved_color(color: Color, default: Option<RgbColor>, host: &TerminalTheme) -> Option<String> {
    let index = match color {
        Color::Reset => return default.map(hex),
        Color::Rgb(r, g, b) => return Some(hex(RgbColor { r, g, b })),
        Color::Indexed(index) => index,
        Color::Black => 0,
        Color::Red => 1,
        Color::Green => 2,
        Color::Yellow => 3,
        Color::Blue => 4,
        Color::Magenta => 5,
        Color::Cyan => 6,
        Color::Gray => 7,
        Color::DarkGray => 8,
        Color::LightRed => 9,
        Color::LightGreen => 10,
        Color::LightYellow => 11,
        Color::LightBlue => 12,
        Color::LightMagenta => 13,
        Color::LightCyan => 14,
        Color::White => 15,
    };
    host.palette[usize::from(index)].map(hex)
}

pub(super) fn snapshot(name: &str, palette: &Palette, host: &TerminalTheme) -> ClientTheme {
    let fg = |color| resolved_color(color, host.foreground, host);
    let bg = |color| resolved_color(color, host.background, host);
    ClientTheme {
        name: name.to_owned(),
        colors: ClientThemeColors {
            background: bg(palette.panel_bg),
            text: fg(palette.text),
            accent: fg(palette.accent),
            muted: fg(palette.subtext0),
            border: fg(palette.overlay1),
            surface: bg(palette.surface0),
            selection: bg(palette.active_row_bg),
            working: fg(palette.yellow),
            blocked: fg(palette.red),
            done: fg(palette.teal),
            unknown: fg(palette.overlay0),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn client_theme_preserves_custom_palette_colors() {
        let mut palette = Palette::catppuccin_latte();
        palette.accent = Color::Rgb(1, 2, 3);
        let theme = snapshot("custom-light", &palette, &TerminalTheme::default());
        assert_eq!(theme.colors.accent.as_deref(), Some("#010203"));
        assert_eq!(theme.colors.background.as_deref(), Some("#eff1f5"));
        assert_eq!(theme.colors.text.as_deref(), Some("#4c4f69"));
    }
    #[test]
    fn client_theme_resolves_terminal_defaults_and_symbolic_colors() {
        let mut host = TerminalTheme {
            foreground: Some(RgbColor {
                r: 220,
                g: 221,
                b: 222,
            }),
            background: Some(RgbColor {
                r: 20,
                g: 21,
                b: 22,
            }),
            ..Default::default()
        };
        host.palette[4] = Some(RgbColor {
            r: 100,
            g: 150,
            b: 200,
        });
        let theme = snapshot("terminal", &Palette::terminal(), &host);
        assert_eq!(theme.colors.background.as_deref(), Some("#141516"));
        assert_eq!(theme.colors.text.as_deref(), Some("#dcddde"));
        assert_eq!(theme.colors.accent.as_deref(), Some("#6496c8"));
        assert_eq!(theme.colors.done, None);
    }

    #[test]
    fn client_theme_partial_palette_updates_preserve_other_observed_colors() {
        let first = crate::protocol::ClientHostColor { r: 1, g: 2, b: 3 };
        let second = crate::protocol::ClientHostColor { r: 4, g: 5, b: 6 };
        let replacement = crate::protocol::ClientHostColor { r: 7, g: 8, b: 9 };
        let mut palette = vec![(4, first), (6, second)];
        merge_palette_colors(&mut palette, &[(4, replacement)]);
        let host = observed_theme(&[ClientHostThemeUpdate::PaletteColors(palette)]);
        assert_eq!(host.palette[4], Some(replacement.into()));
        assert_eq!(host.palette[6], Some(second.into()));
    }
}
