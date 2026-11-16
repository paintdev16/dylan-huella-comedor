pub mod cash;
pub mod consumptions;
pub mod dashboard;
pub mod identification;
pub mod pensioners;
pub mod reports;
pub mod settings;

use gpui_kit::{App, Context, Entity, IntoElement, SharedString, component::input::InputState};

use crate::{
    biometric_service::BiometricReaderState,
    layout::DesktopApp,
    state::{AppState, Page},
};

/// Renderiza la página activa de acuerdo con el estado de navegación.
pub fn render_page(
    state: &AppState,
    app_view: &DesktopApp,
    cx: &Context<DesktopApp>,
    app_cx: &App,
    search: &Entity<InputState>,
    filter: pensioners::Filter,
    selected: Option<u32>,
    reader_state: &BiometricReaderState,
    biometric_events: &[SharedString],
) -> impl IntoElement {
    match state.page {
        Page::Dashboard => dashboard::render(state, cx, app_cx).into_any_element(),
        Page::Pensionistas => {
            pensioners::render(state, cx, app_cx, search, filter, selected, app_view.biometric_modal_open).into_any_element()
        }
        Page::Identificacion => identification::render(
            state,
            reader_state,
            biometric_events,
            app_view.biometric_gateway.is_finger_present(),
            app_view.biometric_modal_open,
            cx,
            app_cx,
        )
        .into_any_element(),
        Page::Consumos => consumptions::render(state, cx, app_cx).into_any_element(),
        Page::Caja => cash::render(state, app_view, cx, app_cx).into_any_element(),
        Page::Reportes => reports::render(state, app_view, cx, app_cx).into_any_element(),
        Page::Configuracion => settings::render(state, app_view, cx, app_cx).into_any_element(),
    }
}
