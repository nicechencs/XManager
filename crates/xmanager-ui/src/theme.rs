//! Runtime light/dark theme colors for XManager.

use std::sync::atomic::{AtomicU8, Ordering};

use gpui::{rgb, Rgba};

/// The active application appearance.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[repr(u8)]
pub enum ThemeMode {
    /// Calm, high-contrast light surfaces suitable for daytime use.
    #[default]
    Light = 0,
    /// The original XManager dark palette.
    Dark = 1,
}

impl ThemeMode {
    pub const fn label_zh(self) -> &'static str {
        match self {
            Self::Light => "浅色",
            Self::Dark => "深色",
        }
    }

    const fn from_u8(value: u8) -> Self {
        match value {
            1 => Self::Dark,
            _ => Self::Light,
        }
    }
}

static CURRENT_MODE: AtomicU8 = AtomicU8::new(ThemeMode::Light as u8);

/// Return the process-wide appearance used by [`c`].
#[inline]
pub fn current_mode() -> ThemeMode {
    ThemeMode::from_u8(CURRENT_MODE.load(Ordering::Relaxed))
}

/// Set the process-wide appearance used by [`c`].
#[inline]
pub fn set_mode(mode: ThemeMode) {
    CURRENT_MODE.store(mode as u8, Ordering::Relaxed);
}

/// Flip the process-wide appearance and return the new mode.
#[inline]
pub fn toggle_mode() -> ThemeMode {
    let previous = CURRENT_MODE.fetch_xor(1, Ordering::Relaxed);
    ThemeMode::from_u8(previous ^ 1)
}

// Dark palette values are intentionally kept unchanged so existing callers and
// screenshots retain the original appearance when ThemeMode::Dark is active.
pub const BG: u32 = 0x0f_11_15;
pub const BG_ELEVATED: u32 = 0x16_1a_22;
pub const BG_PANEL: u32 = 0x1b_20_2a;
pub const BG_ROW: u32 = 0x14_18_20;
pub const BG_ROW_ALT: u32 = 0x18_1d_27;
pub const BG_ROW_SELECTED: u32 = 0x1f_2a_3d;
pub const BG_SELECTED: u32 = 0x24_3b_5f;
pub const BG_HOVER: u32 = 0x22_28_34;
pub const BORDER: u32 = 0x2a_31_3d;
pub const BORDER_STRONG: u32 = 0x3a_44_55;
pub const TEXT: u32 = 0xe6_ea_f2;
pub const TEXT_MUTED: u32 = 0x8b_95_a8;
pub const TEXT_DIM: u32 = 0x5c_6678;
pub const ACCENT: u32 = 0x3b_82_f6;
pub const ACCENT_HOVER: u32 = 0x2563_eb;
pub const SUCCESS: u32 = 0x22_c5_5e;
pub const WARNING: u32 = 0xf5_9e_0b;
pub const DANGER: u32 = 0xef_44_44;
pub const DANGER_HOVER: u32 = 0xdc_26_26;
pub const CHIP: u32 = 0x24_2b_38;
pub const CHIP_ACTIVE: u32 = 0x1d_3a_6d;
pub const HIST_BAR: u32 = 0x38_bd_f8;
pub const QUOTE: u32 = 0xa7_8b_fa;

// Explicit semantic tokens for states that used to be hard-coded at call sites.
pub const DISABLED_BG: u32 = 0x30_36_42;
pub const DISABLED_DANGER_BG: u32 = 0x3a_1f_1f;
pub const DISABLED_TEXT: u32 = 0x6b_75_88;
pub const ERROR_BG: u32 = 0x3b_15_15;
pub const TEXT_ON_ACCENT: u32 = 0xf8_fa_ff;
pub const TEXT_ON_DANGER: u32 = 0xff_f7_f7;

#[inline]
fn palette(hex: u32, mode: ThemeMode) -> u32 {
    if mode == ThemeMode::Dark {
        return match hex {
            // These semantic tokens replace former call-site literals while
            // retaining the exact colors of the original dark UI.
            DISABLED_BG => BORDER,
            DISABLED_DANGER_BG => 0x3a_1f_1f,
            DISABLED_TEXT => TEXT_DIM,
            ERROR_BG => 0x3b_15_15,
            TEXT_ON_ACCENT | TEXT_ON_DANGER => TEXT,
            _ => hex,
        };
    }

    match hex {
        BG => 0xf5_f7_fb,
        BG_ELEVATED => 0xff_ff_ff,
        BG_PANEL => 0xff_ff_ff,
        BG_ROW => 0xff_ff_ff,
        BG_ROW_ALT => 0xf7_f9_fc,
        BG_ROW_SELECTED => 0xe6_f0_ff,
        BG_SELECTED => 0xcf_e3_ff,
        BG_HOVER => 0xed_f4_ff,
        BORDER => 0xd7_de_e8,
        BORDER_STRONG => 0xb7_c3_d3,
        TEXT => 0x17_20_33,
        TEXT_MUTED => 0x4f_5d_70,
        TEXT_DIM => 0x7b_87_98,
        ACCENT => 0x25_63_eb,
        ACCENT_HOVER => 0x1d_4e_d8,
        SUCCESS => 0x15_80_3d,
        WARNING => 0xa1_5c_00,
        DANGER => 0xc6_28_28,
        DANGER_HOVER => 0xb9_1c_1c,
        CHIP => 0xe9_ee_f5,
        CHIP_ACTIVE => 0xdb_ea_fe,
        HIST_BAR => 0x02_84_c7,
        QUOTE => 0x7c_3a_ed,
        DISABLED_BG => 0xe5_ea_f1,
        DISABLED_DANGER_BG => 0xe5_ea_f1,
        DISABLED_TEXT => 0x5e_6a_79,
        ERROR_BG => 0xfe_e2_e2,
        // White text keeps accent and destructive controls readable in both
        // modes, including the light palette's saturated button colors.
        TEXT_ON_ACCENT | TEXT_ON_DANGER => 0xff_ff_ff,
        _ => hex,
    }
}

/// Convert a semantic palette token to a GPUI color in the active mode.
///
/// Unknown values are passed through unchanged for compatibility with callers
/// that provide a one-off color. UI code should prefer the named tokens above.
#[inline]
pub fn c(hex: u32) -> Rgba {
    rgb(palette(hex, current_mode()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn light_is_the_default_mode() {
        assert_eq!(ThemeMode::default(), ThemeMode::Light);
    }

    #[test]
    fn switching_modes_changes_surface_palette_and_preserves_dark_tokens() {
        assert_eq!(palette(BG, ThemeMode::Dark), BG);
        assert_ne!(palette(BG, ThemeMode::Light), palette(BG, ThemeMode::Dark));
        assert_eq!(palette(TEXT_ON_ACCENT, ThemeMode::Light), 0xff_ff_ff);
        assert_eq!(palette(TEXT_ON_DANGER, ThemeMode::Dark), TEXT);
    }

    fn relative_luminance(hex: u32) -> f64 {
        fn linear_channel(channel: u32) -> f64 {
            let srgb = channel as f64 / 255.0;
            if srgb <= 0.03928 {
                srgb / 12.92
            } else {
                ((srgb + 0.055) / 1.055).powf(2.4)
            }
        }

        let red = linear_channel((hex >> 16) & 0xff);
        let green = linear_channel((hex >> 8) & 0xff);
        let blue = linear_channel(hex & 0xff);
        0.2126 * red + 0.7152 * green + 0.0722 * blue
    }

    fn contrast_ratio(foreground: u32, background: u32) -> f64 {
        let foreground = relative_luminance(foreground);
        let background = relative_luminance(background);
        (foreground.max(background) + 0.05) / (foreground.min(background) + 0.05)
    }

    #[test]
    fn light_action_and_disabled_pairs_meet_accessible_contrast() {
        let light_accent = palette(ACCENT, ThemeMode::Light);
        let light_danger = palette(DANGER, ThemeMode::Light);
        let light_disabled_bg = palette(DISABLED_BG, ThemeMode::Light);
        let light_disabled_text = palette(DISABLED_TEXT, ThemeMode::Light);

        assert!(contrast_ratio(palette(TEXT_ON_ACCENT, ThemeMode::Light), light_accent) >= 4.5);
        assert!(contrast_ratio(palette(TEXT_ON_DANGER, ThemeMode::Light), light_danger) >= 4.5);
        assert!(contrast_ratio(light_disabled_text, light_disabled_bg) >= 4.5);
    }
}
