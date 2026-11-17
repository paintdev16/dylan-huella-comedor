use gpui_kit::{
    App, AppContext as _, Context, Entity, FontWeight, IntoElement, ParentElement, SharedString,
    Styled, Window,
    assets::IconName as LucideIcon,
    component::{
        ActiveTheme as _, Disableable as _, Icon, Sizable as _, WindowExt as _,
        alert::Alert,
        button::{Button, ButtonVariants as _},
        empty::{Empty, EmptyDescription, EmptyHeader, EmptyMedia, EmptyMediaVariant, EmptyTitle},
        group_box::{GroupBox, GroupBoxVariants as _},
        h_flex,
        input::{Input, InputState},
        notification::Notification,
        scroll::ScrollableElement as _,
        spinner::Spinner,
        tag::Tag,
        v_flex,
    },
    div,
    prelude::FluentBuilder as _,
    px,
};

use crate::{
    components::common::{SemanticTone, info_panel, metric_card},
    layout::DesktopApp,
    mock_data::{CashMovementKind, parse_price_cents},
    state::{AppState, DataStatus, Page},
};

#[derive(Clone, Copy, PartialEq, Eq, Default)]
pub enum CashFilter {
    #[default]
    All,
    WithDebt,
    Settled,
    Audit,
}

impl CashFilter {
    pub fn label(self) -> &'static str {
        match self {
            Self::All => "Todos",
            Self::WithDebt => "Pagos en efectivo",
            Self::Settled => "Tickets físicos",
            Self::Audit => "Apertura y cierre",
        }
    }

    pub fn matches_kind(self, kind: CashMovementKind) -> bool {
        match self {
            Self::All => true,
            Self::WithDebt => kind == CashMovementKind::Payment,
            Self::Settled => kind == CashMovementKind::TicketCredit,
            Self::Audit => matches!(kind, CashMovementKind::Opening | CashMovementKind::Closing),
        }
    }
}

fn allowed(state: &AppState, permission: &str) -> bool {
    state.auth.user().is_some_and(|user| user.can(permission))
}

fn amount(cents: i64) -> String {
    let sign = if cents < 0 { "-" } else { "" };
    let abs = cents.abs();
    format!("{sign}S/ {}.{:02}", abs / 100, abs % 100)
}

fn format_decimal(cents: i64) -> String {
    format!("{}.{:02}", cents / 100, cents.abs() % 100)
}

fn text_cell(value: impl Into<SharedString>, width: f32) -> impl IntoElement {
    div()
        .w(px(width))
        .flex_shrink_0()
        .text_xs()
        .truncate()
        .child(value.into())
}

fn movement_tag(kind: CashMovementKind) -> Tag {
    let label = kind.label();
    match kind {
        CashMovementKind::Opening => Tag::info().outline().small().child(label),
        CashMovementKind::Closing => Tag::secondary().small().child(label),
        CashMovementKind::Payment => Tag::success().small().child(label),
        CashMovementKind::TicketCredit => Tag::info().small().child(label),
    }
}

impl DesktopApp {
    fn has_cash_permission(&self, permission: &str) -> bool {
        self.state.auth.can_view(Page::Caja) && allowed(&self.state, permission)
    }

    pub fn select_cash_pensioner(
        &mut self,
        id: u32,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.cash_selected_pensioner = Some(id);
        self.cash_filter = CashFilter::All;
        cx.notify();
    }

    pub fn open_ticket_config_dialog(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !self.has_cash_permission("payments.create") {
            window.push_notification(
                Notification::error("No tienes permiso para configurar caja."),
                cx,
            );
            return;
        }
        let current_ticket = match &self.state.data {
            DataStatus::Ready(d) => d.cash_register.ticket_value_cents,
            _ => 700,
        };
        let ticket_input = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder("7.00")
                .default_value(format_decimal(current_ticket))
        });
        let err_state: Entity<Option<String>> = cx.new(|_| None);
        let app = cx.entity().downgrade();

        window.open_dialog(cx, move |dialog, _, cx| {
            let t_view = ticket_input.clone();
            let t_submit = ticket_input.clone();
            let err_view = err_state.clone();
            let err_submit = err_state.clone();
            let app = app.clone();
            let current_err = err_view.read(cx).clone();

            dialog
                .title("Configurar valor de ticket físico")
                .child(
                    v_flex()
                        .gap_3()
                        .when_some(current_err, |this, err| {
                            this.child(Alert::error("ticket-cfg-err", err))
                        })
                        .child(
                            v_flex()
                                .gap_1()
                                .child(
                                    div()
                                        .text_sm()
                                        .font_weight(FontWeight::MEDIUM)
                                        .child("Valor por ticket (S/) *"),
                                )
                                .child(Input::new(&t_view)),
                        )
                        .child(
                            div()
                                .text_xs()
                                .text_color(cx.theme().muted_foreground)
                                .child("Valor predeterminado de prueba: S/ 7.00."),
                        ),
                )
                .footer(
                    h_flex()
                        .gap_3()
                        .justify_end()
                        .child(
                            Button::new("ticket-cfg-cancel")
                                .outline()
                                .label("Cancelar")
                                .on_click(|_, window, cx| window.close_dialog(cx)),
                        )
                        .child(
                            Button::new("ticket-cfg-save")
                                .primary()
                                .label("Guardar")
                                .on_click(move |_, window, cx| {
                                    let raw = t_submit.read(cx).value().to_string();
                                    match parse_price_cents(&raw) {
                                        Ok(cents) => {
                                            let _ = app.update(cx, |this, cx| {
                                                if let DataStatus::Ready(data) =
                                                    &mut this.state.data
                                                {
                                                    data.cash_register.ticket_value_cents = cents;
                                                }
                                                cx.notify();
                                            });
                                            err_submit.update(cx, |e, _| *e = None);
                                            window.close_dialog(cx);
                                            window.push_notification(
                                                Notification::success(
                                                    "Valor del ticket actualizado.",
                                                ),
                                                cx,
                                            );
                                        }
                                        Err(err) => {
                                            err_submit.update(cx, |e, cx| {
                                                *e = Some(err);
                                                cx.notify();
                                            });
                                        }
                                    }
                                }),
                        ),
                )
        });
    }

    pub fn open_cash_toggle_dialog(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !self.has_cash_permission("payments.create") {
            window.push_notification(
                Notification::error("No tienes permiso para abrir o cerrar caja."),
                cx,
            );
            return;
        }
        let DataStatus::Ready(data) = &self.state.data else {
            return;
        };
        let is_open = data.cash_register.is_open;
        let expected = data.cash_register.opening_cents + data.cash_inflow_cents();
        let opening_input = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder("100.00")
                .default_value("100.00")
        });
        let err_state: Entity<Option<String>> = cx.new(|_| None);
        let token = self.cash_confirm_token;
        let app = cx.entity().downgrade();

        window.open_dialog(cx, move |dialog, _, cx| {
            let o_view = opening_input.clone();
            let o_submit = opening_input.clone();
            let err_view = err_state.clone();
            let err_submit = err_state.clone();
            let app = app.clone();
            let current_err = err_view.read(cx).clone();

            dialog
                .title(if is_open {
                    "Confirmar cierre de caja"
                } else {
                    "Confirmar apertura de caja"
                })
                .child(
                    v_flex()
                        .gap_3()
                        .when_some(current_err, |this, err| {
                            this.child(Alert::error("cash-toggle-err", err))
                        })
                        .when(is_open, |this| {
                            this.child(Alert::info(
                                "cash-close-summary",
                                format!(
                                    "Saldo esperado en caja al cierre: {}. Esta operación bloqueará nuevos pagos hasta reabrir.",
                                    amount(expected)
                                ),
                            ))
                        })
                        .when(!is_open, |this| {
                            this.child(
                                v_flex()
                                    .gap_1()
                                    .child(
                                        div()
                                            .text_sm()
                                            .font_weight(FontWeight::MEDIUM)
                                            .child("Monto inicial de apertura (S/) *"),
                                    )
                                    .child(Input::new(&o_view)),
                            )
                        }),
                )
                .footer(
                    h_flex()
                        .gap_3()
                        .justify_end()
                        .child(
                            Button::new("cash-toggle-cancel")
                                .outline()
                                .label("Cancelar")
                                .on_click(|_, window, cx| window.close_dialog(cx)),
                        )
                        .child(
                            Button::new("cash-toggle-confirm")
                                .primary()
                                .label(if is_open { "Cerrar caja" } else { "Abrir caja" })
                                .on_click(move |_, window, cx| {
                                    let raw_open = o_submit.read(cx).value().to_string();
                                    let parsed_open = if is_open {
                                        Ok(0)
                                    } else if raw_open.trim() == "0" || raw_open.trim() == "0.00" {
                                        Ok(0)
                                    } else {
                                        parse_price_cents(&raw_open)
                                    };
                                    let open_cents = match parsed_open {
                                        Ok(c) => c,
                                        Err(err) => {
                                            err_submit.update(cx, |e, cx| {
                                                *e = Some(err);
                                                cx.notify();
                                            });
                                            return;
                                        }
                                    };

                                    let res = app.update(cx, |this, cx| {
                                        if this.cash_submitting {
                                            return Err("Operación en curso.".to_string());
                                        }
                                        this.cash_submitting = true;
                                        let operator = this.state.user.name.to_string();
                                        let date = this.state.dashboard_date;
                                        let out = match &mut this.state.data {
                                            DataStatus::Ready(data) => {
                                                if is_open {
                                                    data.close_cash_register(
                                                        date,
                                                        "20:00",
                                                        token,
                                                        &mut this.cash_processed_tokens,
                                                    )
                                                    .map(|(id, exp)| {
                                                        data.set_movement_operator(&id, &operator);
                                                        format!(
                                                            "Caja cerrada ({id}) · Arqueo: {}",
                                                            amount(exp)
                                                        )
                                                    })
                                                } else {
                                                    data.open_cash_register(
                                                        date,
                                                        "08:00",
                                                        open_cents,
                                                        token,
                                                        &mut this.cash_processed_tokens,
                                                    )
                                                    .map(|id| {
                                                        data.set_movement_operator(&id, &operator);
                                                        format!("Caja abierta ({id}).")
                                                    })
                                                }
                                            }
                                            _ => Err("Datos no disponibles.".into()),
                                        };
                                        this.cash_submitting = false;
                                        if out.is_ok() {
                                            this.cash_confirm_token =
                                                this.cash_confirm_token.wrapping_add(1);
                                            cx.notify();
                                        }
                                        out
                                    });

                                    match res {
                                        Ok(Ok(msg)) => {
                                            window.close_dialog(cx);
                                            window
                                                .push_notification(Notification::success(msg), cx);
                                        }
                                        Ok(Err(err)) => {
                                            window.close_dialog(cx);
                                            window.push_notification(Notification::error(err), cx);
                                        }
                                        Err(_) => {
                                            window.close_dialog(cx);
                                        }
                                    }
                                }),
                        ),
                )
        });
    }

    pub fn open_receipt_preview_dialog(
        &self,
        mov_id: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let DataStatus::Ready(data) = &self.state.data else {
            return;
        };
        let Some(mov) = data
            .cash_register
            .movements
            .iter()
            .find(|m| m.id == mov_id)
            .cloned()
        else {
            return;
        };
        let person_label = mov
            .pensioner_id
            .and_then(|pid| data.pensioners.iter().find(|p| p.id == pid))
            .map(|p| p.name())
            .unwrap_or_else(|| "Operación de caja".to_string());
        let remaining_balance = mov.pensioner_id.map(|pid| data.net_balance(pid));
        let dining_hall = self.preferences.dining_hall_name.clone();

        window.open_dialog(cx, move |dialog, _, cx| {
            let hall = dining_hall.clone();
            let person = person_label.clone();
            let mov_item = mov.clone();
            dialog
                .width(px(420.))
                .title(format!("Comprobante simulado · {}", mov_item.id))
                .child(
                    v_flex()
                        .gap_2p5()
                        .p_3()
                        .rounded(cx.theme().radius)
                        .border_1()
                        .border_color(cx.theme().border)
                        .bg(cx.theme().muted)
                        .child(
                            div()
                                .text_sm()
                                .font_weight(FontWeight::BOLD)
                                .child(hall),
                        )
                        .child(
                            div()
                                .text_xs()
                                .text_color(cx.theme().muted_foreground)
                                .child(format!(
                                    "Comprobante {} · {} {} · Responsable: {}",
                                    mov_item.id,
                                    mov_item.date.format("%d/%m/%Y"),
                                    mov_item.time,
                                    mov_item.operator_name
                                )),
                        )
                        .child(div().text_xs().child(format!("Titular: {person}")))
                        .child(
                            div()
                                .text_xs()
                                .child(format!("Concepto: {}", mov_item.description)),
                        )
                        .when_some(
                            mov_item.ticket_count.zip(mov_item.ticket_unit_cents),
                            |this, (cnt, unit)| {
                                this.child(div().text_xs().child(format!(
                                    "Detalle de tickets: {cnt} unidad(es) × {}",
                                    amount(unit)
                                )))
                            },
                        )
                        .when_some(mov_item.consumption_cents, |this, cons| {
                            this.child(
                                div()
                                    .text_xs()
                                    .child(format!("Consumo asociado: {}", amount(cons))),
                            )
                        })
                        .when_some(mov_item.resulting_balance_cents, |this, res_bal| {
                            this.child(div().text_xs().child(if res_bal < 0 {
                                format!("Saldo resultante: A favor {}", amount(-res_bal))
                            } else if res_bal > 0 {
                                format!("Saldo resultante: Deuda {}", amount(res_bal))
                            } else {
                                "Saldo resultante: S/ 0.00 (Al día)".to_string()
                            }))
                        })
                        .child(
                            h_flex()
                                .justify_between()
                                .pt_1()
                                .border_t_1()
                                .border_color(cx.theme().border)
                                .child(
                                    div()
                                        .text_xs()
                                        .font_weight(FontWeight::SEMIBOLD)
                                        .child("Importe:"),
                                )
                                .child(
                                    div()
                                        .text_sm()
                                        .font_weight(FontWeight::BOLD)
                                        .child(amount(mov_item.cents)),
                                ),
                        )
                        .when_some(remaining_balance, |this, bal| {
                            this.child(
                                div()
                                    .text_xs()
                                    .text_color(cx.theme().muted_foreground)
                                    .child(if bal < 0 {
                                        format!("Saldo actual: A favor {}", amount(-bal))
                                    } else {
                                        format!("Saldo pendiente actual: {}", amount(bal))
                                    }),
                            )
                        }),
                )
                .footer(
                    h_flex().justify_end().child(
                        Button::new("receipt-close")
                            .primary()
                            .label("Cerrar")
                            .on_click(|_, window, cx| window.close_dialog(cx)),
                    ),
                )
        });
    }
}

fn kpi_box(
    label: &'static str,
    value: String,
    detail: impl Into<SharedString>,
    icon: LucideIcon,
    tone: SemanticTone,
    app: &App,
) -> impl IntoElement {
    metric_card(label, value, Some(detail.into()), icon, tone, app)
}

pub fn render(
    state: &AppState,
    app_view: &DesktopApp,
    cx: &Context<DesktopApp>,
    app: &App,
) -> impl IntoElement {
    let theme = app.theme();
    let data = match &state.data {
        DataStatus::Ready(data) => data,
        DataStatus::Loading => {
            return h_flex()
                .gap_3()
                .items_center()
                .p_6()
                .rounded(theme.radius_lg)
                .border_1()
                .border_color(theme.border)
                .bg(theme.colors.list)
                .text_color(theme.muted_foreground)
                .child(Spinner::new().small())
                .child("Cargando módulo de caja…")
                .into_any_element();
        }
        DataStatus::Error(err) => {
            return Alert::error("cash-load-err", err.clone()).into_any_element();
        }
    };

    let can_operate = allowed(state, "payments.create");
    let is_open = data.cash_register.is_open;
    let ticket_unit = data.cash_register.ticket_value_cents;
    let total_debt: i64 = data.pensioners.iter().map(|p| data.balance(p.id)).sum();
    let total_credit: i64 = data
        .pensioners
        .iter()
        .map(|p| data.credit_balance(p.id))
        .sum();
    let expected_cash = data.cash_register.opening_cents + data.cash_inflow_cents();
    let (total_tickets_count, total_tickets_cents) = data.total_tickets();

    let selected_person = app_view
        .cash_selected_pensioner
        .and_then(|id| data.pensioners.iter().find(|p| p.id == id));

    let query = crate::pages::identification::normalize_search_text(
        &app_view.cash_search.read(app).value(),
    );

    let filtered_movements: Vec<_> = data
        .cash_register
        .movements
        .iter()
        .rev()
        .filter(|m| {
            if !app_view.cash_filter.matches_kind(m.kind) {
                return false;
            }
            if let Some(pid) = app_view.cash_selected_pensioner {
                if m.pensioner_id != Some(pid) {
                    return false;
                }
            }
            if query.is_empty() {
                return true;
            }
            let p_name = m
                .pensioner_id
                .and_then(|pid| data.pensioners.iter().find(|p| p.id == pid))
                .map(|p| format!("#{} {} {}", p.id, p.id, p.name()))
                .unwrap_or_default();
            let haystack = crate::pages::identification::normalize_search_text(&format!(
                "{} {} {} {} {}",
                m.id,
                m.kind.label(),
                m.description,
                m.operator_name,
                p_name
            ));
            query.split_whitespace().all(|tok| haystack.contains(tok))
        })
        .collect();

    v_flex()
        .gap_5()
        .child(
            h_flex()
                .justify_between()
                .gap_3()
                .flex_wrap()
                .child(
                    v_flex()
                        .gap_1()
                        .child(
                            div()
                                .text_2xl()
                                .font_weight(FontWeight::BOLD)
                                .child("Caja y control financiero"),
                        )
                        .child(
                            div()
                                .text_sm()
                                .text_color(theme.muted_foreground)
                                .child("Apertura y cierre de caja, arqueo, auditoría e historial de movimientos (la atención y entrega de tickets se centraliza en Atención automática)"),
                        ),
                )
                .child(
                    h_flex()
                        .gap_2()
                        .flex_wrap()
                        .child(if is_open {
                            Tag::success().small().child("Caja abierta")
                        } else {
                            Tag::danger().small().child("Caja cerrada")
                        })
                        .child(
                            Button::new("cash-ticket-cfg-btn")
                                .outline()
                                .small()
                                .disabled(!can_operate)
                                .label(format!("Ticket: {}", amount(ticket_unit)))
                                .on_click(cx.listener(|this, _, window, cx| {
                                    this.open_ticket_config_dialog(window, cx);
                                })),
                        )
                        .child(
                            Button::new("cash-open-close-btn")
                                .when(is_open, |b| b.outline())
                                .when(!is_open, |b| b.primary())
                                .small()
                                .disabled(!can_operate)
                                .label(if is_open { "Cerrar caja" } else { "Abrir caja" })
                                .on_click(cx.listener(|this, _, window, cx| {
                                    this.open_cash_toggle_dialog(window, cx);
                                })),
                        ),
                ),
        )
        .when(!is_open, |this| {
            this.child(
                Alert::warning(
                    "cash-closed-banner",
                    "La caja está cerrada. Abre la caja para habilitar el registro de tickets o cobros desde Atención automática.",
                )
                .title("Caja cerrada"),
            )
        })
        .child(
            v_flex()
                .gap_4()
                .child(
                    h_flex()
                        .gap_4()
                        .flex_wrap()
                        .child(kpi_box(
                            "Apertura de caja",
                            amount(data.cash_register.opening_cents),
                            "Fondo inicial de sesión",
                            LucideIcon::Wallet,
                            SemanticTone::Neutral,
                            app,
                        ))
                        .child(kpi_box(
                            "Efectivo ingresado",
                            amount(data.cash_inflow_cents()),
                            "Ingresos en efectivo",
                            LucideIcon::Wallet,
                            SemanticTone::Green,
                            app,
                        ))
                        .child(kpi_box(
                            "Tickets recibidos",
                            format!("{total_tickets_count} und."),
                            format!("Equivalente: {}", amount(total_tickets_cents)),
                            LucideIcon::Ticket,
                            SemanticTone::Blue,
                            app,
                        )),
                )
                .child(
                    h_flex()
                        .gap_4()
                        .flex_wrap()
                        .child(kpi_box(
                            "Saldo esperado en caja",
                            amount(expected_cash),
                            "Apertura + efectivo ingresado",
                            LucideIcon::ShieldCheck,
                            SemanticTone::Violet,
                            app,
                        ))
                        .child(kpi_box(
                            "Saldo a favor",
                            amount(total_credit),
                            "Excedentes acumulados de pensionistas",
                            LucideIcon::Wallet,
                            SemanticTone::SoftGreen,
                            app,
                        ))
                        .child(kpi_box(
                            "Deuda pendiente",
                            amount(total_debt),
                            "Por cobrar a pensionistas",
                            LucideIcon::Wallet,
                            SemanticTone::SoftRed,
                            app,
                        )),
                ),
        )
        .when_some(selected_person, |this, person| {
            let pid = person.id;
            let consumed = data.consumed_total(pid);
            let paid = data.paid_total(pid);
            let debt = data.balance(pid);
            let credit = data.credit_balance(pid);
            let (p_tkt_count, p_tkt_cents) = data.tickets_for_pensioner(pid);
            let tone = if credit > 0 {
                SemanticTone::SoftGreen
            } else if debt > 0 {
                SemanticTone::SoftRed
            } else {
                SemanticTone::Blue
            };
            this.child(
                info_panel(tone, app).child(
                    h_flex()
                        .justify_between()
                        .gap_3()
                        .flex_wrap()
                        .items_center()
                        .child(
                            v_flex()
                                .gap_1()
                                .child(
                                    h_flex()
                                        .gap_2()
                                        .items_center()
                                        .child(
                                            div()
                                                .text_base()
                                                .font_weight(FontWeight::BOLD)
                                                .child(format!(
                                                    "Movimientos de {} (#{pid})",
                                                    person.name()
                                                )),
                                        )
                                        .child(if credit > 0 {
                                            Tag::info()
                                                .small()
                                                .child(format!("A favor {}", amount(credit)))
                                        } else if debt > 0 {
                                            Tag::warning()
                                                .small()
                                                .child(format!("Deuda {}", amount(debt)))
                                        } else {
                                            Tag::success().outline().small().child("Al día")
                                        }),
                                )
                                .child(
                                    div()
                                        .text_xs()
                                        .text_color(theme.muted_foreground)
                                        .child(format!(
                                            "Consumido: {} · Abonado total: {} · Tickets entregados: {} und. ({})",
                                            amount(consumed),
                                            amount(paid),
                                            p_tkt_count,
                                            amount(p_tkt_cents)
                                        )),
                                ),
                        )
                        .child(
                            h_flex()
                                .gap_2()
                                .child(
                                    Button::new("cash-view-pensioner-detail")
                                        .outline()
                                        .xsmall()
                                        .icon(LucideIcon::Eye)
                                        .label("Ver ficha")
                                        .on_click(cx.listener(move |this, _, _, cx| {
                                            this.selected_pensioner = Some(pid);
                                            this.navigate_to(Page::Pensionistas, cx);
                                        })),
                                )
                                .child(
                                    Button::new("cash-clear-selected-pensioner")
                                        .ghost()
                                        .xsmall()
                                        .label("Ver todos los movimientos")
                                        .on_click(cx.listener(|this, _, _, cx| {
                                            this.cash_selected_pensioner = None;
                                            cx.notify();
                                        })),
                                ),
                        ),
                ),
            )
        })
        .child(
            GroupBox::new()
                .outline()
                .title("Historial y auditoría de movimientos de caja")
                .child(
                    v_flex()
                        .gap_4()
                        .child(
                            h_flex()
                                .justify_between()
                                .gap_3()
                                .flex_wrap()
                                .child(
                                    div().w(px(340.)).max_w_full().child(
                                        Input::new(&app_view.cash_search)
                                            .cleanable(true)
                                            .prefix(Icon::new(LucideIcon::Search).xsmall()),
                                    ),
                                )
                                .child(
                                    h_flex().gap_1p5().flex_wrap().children(
                                        [
                                            CashFilter::All,
                                            CashFilter::WithDebt,
                                            CashFilter::Settled,
                                            CashFilter::Audit,
                                        ]
                                        .into_iter()
                                        .map(|choice| {
                                            let lbl = choice.label();
                                            Button::new(format!("cash-filter-{lbl}"))
                                                .xsmall()
                                                .when(app_view.cash_filter == choice, |b| {
                                                    b.primary()
                                                })
                                                .when(app_view.cash_filter != choice, |b| {
                                                    b.outline()
                                                })
                                                .label(lbl)
                                                .on_click(cx.listener(move |this, _, _, cx| {
                                                    this.cash_filter = choice;
                                                    cx.notify();
                                                }))
                                        }),
                                    ),
                                ),
                        )
                        .child(if filtered_movements.is_empty() {
                            Empty::new()
                                .py_8()
                                .header(
                                    EmptyHeader::new()
                                        .media(
                                            EmptyMedia::new()
                                                .with_variant(EmptyMediaVariant::Icon)
                                                .child(Icon::new(LucideIcon::Wallet).size_6()),
                                        )
                                        .title(EmptyTitle::new().child("Sin movimientos"))
                                        .description(
                                            EmptyDescription::new().child(
                                                "No hay movimientos de caja que coincidan con los filtros seleccionados.",
                                            ),
                                        ),
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
                                        .min_w(px(880.))
                                        .child(
                                            h_flex()
                                                .gap_2()
                                                .px_3()
                                                .py_2p5()
                                                .bg(theme.colors.table_head)
                                                .text_xs()
                                                .font_weight(FontWeight::SEMIBOLD)
                                                .text_color(theme.muted_foreground)
                                                .child(text_cell("Movimiento", 95.))
                                                .child(text_cell("Fecha / Hora", 120.))
                                                .child(text_cell("Tipo", 130.))
                                                .child(text_cell("Pensionista", 185.))
                                                .child(text_cell("Concepto y detalle", 220.))
                                                .child(text_cell("Responsable", 120.))
                                                .child(text_cell("Importe", 95.))
                                                .child(text_cell("Auditoría", 90.)),
                                        )
                                        .children(filtered_movements.into_iter().map(|m| {
                                            let mov_id_btn = m.id.clone();
                                            let person_info = m.pensioner_id.and_then(|pid| {
                                                data.pensioners
                                                    .iter()
                                                    .find(|p| p.id == pid)
                                                    .map(|p| (p.id, p.name()))
                                            });
                                            let detail_text = match (
                                                m.ticket_count,
                                                m.ticket_unit_cents,
                                            ) {
                                                (Some(cnt), Some(unit)) => format!(
                                                    "{} ({cnt} tkt × {})",
                                                    m.description,
                                                    amount(unit)
                                                ),
                                                _ => m.description.clone(),
                                            };

                                            h_flex()
                                                .gap_2()
                                                .px_3()
                                                .py_2p5()
                                                .items_center()
                                                .border_t_1()
                                                .border_color(theme.border)
                                                .child(
                                                    div().w(px(95.)).flex_shrink_0().child(
                                                        Tag::secondary()
                                                            .small()
                                                            .child(m.id.clone()),
                                                    ),
                                                )
                                                .child(text_cell(
                                                    format!(
                                                        "{} {}",
                                                        m.date.format("%d/%m/%Y"),
                                                        m.time
                                                    ),
                                                    120.,
                                                ))
                                                .child(
                                                    div()
                                                        .w(px(130.))
                                                        .flex_shrink_0()
                                                        .child(movement_tag(m.kind)),
                                                )
                                                .child(
                                                    div().w(px(185.)).flex_shrink_0().child(
                                                        if let Some((pid, p_name)) = person_info {
                                                            Button::new(format!(
                                                                "cash-mov-person-{}-{pid}",
                                                                m.id
                                                            ))
                                                            .ghost()
                                                            .xsmall()
                                                            .label(format!("#{pid} {p_name}"))
                                                            .tooltip(
                                                                "Filtrar movimientos de este pensionista",
                                                            )
                                                            .on_click(cx.listener(
                                                                move |this, _, window, cx| {
                                                                    this.select_cash_pensioner(
                                                                        pid, window, cx,
                                                                    );
                                                                },
                                                            ))
                                                            .into_any_element()
                                                        } else {
                                                            div()
                                                                .text_xs()
                                                                .text_color(theme.muted_foreground)
                                                                .child("Caja general")
                                                                .into_any_element()
                                                        },
                                                    ),
                                                )
                                                .child(text_cell(detail_text, 220.))
                                                .child(text_cell(m.operator_name.clone(), 120.))
                                                .child(
                                                    div()
                                                        .w(px(95.))
                                                        .flex_shrink_0()
                                                        .text_xs()
                                                        .font_weight(FontWeight::SEMIBOLD)
                                                        .child(amount(m.cents)),
                                                )
                                                .child(
                                                    div().w(px(90.)).flex_shrink_0().child(
                                                        Button::new(format!(
                                                            "cash-receipt-{}",
                                                            m.id
                                                        ))
                                                        .outline()
                                                        .xsmall()
                                                        .icon(LucideIcon::FileText)
                                                        .label("Recibo")
                                                        .tooltip("Ver comprobante simulado")
                                                        .on_click(cx.listener(
                                                            move |this, _, window, cx| {
                                                                this.open_receipt_preview_dialog(
                                                                    &mov_id_btn,
                                                                    window,
                                                                    cx,
                                                                );
                                                            },
                                                        )),
                                                    ),
                                                )
                                        })),
                                )
                                .into_any_element()
                        }),
                ),
        )
        .into_any_element()
}
