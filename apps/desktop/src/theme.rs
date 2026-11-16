use gpui_kit::{
    App, Hsla, Window,
    component::{Theme, ThemeMode},
    px, rgb,
};

/// Aplica el modo de tema (claro u oscuro) y personaliza los tokens de marca
/// sobre el tema global de GPUI Kit para mantener consistencia en todos los
/// componentes (`Sidebar`, `Button`, `Alert`, `Switch`, `GroupBox`, etc.).
pub fn apply_theme(mode: ThemeMode, window: Option<&mut Window>, cx: &mut App) {
    Theme::change(mode, window, cx);

    Theme::update(cx, |theme| {
        theme.radius = px(8.);
        theme.radius_lg = px(12.);

        if theme.mode.is_dark() {
            // Paleta oscura con contraste limpio entre sidebar, fondo y tarjetas.
            theme.colors.primary = Hsla::from(rgb(0x3b82f6));
            theme.colors.primary_hover = Hsla::from(rgb(0x60a5fa));
            theme.colors.primary_active = Hsla::from(rgb(0x2563eb));
            theme.colors.primary_foreground = Hsla::from(rgb(0xffffff));
            theme.colors.sidebar_primary = Hsla::from(rgb(0x3b82f6));
            theme.colors.sidebar_primary_foreground = Hsla::from(rgb(0xffffff));
            theme.colors.ring = Hsla::from(rgb(0x60a5fa));
        } else {
            // Paleta clara corporativa con acento azul sobrio.
            theme.colors.primary = Hsla::from(rgb(0x2563eb));
            theme.colors.primary_hover = Hsla::from(rgb(0x1d4ed8));
            theme.colors.primary_active = Hsla::from(rgb(0x1e40af));
            theme.colors.primary_foreground = Hsla::from(rgb(0xffffff));
            theme.colors.sidebar_primary = Hsla::from(rgb(0x2563eb));
            theme.colors.sidebar_primary_foreground = Hsla::from(rgb(0xffffff));
            theme.colors.ring = Hsla::from(rgb(0x3b82f6));
        }
    });
}
