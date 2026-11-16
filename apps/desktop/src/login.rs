use gpui_kit::{
    assets::IconName,
    component::{ActiveTheme as _, Disableable as _, Icon, button::{Button, ButtonVariants as _}, input::{Input, InputState}, v_flex},
    div, px, Context, Entity, FontWeight, IntoElement, ParentElement, Styled,
};
use crate::{auth::AuthStatus, layout::DesktopApp};

pub fn render(status: &AuthStatus, username: &Entity<InputState>, password: &Entity<InputState>, cx: &Context<DesktopApp>) -> impl IntoElement {
    let theme = cx.theme();
    let loading = matches!(status, AuthStatus::Authenticating);
    let message = match status {
        AuthStatus::Error(message) => Some(message.as_str()),
        AuthStatus::SessionExpired => Some("La sesión ha expirado. Inicia sesión nuevamente."),
        _ => None,
    };
    div().flex_1().min_h_0().flex().items_center().justify_center().p_6()
        .child(v_flex().w_full().max_w(px(420.)).p_8().gap_4().rounded(theme.radius_lg)
            .border_1().border_color(theme.border).bg(theme.colors.list)
            .child(div().size(px(52.)).rounded(theme.radius).bg(theme.primary).text_color(theme.primary_foreground)
                .flex().items_center().justify_center().child(Icon::new(IconName::Utensils).size_6()))
            .child(v_flex().gap_1().child(div().text_2xl().font_weight(FontWeight::BOLD).child("Comedor"))
                .child(div().text_sm().text_color(theme.muted_foreground).child("Gestión de Pensionistas")))
            .child(div().font_weight(FontWeight::SEMIBOLD).child("Iniciar sesión"))
            .child(v_flex().gap_2().child("Usuario o correo").child(Input::new(username)))
            .child(v_flex().gap_2().child("Contraseña").child(Input::new(password).mask_toggle()))
            .children(message.map(|msg| div().text_sm().text_color(theme.danger).child(msg.to_string())))
            .child(Button::new("auth-login").primary().label(if loading { "Verificando..." } else { "Iniciar sesión" })
                .loading(loading).disabled(loading).on_click(cx.listener(|this, _, window, cx| this.sign_in(window, cx))))
            .child(div().text_xs().text_color(theme.muted_foreground).child(if cfg!(debug_assertions) { "Demostración local: consulta las credenciales de prueba en el README." } else { "Autenticación real pendiente de integración." }))
            .child(div().text_xs().text_color(theme.muted_foreground).child(format!("Versión {}", env!("CARGO_PKG_VERSION")))))
}
