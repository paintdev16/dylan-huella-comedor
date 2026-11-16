use gpui_kit::{
    App, Context, FontWeight, IntoElement, ParentElement, SharedString, Styled,
    assets::IconName as LucideIcon,
    component::{
        ActiveTheme as _, Disableable as _, Icon, Sizable as _,
        alert::Alert,
        button::{Button, ButtonVariants as _},
        empty::{Empty, EmptyDescription, EmptyHeader, EmptyMedia, EmptyMediaVariant, EmptyTitle},
        group_box::{GroupBox, GroupBoxVariants as _},
        h_flex,
        input::Input,
        scroll::ScrollableElement as _,
        spinner::Spinner,
        switch::Switch,
        tag::Tag,
        v_flex,
    },
    div,
    prelude::FluentBuilder as _,
    px,
};

use crate::{
    layout::DesktopApp,
    reports_service::{
        ConsumptionSortColumn, ConsumptionTypeFilter, DateRangePreset, DebtSortColumn,
        MockReportsDataSource, PaymentSortColumn, PensionerStatusFilter, ReportQueryFilter,
        ReportsDataSource as _, SortDirection, paginate,
    },
    state::{AppState, DataStatus, Page},
};

const PAGE_SIZE: usize = 6;

#[derive(Clone, Copy, PartialEq, Eq, Default)]
pub enum ReportTab {
    #[default]
    Consumptions,
    Payments,
    Debts,
    Summaries,
}

fn amount(cents: i64) -> String {
    let sign = if cents < 0 { "-" } else { "" };
    let abs = cents.abs();
    format!("{sign}S/ {}.{:02}", abs / 100, abs % 100)
}

fn text_cell(value: impl Into<SharedString>, width: f32) -> impl IntoElement {
    div()
        .w(px(width))
        .flex_shrink_0()
        .text_xs()
        .truncate()
        .child(value.into())
}

fn empty_state(icon: LucideIcon, title: &'static str, description: &'static str) -> impl IntoElement {
    Empty::new().py_8().header(
        EmptyHeader::new()
            .media(
                EmptyMedia::new()
                    .with_variant(EmptyMediaVariant::Icon)
                    .child(Icon::new(icon).size_6()),
            )
            .title(EmptyTitle::new().child(title))
            .description(EmptyDescription::new().child(description)),
    )
}

fn kpi_card(title: &'static str, value: String, subtitle: String, app: &App) -> impl IntoElement {
    let theme = app.theme();
    div()
        .flex_1()
        .min_w(px(175.))
        .p_3p5()
        .rounded(theme.radius_lg)
        .border_1()
        .border_color(theme.border)
        .bg(theme.colors.list)
        .child(
            v_flex()
                .gap_1()
                .child(div().text_xs().text_color(theme.muted_foreground).child(title))
                .child(div().text_xl().font_weight(FontWeight::BOLD).child(value))
                .child(div().text_xs().text_color(theme.muted_foreground).child(subtitle)),
        )
}

fn sort_header(
    id: &'static str,
    label: &'static str,
    width: f32,
    active: bool,
    dir: SortDirection,
    on_click: impl Fn(&gpui_kit::ClickEvent, &mut gpui_kit::Window, &mut App) + 'static,
) -> impl IntoElement {
    let text = if active {
        format!("{label} {}", dir.arrow())
    } else {
        label.to_string()
    };
    div().w(px(width)).flex_shrink_0().child(
        Button::new(id)
            .ghost()
            .xsmall()
            .label(text)
            .on_click(on_click),
    )
}

fn pagination_bar(
    prefix: &'static str,
    page: usize,
    total_pages: usize,
    total_rows: usize,
    cx: &Context<DesktopApp>,
    app: &App,
) -> impl IntoElement {
    let theme = app.theme();
    h_flex()
        .justify_between()
        .pt_2()
        .child(
            div()
                .text_xs()
                .text_color(theme.muted_foreground)
                .child(format!(
                    "Página {} de {} · {} registro(s)",
                    page + 1,
                    total_pages,
                    total_rows
                )),
        )
        .child(
            h_flex()
                .gap_2()
                .child(
                    Button::new(format!("{prefix}-prev"))
                        .outline()
                        .xsmall()
                        .disabled(page == 0)
                        .label("Anterior")
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.report_page = this.report_page.saturating_sub(1);
                            cx.notify();
                        })),
                )
                .child(
                    Button::new(format!("{prefix}-next"))
                        .outline()
                        .xsmall()
                        .disabled(page + 1 >= total_pages)
                        .label("Siguiente")
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.report_page = this.report_page.saturating_add(1);
                            cx.notify();
                        })),
                ),
        )
}

pub fn render(
    state: &AppState,
    app_view: &DesktopApp,
    cx: &Context<DesktopApp>,
    app: &App,
) -> impl IntoElement {
    let theme = app.theme();
    let header = v_flex()
        .gap_1()
        .child(
            div()
                .text_2xl()
                .font_weight(FontWeight::BOLD)
                .child("Historial y reportes"),
        )
        .child(
            div()
                .text_sm()
                .text_color(theme.muted_foreground)
                .child("Consultas operativas, resúmenes diarios/mensuales y capa lista para API Axum"),
        );

    let data = match &state.data {
        DataStatus::Ready(data) => data,
        DataStatus::Loading => {
            return v_flex()
                .gap_5()
                .child(header)
                .child(
                    h_flex()
                        .gap_3()
                        .items_center()
                        .p_6()
                        .rounded(theme.radius_lg)
                        .border_1()
                        .border_color(theme.border)
                        .bg(theme.colors.list)
                        .text_color(theme.muted_foreground)
                        .child(Spinner::new().small())
                        .child("Cargando reportes…"),
                )
                .into_any_element();
        }
        DataStatus::Error(err) => {
            return v_flex()
                .gap_4()
                .child(header)
                .child(Alert::error("reports-load-err", err.clone()))
                .child(
                    h_flex().child(
                        Button::new("reports-retry")
                            .outline()
                            .label("Reintentar")
                            .on_click(cx.listener(|this, _, _, cx| this.load_demo_data(cx))),
                    ),
                )
                .into_any_element();
        }
    };

    let (date_from, date_to) = app_view.report_date_preset.bounds(state.dashboard_date);
    let filter = ReportQueryFilter {
        date_from,
        date_to,
        pensioner_id: app_view.report_pensioner_id,
        consumption_type: app_view.report_type_filter,
        status: app_view.report_status_filter,
        only_third_party: app_view.report_only_third_party,
        search: app_view.report_search.read(app).value().to_string(),
    };

    let source = MockReportsDataSource::new(data);
    let mut snapshot = source.query_reports(&filter);

    // Ordenamiento según columna activa
    let dir = app_view.report_sort_dir;
    match app_view.report_tab {
        ReportTab::Consumptions => {
            snapshot.consumptions.sort_by(|a, b| {
                let ord = match app_view.report_consumption_sort {
                    ConsumptionSortColumn::Date => {
                        a.date.cmp(&b.date).then_with(|| a.time.cmp(&b.time))
                    }
                    ConsumptionSortColumn::Pensioner => {
                        a.pensioner_name.cmp(&b.pensioner_name)
                    }
                    ConsumptionSortColumn::Kind => a
                        .meal_label
                        .cmp(&b.meal_label)
                        .then_with(|| a.menu_kind_label.cmp(&b.menu_kind_label)),
                    ConsumptionSortColumn::Portions => a.portions.cmp(&b.portions),
                    ConsumptionSortColumn::Amount => a.cents.cmp(&b.cents),
                };
                if dir == SortDirection::Desc {
                    ord.reverse()
                } else {
                    ord
                }
            });
        }
        ReportTab::Payments => {
            snapshot.payments.sort_by(|a, b| {
                let ord = match app_view.report_payment_sort {
                    PaymentSortColumn::Date => {
                        a.date.cmp(&b.date).then_with(|| a.time.cmp(&b.time))
                    }
                    PaymentSortColumn::Pensioner => a.pensioner_name.cmp(&b.pensioner_name),
                    PaymentSortColumn::Kind => a.kind_label.cmp(&b.kind_label),
                    PaymentSortColumn::Amount => a.cents.cmp(&b.cents),
                };
                if dir == SortDirection::Desc {
                    ord.reverse()
                } else {
                    ord
                }
            });
        }
        ReportTab::Debts => {
            snapshot.debts.sort_by(|a, b| {
                let ord = match app_view.report_debt_sort {
                    DebtSortColumn::Balance => a.balance_cents.cmp(&b.balance_cents),
                    DebtSortColumn::Pensioner => a.pensioner_name.cmp(&b.pensioner_name),
                    DebtSortColumn::Consumed => a.consumed_cents.cmp(&b.consumed_cents),
                    DebtSortColumn::Paid => a.paid_cents.cmp(&b.paid_cents),
                };
                if dir == SortDirection::Desc {
                    ord.reverse()
                } else {
                    ord
                }
            });
        }
        ReportTab::Summaries => {}
    }

    let totals = &snapshot.totals;

    let selected_pensioner_label = app_view.report_pensioner_id.and_then(|pid| {
        data.pensioners
            .iter()
            .find(|p| p.id == pid)
            .map(|p| format!("#{} · {}", p.id, p.name()))
    });

    v_flex()
        .gap_5()
        .child(
            h_flex()
                .justify_between()
                .gap_3()
                .flex_wrap()
                .child(header)
                .when_some(selected_pensioner_label, |this, lbl| {
                    this.child(
                        h_flex()
                            .gap_2()
                            .child(Tag::info().small().child(format!("Filtrado: {lbl}")))
                            .child(
                                Button::new("report-clear-pensioner")
                                    .ghost()
                                    .xsmall()
                                    .label("Ver todos")
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.report_pensioner_id = None;
                                        this.report_page = 0;
                                        cx.notify();
                                    })),
                            ),
                    )
                }),
        )
        .child(
            h_flex()
                .gap_3()
                .flex_wrap()
                .child(kpi_card(
                    "Raciones entregadas",
                    totals.total_portions.to_string(),
                    format!(
                        "{} atención(es) · Desayunos: {} · Alm. Econ.: {} · Esp.: {}",
                        totals.total_attentions,
                        totals.breakfast_portions,
                        totals.economic_lunch_portions,
                        totals.special_portions
                    ),
                    app,
                ))
                .child(kpi_card(
                    "Importe consumido",
                    amount(totals.total_consumed_cents),
                    format!(
                        "Atenciones: {} · Terceros: {}",
                        totals.total_attentions, totals.third_party_deliveries
                    ),
                    app,
                ))
                .child(kpi_card(
                    "Pagos recibidos",
                    amount(totals.total_paid_cents),
                    format!("{} movimiento(s)", snapshot.payments.len()),
                    app,
                ))
                .child(kpi_card(
                    "Deuda pendiente",
                    amount(totals.total_debt_cents),
                    format!("{} pensionista(s)", snapshot.debts.len()),
                    app,
                )),
        )
        .child(
            GroupBox::new()
                .outline()
                .title("Filtros de consulta")
                .child(
                    v_flex()
                        .gap_3()
                        .child(
                            h_flex()
                                .gap_3()
                                .flex_wrap()
                                .child(
                                    div().w(px(300.)).max_w_full().child(
                                        Input::new(&app_view.report_search)
                                            .cleanable(true)
                                            .prefix(Icon::new(LucideIcon::Search).xsmall()),
                                    ),
                                )
                                .child(
                                    h_flex().gap_1p5().flex_wrap().children(
                                        [
                                            DateRangePreset::All,
                                            DateRangePreset::Today,
                                            DateRangePreset::Last7Days,
                                            DateRangePreset::CurrentMonth,
                                        ]
                                        .into_iter()
                                        .map(|preset| {
                                            Button::new(format!("rep-date-{}", preset.label()))
                                                .xsmall()
                                                .when(app_view.report_date_preset == preset, |b| b.primary())
                                                .when(app_view.report_date_preset != preset, |b| b.outline())
                                                .label(preset.label())
                                                .on_click(cx.listener(move |this, _, _, cx| {
                                                    this.report_date_preset = preset;
                                                    this.report_page = 0;
                                                    cx.notify();
                                                }))
                                        }),
                                    ),
                                ),
                        )
                        .child(
                            h_flex()
                                .gap_3()
                                .flex_wrap()
                                .justify_between()
                                .child(
                                    h_flex().gap_1p5().flex_wrap().children(
                                        [
                                            ConsumptionTypeFilter::All,
                                            ConsumptionTypeFilter::Breakfast,
                                            ConsumptionTypeFilter::EconomicLunch,
                                            ConsumptionTypeFilter::SpecialLunch,
                                        ]
                                        .into_iter()
                                        .map(|kind| {
                                            Button::new(format!("rep-kind-{}", kind.label()))
                                                .xsmall()
                                                .when(app_view.report_type_filter == kind, |b| b.primary())
                                                .when(app_view.report_type_filter != kind, |b| b.outline())
                                                .label(kind.label())
                                                .on_click(cx.listener(move |this, _, _, cx| {
                                                    this.report_type_filter = kind;
                                                    this.report_page = 0;
                                                    cx.notify();
                                                }))
                                        }),
                                    ),
                                )
                                .child(
                                    h_flex()
                                        .gap_3()
                                        .flex_wrap()
                                        .child(
                                            h_flex().gap_1p5().children(
                                                [
                                                    PensionerStatusFilter::All,
                                                    PensionerStatusFilter::Active,
                                                    PensionerStatusFilter::Inactive,
                                                ]
                                                .into_iter()
                                                .map(|st| {
                                                    Button::new(format!("rep-status-{}", st.label()))
                                                        .xsmall()
                                                        .when(app_view.report_status_filter == st, |b| b.primary())
                                                        .when(app_view.report_status_filter != st, |b| b.outline())
                                                        .label(st.label())
                                                        .on_click(cx.listener(move |this, _, _, cx| {
                                                            this.report_status_filter = st;
                                                            this.report_page = 0;
                                                            cx.notify();
                                                        }))
                                                }),
                                            ),
                                        )
                                        .child(
                                            h_flex()
                                                .gap_2()
                                                .child(
                                                    div()
                                                        .text_xs()
                                                        .text_color(theme.muted_foreground)
                                                        .child("Solo terceros"),
                                                )
                                                .child(
                                                    Switch::new("rep-third-party-only")
                                                        .small()
                                                        .checked(app_view.report_only_third_party)
                                                        .on_change(cx.listener(
                                                            |this, checked: &bool, _, cx| {
                                                                this.report_only_third_party =
                                                                    *checked;
                                                                this.report_page = 0;
                                                                cx.notify();
                                                            },
                                                        )),
                                                ),
                                        ),
                                ),
                        ),
                ),
        )
        .child(
            h_flex().gap_2().flex_wrap().children(
                [
                    (ReportTab::Consumptions, "Historial de consumos"),
                    (ReportTab::Payments, "Pagos y abonos"),
                    (ReportTab::Debts, "Deudas pendientes"),
                    (ReportTab::Summaries, "Resumen diario y mensual"),
                ]
                .into_iter()
                .map(|(tab, lbl)| {
                    Button::new(format!("rep-tab-{lbl}"))
                        .small()
                        .when(app_view.report_tab == tab, |b| b.primary())
                        .when(app_view.report_tab != tab, |b| b.outline())
                        .label(lbl)
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.report_tab = tab;
                            this.report_page = 0;
                            cx.notify();
                        }))
                }),
            ),
        )
        .child(match app_view.report_tab {
            ReportTab::Consumptions => {
                let (rows_slice, page, total_pages) =
                    paginate(&snapshot.consumptions, app_view.report_page, PAGE_SIZE);
                if snapshot.consumptions.is_empty() {
                    empty_state(
                        LucideIcon::FileChartColumn,
                        "Sin consumos",
                        "No hay consumos que coincidan con los filtros.",
                    )
                    .into_any_element()
                } else {
                    v_flex()
                        .gap_2()
                        .child(
                            div()
                                .rounded(theme.radius_lg)
                                .border_1()
                                .border_color(theme.border)
                                .bg(theme.colors.list)
                                .overflow_x_scrollbar()
                                .child(
                                    v_flex()
                                        .min_w(px(860.))
                                        .child(
                                            h_flex()
                                                .gap_2()
                                                .px_3()
                                                .py_2p5()
                                                .bg(theme.colors.table_head)
                                                .child(sort_header(
                                                    "sort-c-date",
                                                    "Fecha / Hora",
                                                    120.,
                                                    app_view.report_consumption_sort == ConsumptionSortColumn::Date,
                                                    dir,
                                                    cx.listener(|this, _, _, cx| {
                                                        this.toggle_consumption_sort(ConsumptionSortColumn::Date, cx);
                                                    }),
                                                ))
                                                .child(sort_header(
                                                    "sort-c-person",
                                                    "Pensionista",
                                                    175.,
                                                    app_view.report_consumption_sort == ConsumptionSortColumn::Pensioner,
                                                    dir,
                                                    cx.listener(|this, _, _, cx| {
                                                        this.toggle_consumption_sort(ConsumptionSortColumn::Pensioner, cx);
                                                    }),
                                                ))
                                                .child(sort_header(
                                                    "sort-c-kind",
                                                    "Tipo",
                                                    175.,
                                                    app_view.report_consumption_sort == ConsumptionSortColumn::Kind,
                                                    dir,
                                                    cx.listener(|this, _, _, cx| {
                                                        this.toggle_consumption_sort(ConsumptionSortColumn::Kind, cx);
                                                    }),
                                                ))
                                                .child(sort_header(
                                                    "sort-c-portions",
                                                    "Raciones",
                                                    75.,
                                                    app_view.report_consumption_sort == ConsumptionSortColumn::Portions,
                                                    dir,
                                                    cx.listener(|this, _, _, cx| {
                                                        this.toggle_consumption_sort(ConsumptionSortColumn::Portions, cx);
                                                    }),
                                                ))
                                                .child(sort_header(
                                                    "sort-c-amount",
                                                    "Importe",
                                                    85.,
                                                    app_view.report_consumption_sort == ConsumptionSortColumn::Amount,
                                                    dir,
                                                    cx.listener(|this, _, _, cx| {
                                                        this.toggle_consumption_sort(ConsumptionSortColumn::Amount, cx);
                                                    }),
                                                ))
                                                .child(text_cell("Entrega / Tercero", 150.)),
                                        )
                                        .children(rows_slice.iter().enumerate().map(|(idx, r)| {
                                            let pid = r.pensioner_id;
                                            let kind_display = if let Some(d) = &r.detail {
                                                format!("{} · {} ({})", r.meal_label, r.menu_kind_label, d)
                                            } else {
                                                format!("{} · {}", r.meal_label, r.menu_kind_label)
                                            };
                                            h_flex()
                                                .gap_2()
                                                .px_3()
                                                .py_2p5()
                                                .border_t_1()
                                                .border_color(theme.border)
                                                .child(text_cell(
                                                    format!("{} {}", r.date.format("%d/%m/%Y"), r.time),
                                                    120.,
                                                ))
                                                .child(
                                                    div().w(px(175.)).flex_shrink_0().child(
                                                        Button::new(format!("rep-pick-{idx}-{pid}"))
                                                            .ghost()
                                                            .xsmall()
                                                            .label(format!("#{pid} {}", r.pensioner_name))
                                                            .on_click(cx.listener(move |this, _, _, cx| {
                                                                this.report_pensioner_id = Some(pid);
                                                                this.report_page = 0;
                                                                cx.notify();
                                                            })),
                                                    ),
                                                )
                                                .child(text_cell(kind_display, 175.))
                                                .child(text_cell(r.portions.to_string(), 75.))
                                                .child(text_cell(amount(r.cents), 85.))
                                                .child(
                                                    div().w(px(150.)).flex_shrink_0().child(
                                                        if let Some(third) = &r.picked_up_by {
                                                            Tag::warning()
                                                                .small()
                                                                .child(format!("Tercero: {third}"))
                                                        } else {
                                                            Tag::secondary().small().child("Titular")
                                                        },
                                                    ),
                                                )
                                        })),
                                ),
                        )
                        .child(pagination_bar(
                            "rep-c",
                            page,
                            total_pages,
                            snapshot.consumptions.len(),
                            cx,
                            app,
                        ))
                        .into_any_element()
                }
            }
            ReportTab::Payments => {
                let (rows_slice, page, total_pages) =
                    paginate(&snapshot.payments, app_view.report_page, PAGE_SIZE);
                if snapshot.payments.is_empty() {
                    empty_state(
                        LucideIcon::Wallet,
                        "Sin pagos ni abonos",
                        "No hay pagos ni abonos que coincidan con los filtros.",
                    )
                    .into_any_element()
                } else {
                    v_flex()
                        .gap_2()
                        .child(
                            div()
                                .rounded(theme.radius_lg)
                                .border_1()
                                .border_color(theme.border)
                                .bg(theme.colors.list)
                                .overflow_x_scrollbar()
                                .child(
                                    v_flex()
                                        .min_w(px(740.))
                                        .child(
                                            h_flex()
                                                .gap_2()
                                                .px_3()
                                                .py_2p5()
                                                .bg(theme.colors.table_head)
                                                .child(text_cell("Movimiento", 95.))
                                                .child(sort_header(
                                                    "sort-p-date",
                                                    "Fecha / Hora",
                                                    130.,
                                                    app_view.report_payment_sort == PaymentSortColumn::Date,
                                                    dir,
                                                    cx.listener(|this, _, _, cx| {
                                                        this.toggle_payment_sort(PaymentSortColumn::Date, cx);
                                                    }),
                                                ))
                                                .child(sort_header(
                                                    "sort-p-person",
                                                    "Pensionista",
                                                    190.,
                                                    app_view.report_payment_sort == PaymentSortColumn::Pensioner,
                                                    dir,
                                                    cx.listener(|this, _, _, cx| {
                                                        this.toggle_payment_sort(PaymentSortColumn::Pensioner, cx);
                                                    }),
                                                ))
                                                .child(sort_header(
                                                    "sort-p-kind",
                                                    "Modalidad",
                                                    150.,
                                                    app_view.report_payment_sort == PaymentSortColumn::Kind,
                                                    dir,
                                                    cx.listener(|this, _, _, cx| {
                                                        this.toggle_payment_sort(PaymentSortColumn::Kind, cx);
                                                    }),
                                                ))
                                                .child(sort_header(
                                                    "sort-p-amount",
                                                    "Importe",
                                                    100.,
                                                    app_view.report_payment_sort == PaymentSortColumn::Amount,
                                                    dir,
                                                    cx.listener(|this, _, _, cx| {
                                                        this.toggle_payment_sort(PaymentSortColumn::Amount, cx);
                                                    }),
                                                )),
                                        )
                                        .children(rows_slice.iter().map(|r| {
                                            h_flex()
                                                .gap_2()
                                                .px_3()
                                                .py_2p5()
                                                .border_t_1()
                                                .border_color(theme.border)
                                                .child(div().w(px(95.)).flex_shrink_0().child(
                                                    Tag::secondary().small().child(r.movement_id.clone()),
                                                ))
                                                .child(text_cell(
                                                    format!("{} {}", r.date.format("%d/%m/%Y"), r.time),
                                                    130.,
                                                ))
                                                .child(text_cell(
                                                    format!("#{} {}", r.pensioner_id, r.pensioner_name),
                                                    190.,
                                                ))
                                                .child(text_cell(r.kind_label.clone(), 150.))
                                                .child(text_cell(amount(r.cents), 100.))
                                        })),
                                ),
                        )
                        .child(pagination_bar(
                            "rep-p",
                            page,
                            total_pages,
                            snapshot.payments.len(),
                            cx,
                            app,
                        ))
                        .into_any_element()
                }
            }
            ReportTab::Debts => {
                let (rows_slice, page, total_pages) =
                    paginate(&snapshot.debts, app_view.report_page, PAGE_SIZE);
                if snapshot.debts.is_empty() {
                    empty_state(
                        LucideIcon::Users,
                        "Sin pensionistas",
                        "No hay pensionistas que coincidan con los filtros.",
                    )
                    .into_any_element()
                } else {
                v_flex()
                    .gap_2()
                    .child(
                        div()
                            .rounded(theme.radius_lg)
                            .border_1()
                            .border_color(theme.border)
                            .bg(theme.colors.list)
                            .overflow_x_scrollbar()
                            .child(
                                v_flex()
                                    .min_w(px(760.))
                                    .child(
                                        h_flex()
                                            .gap_2()
                                            .px_3()
                                            .py_2p5()
                                            .bg(theme.colors.table_head)
                                            .child(sort_header(
                                                "sort-d-person",
                                                "Pensionista",
                                                200.,
                                                app_view.report_debt_sort == DebtSortColumn::Pensioner,
                                                dir,
                                                cx.listener(|this, _, _, cx| {
                                                    this.toggle_debt_sort(DebtSortColumn::Pensioner, cx);
                                                }),
                                            ))
                                            .child(text_cell("Estado", 85.))
                                            .child(text_cell("Raciones", 75.))
                                            .child(sort_header(
                                                "sort-d-consumed",
                                                "Consumido",
                                                105.,
                                                app_view.report_debt_sort == DebtSortColumn::Consumed,
                                                dir,
                                                cx.listener(|this, _, _, cx| {
                                                    this.toggle_debt_sort(DebtSortColumn::Consumed, cx);
                                                }),
                                            ))
                                            .child(sort_header(
                                                "sort-d-paid",
                                                "Abonado",
                                                105.,
                                                app_view.report_debt_sort == DebtSortColumn::Paid,
                                                dir,
                                                cx.listener(|this, _, _, cx| {
                                                    this.toggle_debt_sort(DebtSortColumn::Paid, cx);
                                                }),
                                            ))
                                            .child(sort_header(
                                                "sort-d-balance",
                                                "Saldo pendiente",
                                                120.,
                                                app_view.report_debt_sort == DebtSortColumn::Balance,
                                                dir,
                                                cx.listener(|this, _, _, cx| {
                                                    this.toggle_debt_sort(DebtSortColumn::Balance, cx);
                                                }),
                                            )),
                                    )
                                    .children(rows_slice.iter().map(|r| {
                                        let pid = r.pensioner_id;
                                        h_flex()
                                            .gap_2()
                                            .px_3()
                                            .py_2p5()
                                            .border_t_1()
                                            .border_color(theme.border)
                                            .child(text_cell(
                                                format!("#{pid} {}", r.pensioner_name),
                                                200.,
                                            ))
                                            .child(div().w(px(85.)).flex_shrink_0().child(
                                                if r.pensioner_active {
                                                    Tag::success().outline().small().child("Activo")
                                                } else {
                                                    Tag::secondary().small().child("Inactivo")
                                                },
                                            ))
                                            .child(text_cell(r.total_portions.to_string(), 75.))
                                            .child(text_cell(amount(r.consumed_cents), 105.))
                                            .child(text_cell(amount(r.paid_cents), 105.))
                                            .child(div().w(px(120.)).flex_shrink_0().child(
                                                if r.balance_cents > 0 {
                                                    Tag::warning().small().child(amount(r.balance_cents))
                                                } else {
                                                    Tag::success().outline().small().child("S/ 0.00")
                                                },
                                            ))
                                            .child(
                                                Button::new(format!("debt-go-cash-{pid}"))
                                                    .ghost()
                                                    .xsmall()
                                                    .label("Ir a Caja")
                                                    .on_click(cx.listener(move |this, _, window, cx| {
                                                        this.select_cash_pensioner(pid, window, cx);
                                                        this.navigate_to(Page::Caja, cx);
                                                    })),
                                            )
                                    })),
                            ),
                    )
                    .child(pagination_bar(
                        "rep-d",
                        page,
                        total_pages,
                        snapshot.debts.len(),
                        cx,
                        app,
                    ))
                    .into_any_element()
                }
            }
            ReportTab::Summaries => v_flex()
                .gap_4()
                .child(
                    GroupBox::new()
                        .outline()
                        .title("Resumen diario")
                        .child(if snapshot.daily_summary.is_empty() {
                            div()
                                .text_sm()
                                .text_color(theme.muted_foreground)
                                .child("Sin movimientos para resumir.")
                                .into_any_element()
                        } else {
                            v_flex()
                                .gap_2()
                                .children(snapshot.daily_summary.iter().map(|d| {
                                    h_flex()
                                        .justify_between()
                                        .gap_3()
                                        .flex_wrap()
                                        .py_1p5()
                                        .border_b_1()
                                        .border_color(theme.border)
                                        .child(
                                            v_flex()
                                                .gap_0p5()
                                                .child(
                                                    div()
                                                        .text_sm()
                                                        .font_weight(FontWeight::SEMIBOLD)
                                                        .child(d.period_label.clone()),
                                                )
                                                .child(
                                                    div()
                                                        .text_xs()
                                                        .text_color(theme.muted_foreground)
                                                        .child(format!(
                                                            "Desayunos: {} · Alm. Económicos: {} · Especiales: {} · Total raciones: {} · Terceros: {}",
                                                            d.breakfast_portions,
                                                            d.economic_lunch_portions,
                                                            d.special_portions,
                                                            d.total_portions,
                                                            d.third_party_deliveries
                                                        )),
                                                ),
                                        )
                                        .child(
                                            div()
                                                .text_xs()
                                                .font_weight(FontWeight::SEMIBOLD)
                                                .child(format!(
                                                    "Consumos: {} · Pagos: {}",
                                                    amount(d.consumed_cents),
                                                    amount(d.paid_cents)
                                                )),
                                        )
                                }))
                                .into_any_element()
                        }),
                )
                .child(
                    GroupBox::new()
                        .outline()
                        .title("Resumen mensual")
                        .child(if snapshot.monthly_summary.is_empty() {
                            div()
                                .text_sm()
                                .text_color(theme.muted_foreground)
                                .child("Sin movimientos mensuales para resumir.")
                                .into_any_element()
                        } else {
                            v_flex()
                                .gap_2()
                                .children(snapshot.monthly_summary.iter().map(|m| {
                                    h_flex()
                                        .justify_between()
                                        .gap_3()
                                        .flex_wrap()
                                        .py_1p5()
                                        .border_b_1()
                                        .border_color(theme.border)
                                        .child(
                                            v_flex()
                                                .gap_0p5()
                                                .child(
                                                    div()
                                                        .text_sm()
                                                        .font_weight(FontWeight::SEMIBOLD)
                                                        .child(format!("Mes {}", m.period_label)),
                                                )
                                                .child(
                                                    div()
                                                        .text_xs()
                                                        .text_color(theme.muted_foreground)
                                                        .child(format!(
                                                            "Desayunos: {} · Alm. Económicos: {} · Especiales: {} · Total raciones: {} · Terceros: {}",
                                                            m.breakfast_portions,
                                                            m.economic_lunch_portions,
                                                            m.special_portions,
                                                            m.total_portions,
                                                            m.third_party_deliveries
                                                        )),
                                                ),
                                        )
                                        .child(
                                            div()
                                                .text_xs()
                                                .font_weight(FontWeight::SEMIBOLD)
                                                .child(format!(
                                                    "Consumos: {} · Pagos: {}",
                                                    amount(m.consumed_cents),
                                                    amount(m.paid_cents)
                                                )),
                                        )
                                }))
                                .into_any_element()
                        }),
                )
                .into_any_element(),
        })
        .into_any_element()
}
