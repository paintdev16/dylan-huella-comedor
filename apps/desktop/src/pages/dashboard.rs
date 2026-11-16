use gpui_kit::{
    App, Context, FontWeight, IntoElement, ParentElement, Styled, div, px,
    assets::IconName,
    component::{ActiveTheme as _, Icon, Sizable as _, button::{Button, ButtonVariants as _}, h_flex, scroll::ScrollableElement as _, spinner::Spinner, v_flex},
};
use crate::{layout::DesktopApp, mock_data::DashboardSummary, state::{AppState, DataStatus}};

fn money(cents: i64) -> String { format!("S/ {}.{:02}", cents / 100, cents.abs() % 100) }

fn card(
    title: &'static str,
    value: String,
    detail: impl Into<gpui_kit::SharedString>,
    icon: IconName,
    cx: &App,
) -> impl IntoElement {
    let theme = cx.theme();
    v_flex().flex_1().min_w(px(200.)).gap_3().p_4().rounded(theme.radius_lg)
        .bg(theme.colors.list).border_1().border_color(theme.border)
        .child(h_flex().justify_between().gap_2()
            .child(div().text_sm().text_color(theme.muted_foreground).child(title))
            .child(Icon::new(icon).size_4().text_color(theme.primary)))
        .child(div().text_2xl().font_weight(FontWeight::BOLD).child(value))
        .child(div().text_xs().text_color(theme.muted_foreground).child(detail.into()))
}

fn cell(text: impl Into<gpui_kit::SharedString>, width: f32) -> impl IntoElement {
    div().w(px(width)).flex_shrink_0().text_sm().truncate().child(text.into())
}

fn table(summary: &DashboardSummary, cx: &App) -> impl IntoElement {
    let theme = cx.theme();
    let mut rows = v_flex().w_full();
    for row in summary.activity.iter().take(10) {
        rows = rows.child(h_flex().w_full().gap_4().px_4().py_3().border_t_1().border_color(theme.border)
            .child(cell(row.time.clone(), 64.))
            .child(cell(row.person.clone(), 180.))
            .child(cell(row.kind, 130.))
            .child(cell(row.portions.map(|n| n.to_string()).unwrap_or_else(|| "—".into()), 80.))
            .child(cell(money(row.cents), 110.)));
    }
    v_flex().gap_3()
        .child(div().text_lg().font_weight(FontWeight::SEMIBOLD).child("Actividad reciente"))
        .child(if summary.activity.is_empty() {
            div().p_6().rounded(theme.radius_lg).border_1().border_color(theme.border)
                .bg(theme.colors.list).text_color(theme.muted_foreground)
                .child("No hay consumos ni pagos registrados en esta fecha.").into_any_element()
        } else {
            div().rounded(theme.radius_lg).border_1().border_color(theme.border).bg(theme.colors.list)
                .overflow_x_scrollbar().child(v_flex().min_w(px(660.))
                    .child(h_flex().gap_4().px_4().py_3().text_color(theme.muted_foreground)
                        .child(cell("Hora", 64.)).child(cell("Pensionista", 180.))
                        .child(cell("Actividad", 130.)).child(cell("Raciones", 80.)).child(cell("Importe", 110.)))
                    .child(rows)).into_any_element()
        })
}

pub fn render(state: &AppState, cx: &Context<DesktopApp>, app_cx: &App) -> impl IntoElement {
    let theme = app_cx.theme();
    let date = state.dashboard_date;
    let body = match &state.data {
        DataStatus::Loading => h_flex()
            .gap_3()
            .items_center()
            .p_6()
            .rounded(theme.radius_lg)
            .border_1()
            .border_color(theme.border)
            .bg(theme.colors.list)
            .text_color(theme.muted_foreground)
            .child(Spinner::new().small())
            .child("Cargando datos del dashboard…")
            .into_any_element(),
        DataStatus::Error(message) => v_flex().gap_3().p_6()
            .child(div().text_color(theme.danger).child(format!("No se pudieron cargar los datos: {message}")))
            .child(Button::new("dashboard-retry").outline().label("Reintentar")
                .on_click(cx.listener(|this, _, _, cx| this.load_demo_data(cx)))).into_any_element(),
        DataStatus::Ready(data) => {
            let summary = data.summary(date);
            v_flex().gap_5()
                .child(div().text_sm().text_color(theme.muted_foreground)
                    .child("Datos de demostración · Las operaciones corresponden a la fecha seleccionada."))
                .child(h_flex().gap_4().flex_wrap()
                    .child(card("Pensionistas registrados", summary.registered.to_string(), "Total actual", IconName::Users, app_cx))
                    .child(card("Pensionistas activos", summary.active.to_string(), "Estado actual", IconName::Users, app_cx))
                    .child(card("Desayunos", summary.breakfasts.to_string(), format!("Atenciones · {} ración(es)", summary.breakfast_portions), IconName::Utensils, app_cx))
                    .child(card("Almuerzos", summary.lunches.to_string(), format!("Atenciones · {} ración(es)", summary.lunch_portions), IconName::Utensils, app_cx)))
                .child(h_flex().gap_4().flex_wrap()
                    .child(card("Raciones entregadas", summary.portions.to_string(), format!("En {} atención(es) del día", summary.attentions()), IconName::Utensils, app_cx))
                    .child(card("Importe consumido", money(summary.consumed_cents), "Consumos de la fecha", IconName::Wallet, app_cx))
                    .child(card("Pagos recibidos", money(summary.paid_cents), "Pagos de la fecha", IconName::Wallet, app_cx))
                    .child(card("Deuda pendiente", money(summary.debt_cents), "Acumulada hasta la fecha", IconName::Wallet, app_cx)))
                .child(table(&summary, app_cx)).into_any_element()
        },
    };
    v_flex().gap_5()
        .child(h_flex().justify_between().gap_4().flex_wrap()
            .child(v_flex().gap_1()
                .child(div().text_2xl().font_weight(FontWeight::BOLD).child("Dashboard"))
                .child(div().text_sm().text_color(theme.muted_foreground).child("Resumen de operaciones del comedor")))
            .child(h_flex().gap_2().items_center()
                .child(Button::new("dashboard-prev-date").outline().small().label("‹").tooltip("Día anterior")
                    .on_click(cx.listener(|this, _, _, cx| this.change_dashboard_date(-1, cx))))
                .child(div().text_sm().font_weight(FontWeight::MEDIUM).child(date.format("%d/%m/%Y").to_string()))
                .child(Button::new("dashboard-next-date").outline().small().label("›").tooltip("Día siguiente")
                    .on_click(cx.listener(|this, _, _, cx| this.change_dashboard_date(1, cx))))
                .child(Button::new("dashboard-today").ghost().small().label("Hoy")
                    .on_click(cx.listener(|this, _, _, cx| this.dashboard_today(cx))))))
        .child(body)
}
