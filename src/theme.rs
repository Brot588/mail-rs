use gpui::Rgba;

/// Colour tokens for the two bundled themes.
#[derive(Clone, Copy)]
pub struct Theme {
    pub dark: bool,
    pub bg: Rgba,
    pub sidebar: Rgba,
    pub surface: Rgba,
    pub reader: Rgba,
    pub card: Rgba,
    pub card_hover: Rgba,
    pub border: Rgba,
    pub border_soft: Rgba,
    pub text: Rgba,
    pub text_soft: Rgba,
    pub text_muted: Rgba,
    pub accent: Rgba,
    pub accent_hover: Rgba,
    pub accent_soft: Rgba,
    pub danger: Rgba,
    pub star: Rgba,
    pub success: Rgba,
    pub overlay: Rgba,
    pub selected_row: Rgba,
}

impl Theme {
    pub fn dark() -> Self {
        Self {
            dark: true,
            bg: gpui::rgb(0x0C0E12),
            sidebar: gpui::rgb(0x11141A),
            surface: gpui::rgb(0x0F1217),
            reader: gpui::rgb(0x12161C),
            card: gpui::rgb(0x191E26),
            card_hover: gpui::rgb(0x212734),
            border: gpui::rgb(0x252B36),
            border_soft: gpui::rgb(0x1C212A),
            text: gpui::rgb(0xE9EDF4),
            text_soft: gpui::rgb(0xB2BBCA),
            text_muted: gpui::rgb(0x79839A),
            accent: gpui::rgb(0x8296FF),
            accent_hover: gpui::rgb(0x9AABFF),
            accent_soft: gpui::rgb(0x1D2540),
            danger: gpui::rgb(0xFF7676),
            star: gpui::rgb(0xFFC66D),
            success: gpui::rgb(0x5CD9A0),
            overlay: gpui::rgba(0x05070ACC),
            selected_row: gpui::rgb(0x1A2130),
        }
    }

    pub fn light() -> Self {
        Self {
            dark: false,
            bg: gpui::rgb(0xF1F3F8),
            sidebar: gpui::rgb(0xE9ECF3),
            surface: gpui::rgb(0xFAFBFD),
            reader: gpui::rgb(0xFFFFFF),
            card: gpui::rgb(0xF3F5F9),
            card_hover: gpui::rgb(0xE9EDF6),
            border: gpui::rgb(0xDFE3EC),
            border_soft: gpui::rgb(0xEAEDF3),
            text: gpui::rgb(0x1A1F2A),
            text_soft: gpui::rgb(0x4B5468),
            text_muted: gpui::rgb(0x8A93A6),
            accent: gpui::rgb(0x4F64E6),
            accent_hover: gpui::rgb(0x3E52D4),
            accent_soft: gpui::rgb(0xE5E9FC),
            danger: gpui::rgb(0xDB5252),
            star: gpui::rgb(0xDE9B33),
            success: gpui::rgb(0x2AA57A),
            overlay: gpui::rgba(0x11182766),
            selected_row: gpui::rgb(0xE7ECFA),
        }
    }

    pub fn toggle(self) -> Self {
        if self.dark {
            Self::light()
        } else {
            Self::dark()
        }
    }
}
