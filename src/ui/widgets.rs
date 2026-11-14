use gpui::prelude::*;
use gpui::{
    div, px, svg, FontWeight, Hsla, IntoElement, Rgba, SharedString, Stateful, Div, ElementId,
};

use crate::models::avatar_color;
use crate::theme::Theme;

pub fn icon(name: &str, size: f32, color: impl Into<Hsla>) -> impl IntoElement {
    svg()
        .path(SharedString::from(format!("icons/{name}.svg")))
        .flex_none()
        .size(px(size))
        .text_color(color.into())
}

/// The embedded application logo.
pub fn logo(size: f32) -> impl IntoElement {
    gpui::img(gpui::ImageSource::Resource(gpui::Resource::Embedded(
        SharedString::from("logo.png"),
    )))
    .flex_none()
    .size(px(size))
}

pub fn icon_button(
    id: impl Into<ElementId>,
    name: &'static str,
    size: f32,
    color: Rgba,
    hover_bg: Rgba,
    box_size: f32,
) -> Stateful<Div> {
    let color: Hsla = color.into();
    let hover_bg: Hsla = hover_bg.into();
    div()
        .id(id)
        .flex_none()
        .flex()
        .items_center()
        .justify_center()
        .size(px(box_size))
        .rounded_md()
        .cursor_pointer()
        .hover(move |style| style.bg(hover_bg))
        .child(icon(name, size, color))
}

/// A quiet, text-free contact badge: a neutral rounded tile with a semantic
/// line glyph and a small hue dot derived from the address.
///
/// Keeping the surface neutral avoids the heavy "coloured square + initial"
/// look while the dot and glyph still make rows easy to scan.
pub fn avatar(seed: &str, _label: &str, size: f32, _font_size: f32, theme: &Theme) -> Div {
    let seed_lower = seed.to_lowercase();
    let local = seed_lower.split('@').next().unwrap_or("");
    let role_address = ["notify", "no-reply", "noreply", "team", "hello", "support", "info",
        "news", "digest", "billing", "receipts", "jobs", "hr", "updates", "mailer"]
        .iter()
        .any(|prefix| local.starts_with(prefix));
    let glyph = if seed_lower.contains("figma")
        || seed_lower.contains("linear")
        || seed_lower.contains("notion")
        || seed_lower.contains("dribbble")
        || seed_lower.contains("behance")
    {
        "layers"
    } else if seed_lower.contains("github")
        || seed_lower.contains("gitlab")
        || seed_lower.contains("bitbucket")
    {
        "server"
    } else if seed_lower.contains("stripe")
        || seed_lower.contains("paypal")
        || seed_lower.contains("billing")
        || seed_lower.contains("receipt")
    {
        "tag"
    } else if seed_lower.contains("geek")
        || seed_lower.contains("newsletter")
        || seed_lower.contains("digest")
    {
        "bolt"
    } else if role_address {
        "mail"
    } else {
        "user"
    };

    let base: Hsla = gpui::rgb(avatar_color(seed)).into();
    let tint = Hsla {
        h: base.h,
        s: (base.s * 0.72).min(0.62),
        l: if theme.dark { 0.74 } else { 0.46 },
        a: 1.0,
    };
    let dot = Hsla {
        h: base.h,
        s: (base.s * 0.78).min(0.68),
        l: if theme.dark { 0.66 } else { 0.54 },
        a: 1.0,
    };

    div()
        .relative()
        .flex_none()
        .size(px(size))
        .rounded(px(size * 0.30))
        .bg(if theme.dark { theme.card } else { theme.surface })
        .border_1()
        .border_color(theme.border_soft)
        .flex()
        .items_center()
        .justify_center()
        .child(icon(glyph, size * 0.46, tint))
        .child(
            div()
                .absolute()
                .right(px(size * 0.13))
                .bottom(px(size * 0.13))
                .size(px((size * 0.14).max(2.5)))
                .rounded_full()
                .bg(dot),
        )
}
pub fn pill(text: impl Into<SharedString>, bg: impl Into<Hsla>, fg: impl Into<Hsla>) -> Div {
    div()
        .flex_none()
        .h(px(18.))
        .min_w(px(18.))
        .px(px(5.))
        .rounded_full()
        .flex()
        .items_center()
        .justify_center()
        .bg(bg.into())
        .text_color(fg.into())
        .text_size(px(10.5))
        .font_weight(FontWeight::MEDIUM)
        .child(text.into())
}

pub fn vertical_divider(theme: &Theme, height: f32) -> Div {
    div()
        .flex_none()
        .w(px(1.))
        .h(px(height))
        .bg(theme.border_soft)
        .mx(px(6.))
}

pub fn horizontal_divider(theme: &Theme) -> Div {
    div()
        .flex_none()
        .w_full()
        .h(px(1.))
        .bg(theme.border_soft)
}

pub fn section_label(text: &'static str, theme: &Theme) -> Div {
    div()
        .px(px(12.))
        .pt(px(14.))
        .pb(px(6.))
        .text_size(px(10.5))
        .font_weight(FontWeight::SEMIBOLD)
        .text_color(theme.text_muted)
        .child(text)
}

pub fn keycap(label: &'static str, theme: &Theme) -> Div {
    div()
        .flex_none()
        .px(px(5.))
        .h(px(18.))
        .flex()
        .items_center()
        .justify_center()
        .rounded_sm()
        .border_1()
        .border_color(theme.border)
        .text_size(px(9.5))
        .text_color(theme.text_muted)
        .child(label)
}


