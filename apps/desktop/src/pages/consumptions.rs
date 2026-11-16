use crate::{
    components::common::{SemanticTone, metric_card},
    layout::DesktopApp,
    mock_data,
    state::{AppState, DataStatus, Page},
};
use gpui_kit::{
    assets::IconName as LucideIcon,
    component::{
        button::{Button, ButtonVariants as _},
        h_flex,
        scroll::ScrollableElement as _,
        v_flex, ActiveTheme as _, Sizable as _,
    },
    div,
    prelude::FluentBuilder as _,
    px, App, Context, FontWeight, IntoElement, ParentElement, Styled,
};

fn money(cents: i64) -> String {
    format!("S/ {}.{:02}", cents / 100, cents.abs() % 100)
}
fn column(value: impl Into<gpui_kit::SharedString>, width: f32) -> impl IntoElement {
    div()
        .w(px(width))
        .flex_shrink_0()
        .text_sm()
        .truncate()
        .child(value.into())
}
fn summary(
    label: &'static str,
    value: String,
    detail: &'static str,
    icon: LucideIcon,
    tone: SemanticTone,
    app: &App,
) -> impl IntoElement {
    metric_card(label, value, Some(detail.into()), icon, tone, app)
}

pub fn render(state: &AppState, cx: &Context<DesktopApp>, app: &App) -> impl IntoElement {
    let theme = app.theme();
    let can_create = state
        .auth
        .user()
        .is_some_and(|u| u.can("consumptions.create"));
    let can_reports = state.auth.can_view(Page::Reportes);
    let date = mock_data::today();
    let body = match &state.data {
        DataStatus::Loading => div()
            .p_6()
            .text_color(theme.muted_foreground)
            .child("Cargando consumos…")
            .into_any_element(),
        DataStatus::Error(error) => v_flex()
            .gap_3()
            .p_6()
            .child(
                div()
                    .text_color(theme.danger)
                    .child(format!("No se pudieron cargar los consumos: {error}")),
            )
            .child(
                Button::new("consumptions-retry")
                    .outline()
                    .label("Reintentar")
                    .on_click(cx.listener(|this, _, _, cx| this.load_demo_data(cx))),
            )
            .into_any_element(),
        DataStatus::Ready(data) => {
            let mut records: Vec<_> = data
                .consumptions
                .iter()
                .filter(|c| c.date == date)
                .collect();
            records.sort_by(|a, b| b.time.cmp(&a.time));
            let total_cents: i64 = records.iter().map(|c| c.cents).sum();
            let portions: u32 = records.iter().map(|c| c.portions).sum();
            let mut rows = v_flex();
            for item in records.iter().take(20) {
                let name = data
                    .pensioners
                    .iter()
                    .find(|p| p.id == item.pensioner_id)
                    .map(|p| p.name())
                    .unwrap_or_else(|| "Pensionista desconocido".into());
                let detail = match (&item.detail, &item.picked_up_by) {
                    (Some(d), Some(p)) => format!("{d} · Recogido por {p}"),
                    (Some(d), None) => d.clone(),
                    (None, Some(p)) => format!("Recogido por {p}"),
                    _ => "—".to_string(),
                };
                rows = rows.child(
                    h_flex()
                        .gap_3()
                        .px_4()
                        .py_3()
                        .border_t_1()
                        .border_color(theme.border)
                        .child(column(item.time.clone(), 58.))
                        .child(column(name, 170.))
                        .child(column(
                            format!("{} · {}", item.meal.label(), item.menu_kind.label()),
                            160.,
                        ))
                        .child(column(item.portions.to_string(), 75.))
                        .child(column(money(item.cents), 90.))
                        .child(column(detail, 220.)),
                );
            }
            v_flex()
                .gap_5()
                .child(
                    h_flex()
                        .gap_4()
                        .flex_wrap()
                        .child(summary(
                            "Consumos registrados",
                            records.len().to_string(),
                            "Atenciones registradas hoy",
                            LucideIcon::Utensils,
                            SemanticTone::Blue,
                            app,
                        ))
                        .child(summary(
                            "Raciones entregadas",
                            portions.to_string(),
                            "Unidades entregadas hoy",
                            LucideIcon::Utensils,
                            SemanticTone::Orange,
                            app,
                        ))
                        .child(summary(
                            "Importe consumido",
                            money(total_cents),
                            "Total acumulado hoy",
                            LucideIcon::Wallet,
                            SemanticTone::Neutral,
                            app,
                        )),
                )
                .child(
                    v_flex()
                        .gap_3()
                        .child(
                            h_flex()
                                .justify_between()
                                .gap_2()
                                .flex_wrap()
                                .child(
                                    div()
                                        .text_lg()
                                        .font_weight(FontWeight::SEMIBOLD)
                                        .child("Consumos de hoy"),
                                )
                                .child(div().text_sm().text_color(theme.muted_foreground).child(
                                    if records.len() > 20 {
                                        "Mostrando los 20 más recientes"
                                    } else {
                                        "Ordenados por hora"
                                    },
                                )),
                        )
                        .child(if records.is_empty() {
                            v_flex()
                                .gap_2()
                                .p_6()
                                .rounded(theme.radius_lg)
                                .border_1()
                                .border_color(theme.border)
                                .bg(theme.colors.list)
                                .child(
                                    div()
                                        .font_weight(FontWeight::SEMIBOLD)
                                        .child("Todavía no hay consumos"),
                                )
                                .child(
                                    div()
                                        .text_sm()
                                        .text_color(theme.muted_foreground)
                                        .child("Las atenciones registradas hoy aparecerán aquí."),
                                )
                                .into_any_element()
                        } else {
                            div()
                                .rounded(theme.radius_lg)
                                .border_1()
                                .border_color(theme.border)
                                .bg(theme.colors.list)
                                .overflow_x_scrollbar()
                                .child(
                                    v_flex()
                                        .min_w(px(850.))
                                        .child(
                                            h_flex()
                                                .gap_3()
                                                .px_4()
                                                .py_3()
                                                .text_color(theme.muted_foreground)
                                                .child(column("Hora", 58.))
                                                .child(column("Pensionista", 170.))
                                                .child(column("Servicio", 160.))
                                                .child(column("Raciones", 75.))
                                                .child(column("Importe", 90.))
                                                .child(column("Detalle", 220.)),
                                        )
                                        .child(rows),
                                )
                                .into_any_element()
                        }),
                )
                .into_any_element()
        }
    };
    v_flex()
        .gap_5()
        .child(
            h_flex()
                .justify_between()
                .gap_4()
                .flex_wrap()
                .child(
                    v_flex()
                        .gap_1()
                        .child(
                            div()
                                .text_2xl()
                                .font_weight(FontWeight::BOLD)
                                .child("Consumos"),
                        )
                        .child(
                            div()
                                .text_sm()
                                .text_color(theme.muted_foreground)
                                .child(format!("Actividad del {}", date.format("%d/%m/%Y"))),
                        ),
                )
                .child(
                    h_flex()
                        .gap_2()
                        .flex_wrap()
                        .when(can_reports, |this| {
                            this.child(
                                Button::new("consumptions-reports")
                                    .outline()
                                    .small()
                                    .label("Ver reportes")
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.navigate_to(Page::Reportes, cx)
                                    })),
                            )
                        })
                        .when(can_create, |this| {
                            this.child(
                                Button::new("consumptions-new")
                                    .primary()
                                    .small()
                                    .icon(LucideIcon::Search)
                                    .label("Nueva atención")
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        this.focus_global_search(window, cx)
                                    })),
                            )
                        }),
                ),
        )
        .child(body)
}
