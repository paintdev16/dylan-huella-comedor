use gpui_kit::{
    App, Context, Div, FontWeight, Hsla, IntoElement, ParentElement, SharedString, Styled,
    assets::IconName as LucideIcon,
    component::{
        ActiveTheme as _, Colorize as _, Icon,
        alert::Alert,
        amber_400, amber_500, amber_600, blue_400, blue_500, blue_600, emerald_400, emerald_500,
        emerald_600, green_400, green_500, green_600, h_flex, orange_400, orange_500, orange_600,
        red_400, red_500, red_600, v_flex, violet_400, violet_500, violet_600,
    },
    div,
    prelude::FluentBuilder as _,
    px,
};

use crate::layout::DesktopApp;

/// Tono semántico reutilizable para cards e indicadores de toda la aplicación.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum SemanticTone {
    #[default]
    Neutral,
    /// Azul informativo (conectividad, tickets, búsquedas).
    Blue,
    /// Verde de éxito e ingresos en efectivo.
    Green,
    /// Verde suave para saldos a favor.
    SoftGreen,
    /// Ámbar para desayunos y advertencias operativas.
    Amber,
    /// Naranja para almuerzos y raciones.
    Orange,
    /// Rojo para errores o fallos de dispositivo/conexión.
    Red,
    /// Rojo suave para deudas pendientes y saldos en contra.
    SoftRed,
    /// Violeta para indicadores especiales (plantillas y arqueo).
    Violet,
}

#[derive(Clone, Copy)]
pub struct TonePalette {
    pub bg: Hsla,
    pub border: Hsla,
    pub value: Hsla,
    pub icon: Hsla,
    pub icon_bg: Hsla,
}

impl SemanticTone {
    pub fn palette(self, cx: &App) -> TonePalette {
        let theme = cx.theme();
        let is_dark = theme.is_dark();

        let (accent, tint, bg_mix, border_mix, icon_alpha) = match self {
            Self::Neutral => {
                return TonePalette {
                    bg: theme.colors.list,
                    border: theme.border,
                    value: theme.foreground,
                    icon: theme.muted_foreground,
                    icon_bg: theme.muted,
                };
            }
            Self::Blue => (
                if is_dark { blue_400() } else { blue_600() },
                blue_500(),
                if is_dark { 0.08 } else { 0.055 },
                if is_dark { 0.28 } else { 0.22 },
                if is_dark { 0.15 } else { 0.11 },
            ),
            Self::Green => (
                if is_dark { green_400() } else { green_600() },
                green_500(),
                if is_dark { 0.08 } else { 0.055 },
                if is_dark { 0.28 } else { 0.22 },
                if is_dark { 0.15 } else { 0.11 },
            ),
            Self::SoftGreen => (
                if is_dark {
                    emerald_400().mix(theme.foreground, 0.82)
                } else {
                    emerald_600().mix(theme.foreground, 0.85)
                },
                emerald_500(),
                if is_dark { 0.055 } else { 0.04 },
                if is_dark { 0.20 } else { 0.16 },
                if is_dark { 0.12 } else { 0.09 },
            ),
            Self::Amber => (
                if is_dark { amber_400() } else { amber_600() },
                amber_500(),
                if is_dark { 0.08 } else { 0.055 },
                if is_dark { 0.28 } else { 0.22 },
                if is_dark { 0.15 } else { 0.11 },
            ),
            Self::Orange => (
                if is_dark { orange_400() } else { orange_600() },
                orange_500(),
                if is_dark { 0.08 } else { 0.055 },
                if is_dark { 0.28 } else { 0.22 },
                if is_dark { 0.15 } else { 0.11 },
            ),
            Self::Red => (
                if is_dark { red_400() } else { red_600() },
                red_500(),
                if is_dark { 0.08 } else { 0.055 },
                if is_dark { 0.30 } else { 0.24 },
                if is_dark { 0.15 } else { 0.11 },
            ),
            Self::SoftRed => (
                if is_dark {
                    red_400().mix(theme.foreground, 0.85)
                } else {
                    red_600().mix(theme.foreground, 0.88)
                },
                red_500(),
                if is_dark { 0.055 } else { 0.04 },
                if is_dark { 0.20 } else { 0.16 },
                if is_dark { 0.12 } else { 0.09 },
            ),
            Self::Violet => (
                if is_dark { violet_400() } else { violet_600() },
                violet_500(),
                if is_dark { 0.08 } else { 0.055 },
                if is_dark { 0.28 } else { 0.22 },
                if is_dark { 0.15 } else { 0.11 },
            ),
        };

        TonePalette {
            bg: theme.colors.list.mix(tint, 1.0 - bg_mix),
            border: theme.border.mix(tint, 1.0 - border_mix),
            value: accent,
            icon: accent,
            icon_bg: tint.opacity(icon_alpha),
        }
    }
}

/// Tarjeta de métrica/KPI unificada con ícono, cifra destacada y subtítulo opcional.
pub fn metric_card(
    title: &'static str,
    value: impl Into<SharedString>,
    detail: Option<SharedString>,
    icon: LucideIcon,
    tone: SemanticTone,
    cx: &App,
) -> impl IntoElement {
    let theme = cx.theme();
    let palette = tone.palette(cx);
    v_flex()
        .flex_1()
        .min_w(px(180.))
        .gap_2p5()
        .p_4()
        .rounded(theme.radius_lg)
        .bg(palette.bg)
        .border_1()
        .border_color(palette.border)
        .child(
            h_flex()
                .justify_between()
                .items_center()
                .gap_2()
                .child(div().text_sm().text_color(theme.muted_foreground).child(title))
                .child(
                    div()
                        .p_1p5()
                        .rounded(theme.radius)
                        .bg(palette.icon_bg)
                        .child(Icon::new(icon).size_4().text_color(palette.icon)),
                ),
        )
        .child(
            div()
                .text_2xl()
                .font_weight(FontWeight::BOLD)
                .text_color(palette.value)
                .child(value.into()),
        )
        .when_some(detail, |this, sub| {
            this.child(div().text_xs().text_color(theme.muted_foreground).child(sub))
        })
}

/// Contenedor informativo con borde, radio y fondo semántico sutil unificados.
pub fn info_panel(tone: SemanticTone, cx: &App) -> Div {
    let theme = cx.theme();
    let palette = tone.palette(cx);
    div()
        .p_3()
        .rounded(theme.radius_lg)
        .border_1()
        .border_color(palette.border)
        .bg(palette.bg)
}

/// Banner de error no bloqueante en la parte superior del área de contenido.
pub fn render_error_banner(
    message: SharedString,
    cx: &Context<DesktopApp>,
) -> impl IntoElement {
    Alert::error("app-error-banner", message)
        .title("Atención")
        .on_close(cx.listener(|this, _, _, cx| {
            this.dismiss_error(cx);
        }))
}
