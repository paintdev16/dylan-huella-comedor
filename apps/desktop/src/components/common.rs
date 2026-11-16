use gpui_kit::{
    Context, IntoElement, SharedString,
    component::alert::Alert,
};

use crate::layout::DesktopApp;

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
