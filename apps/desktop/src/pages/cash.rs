use gpui_kit::{
    App, AppContext as _, Context, Entity, FontWeight, IntoElement, ParentElement, Styled, Window,
    assets::IconName as LucideIcon,
    component::{
        ActiveTheme as _, Disableable as _, Icon, Sizable as _, WindowExt as _,
        alert::Alert,
        button::{Button, ButtonVariants as _},
        group_box::{GroupBox, GroupBoxVariants as _},
        h_flex,
        input::{Input, InputState},
        notification::Notification,
        tag::Tag,
        v_flex,
    },
    div,
    prelude::FluentBuilder as _,
    px,
};

use crate::{
    components::common::{SemanticTone, metric_card},
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
}

#[derive(Clone, Copy, PartialEq, Eq, Default)]
pub enum CashOpKind {
    #[default]
    Payment,
    Tickets,
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

    pub fn select_cash_pensioner(&mut self, id: u32, window: &mut Window, cx: &mut Context<Self>) {
        self.cash_selected_pensioner = Some(id);
        let debt = match &self.state.data {
            DataStatus::Ready(data) => data.balance(id),
            _ => 0,
        };
        let default_text = if debt > 0 {
            format_decimal(debt)
        } else {
            String::new()
        };
        self.cash_amount_input.update(cx, |input, cx| {
            input.set_value(default_text, window, cx);
        });
        self.cash_ticket_count = 1;
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
                                .child(div().text_sm().font_weight(FontWeight::MEDIUM).child("Valor por ticket (S/) *"))
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
                                                if let DataStatus::Ready(data) = &mut this.state.data {
                                                    data.cash_register.ticket_value_cents = cents;
                                                }
                                                cx.notify();
                                            });
                                            err_submit.update(cx, |e, _| *e = None);
                                            window.close_dialog(cx);
                                            window.push_notification(
                                                Notification::success("Valor del ticket actualizado."),
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
                            this.child(
                                Alert::info(
                                    "cash-close-summary",
                                    format!(
                                        "Saldo esperado en caja al cierre: {}. Esta operación bloqueará nuevos pagos hasta reabrir.",
                                        amount(expected)
                                    ),
                                ),
                            )
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
                                            window.push_notification(Notification::success(msg), cx);
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

    pub fn open_cash_operation_dialog(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.cash_submitting {
            return;
        }
        if !self.has_cash_permission("payments.create") {
            window.push_notification(
                Notification::error("No tienes permiso para registrar operaciones de caja."),
                cx,
            );
            return;
        }
        let Some(pid) = self.cash_selected_pensioner else {
            window.push_notification(
                Notification::error("Selecciona primero un pensionista."),
                cx,
            );
            return;
        };
        let DataStatus::Ready(data) = &self.state.data else {
            return;
        };
        if !data.cash_register.is_open {
            window.push_notification(
                Notification::error("La caja está cerrada. Ábrela antes de registrar movimientos."),
                cx,
            );
            return;
        }
        let Some(person) = data.pensioners.iter().find(|p| p.id == pid) else {
            window.push_notification(Notification::error("Pensionista no encontrado."), cx);
            return;
        };
        let debt = data.balance(pid);
        let net_balance = data.net_balance(pid);
        let op_kind = self.cash_op_kind;
        if debt == 0 && op_kind != CashOpKind::Tickets {
            window.push_notification(
                Notification::error("El pensionista no tiene deuda pendiente."),
                cx,
            );
            return;
        }
        let ticket_count = self.cash_ticket_count.max(1);
        let ticket_unit = data.cash_register.ticket_value_cents;

        let (op_cents, op_summary) = match op_kind {
            CashOpKind::Payment => {
                let raw = self.cash_amount_input.read(cx).value().to_string();
                let cents = match parse_price_cents(&raw) {
                    Ok(c) => c,
                    Err(err) => {
                        window.push_notification(Notification::error(err), cx);
                        return;
                    }
                };
                if cents > debt {
                    window.push_notification(
                        Notification::error(format!(
                            "El pago ({}) no puede superar la deuda pendiente ({}).",
                            amount(cents),
                            amount(debt)
                        )),
                        cx,
                    );
                    return;
                }
                let kind_lbl = if cents == debt {
                    "Pago completo"
                } else {
                    "Pago parcial"
                };
                (cents, format!("{kind_lbl} de {}", amount(cents)))
            }
            CashOpKind::Tickets => {
                let total = ticket_unit.saturating_mul(ticket_count as i64);
                (
                    total,
                    format!(
                        "{} ticket(s) físico(s) × {} = {}",
                        ticket_count,
                        amount(ticket_unit),
                        amount(total)
                    ),
                )
            }
        };

        let person_name = person.name();
        let new_net = net_balance - op_cents;
        let current_balance_label = if net_balance < 0 {
            format!("Saldo a favor actual: {}", amount(-net_balance))
        } else {
            format!("Deuda actual: {}", amount(debt))
        };
        let new_balance_label = if new_net < 0 {
            format!("Saldo a favor resultante: {}", amount(-new_net))
        } else if new_net > 0 {
            format!("Saldo restante: {}", amount(new_net))
        } else {
            "Saldo restante: S/ 0.00 (Al día)".to_string()
        };
        let operator_preview = self.state.user.name.clone();
        let dining_hall = self.preferences.dining_hall_name.clone();
        let receipt_date = self.state.dashboard_date.format("%d/%m/%Y").to_string();
        let token = self.cash_confirm_token;
        let app = cx.entity().downgrade();

        window.open_dialog(cx, move |dialog, _, cx| {
            let app = app.clone();
            let summary_text = op_summary.clone();
            let p_name = person_name.clone();
            let op_name = operator_preview.clone();
            let hall_name = dining_hall.clone();
            let r_date = receipt_date.clone();

            dialog
                .title("Confirmar operación de caja")
                .child(
                    v_flex()
                        .gap_3()
                        .child(
                            div()
                                .text_sm()
                                .child(format!("Pensionista: {p_name}")),
                        )
                        .child(div().text_sm().font_weight(FontWeight::SEMIBOLD).child(summary_text.clone()))
                        .child(
                            h_flex()
                                .justify_between()
                                .p_3()
                                .rounded(cx.theme().radius)
                                .border_1()
                                .border_color(cx.theme().border)
                                .bg(cx.theme().muted)
                                .child(
                                    div()
                                        .text_xs()
                                        .text_color(cx.theme().muted_foreground)
                                        .child(current_balance_label.clone()),
                                )
                                .child(
                                    div()
                                        .text_sm()
                                        .font_weight(FontWeight::BOLD)
                                        .child(new_balance_label.clone()),
                                ),
                        )
                        .child(
                            v_flex()
                                .gap_1()
                                .p_2p5()
                                .rounded(cx.theme().radius)
                                .border_1()
                                .border_color(cx.theme().border)
                                .child(
                                    div()
                                        .text_xs()
                                        .font_weight(FontWeight::BOLD)
                                        .child(format!("Vista previa de comprobante · {hall_name}")),
                                )
                                .child(
                                    div()
                                        .text_xs()
                                        .text_color(cx.theme().muted_foreground)
                                        .child(format!("Fecha: {r_date} · Responsable: {op_name}")),
                                )
                                .child(
                                    div()
                                        .text_xs()
                                        .child(format!("{summary_text} · {new_balance_label}")),
                                ),
                        ),
                )
                .footer(
                    h_flex()
                        .gap_3()
                        .justify_end()
                        .child(
                            Button::new("cash-op-cancel")
                                .outline()
                                .label("Cancelar")
                                .on_click(|_, window, cx| window.close_dialog(cx)),
                        )
                        .child(
                            Button::new("cash-op-confirm")
                                .primary()
                                .label("Confirmar movimiento")
                                .on_click(move |_, window, cx| {
                                    let res = app.update(cx, |this, cx| {
                                        if this.cash_submitting {
                                            return Err(
                                                "Movimiento en curso; espera un momento."
                                                    .to_string(),
                                            );
                                        }
                                        this.cash_submitting = true;
                                        let operator = this.state.user.name.to_string();
                                        let now = chrono::Local::now();
                                        let date = this.state.dashboard_date;
                                        let time = now.format("%H:%M").to_string();
                                        let out = match &mut this.state.data {
                                            DataStatus::Ready(data) => {
                                                let mov_res = match op_kind {
                                                    CashOpKind::Payment => data.record_payment(
                                                        date,
                                                        &time,
                                                        pid,
                                                        op_cents,
                                                        token,
                                                        &mut this.cash_processed_tokens,
                                                    ),
                                                    CashOpKind::Tickets => data.record_ticket_credit(
                                                        date,
                                                        &time,
                                                        pid,
                                                        ticket_count,
                                                        token,
                                                        &mut this.cash_processed_tokens,
                                                    ),
                                                };
                                                if let Ok(ref mov_id) = mov_res {
                                                    data.set_movement_operator(mov_id, &operator);
                                                }
                                                mov_res
                                            }
                                            _ => Err("Los datos no están disponibles.".into()),
                                        };
                                        this.cash_submitting = false;
                                        if out.is_ok() {
                                            this.cash_confirm_token =
                                                this.cash_confirm_token.wrapping_add(1);
                                            this.cash_ticket_count = 1;
                                            let next_debt = match &this.state.data {
                                                DataStatus::Ready(d) => d.balance(pid),
                                                _ => 0,
                                            };
                                            let next_text = if next_debt > 0 {
                                                format_decimal(next_debt)
                                            } else {
                                                String::new()
                                            };
                                            this.cash_amount_input.update(cx, |input, cx| {
                                                input.set_value(next_text, window, cx);
                                            });
                                            cx.notify();
                                        }
                                        out
                                    });

                                    match res {
                                        Ok(Ok(mov_id)) => {
                                            window.close_dialog(cx);
                                            window.push_notification(
                                                Notification::success(format!(
                                                    "Movimiento {mov_id} registrado."
                                                )),
                                                cx,
                                            );
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
        let remaining_balance = mov.pensioner_id.map(|pid| data.balance(pid));
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
                        .child(div().text_xs().child(format!("Concepto: {}", mov_item.description)))
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
                            this.child(div().text_xs().child(format!(
                                "Consumo asociado: {}",
                                amount(cons)
                            )))
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
                                .child(div().text_xs().font_weight(FontWeight::SEMIBOLD).child("Importe:"))
                                .child(div().text_sm().font_weight(FontWeight::BOLD).child(amount(mov_item.cents))),
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
                    h_flex()
                        .justify_end()
                        .child(
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
    detail: impl Into<gpui_kit::SharedString>,
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
        DataStatus::Loading => return div().child("Cargando módulo de caja…").into_any_element(),
        DataStatus::Error(err) => {
            return Alert::error("cash-load-err", err.clone()).into_any_element();
        }
    };

    let can_operate = allowed(state, "payments.create");
    let is_open = data.cash_register.is_open;
    let ticket_unit = data.cash_register.ticket_value_cents;
    let total_debt: i64 = data.pensioners.iter().map(|p| data.balance(p.id)).sum();
    let total_credit: i64 = data.pensioners.iter().map(|p| data.credit_balance(p.id)).sum();
    let expected_cash = data.cash_register.opening_cents + data.cash_inflow_cents();
    let (total_tickets_count, total_tickets_cents) = data.total_tickets();

    let query = crate::pages::identification::normalize_search_text(
        &app_view.cash_search.read(app).value(),
    );
    let mut filtered_pensioners: Vec<_> = data
        .pensioners
        .iter()
        .filter(|p| {
            let bal = data.balance(p.id);
            let matches_filter = match app_view.cash_filter {
                CashFilter::All => true,
                CashFilter::WithDebt => bal > 0,
                CashFilter::Settled => bal <= 0,
            };
            let matches_query = if query.is_empty() {
                true
            } else {
                let haystack = crate::pages::identification::normalize_search_text(&format!(
                    "#{} {} {} {}",
                    p.id,
                    p.id,
                    p.name(),
                    p.phone.as_deref().unwrap_or("")
                ));
                query.split_whitespace().all(|tok| haystack.contains(tok))
            };
            matches_filter && matches_query
        })
        .collect();
    filtered_pensioners.sort_by_key(|p| (std::cmp::Reverse(data.balance(p.id)), p.id));

    let selected_person = app_view
        .cash_selected_pensioner
        .and_then(|id| data.pensioners.iter().find(|p| p.id == id));

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
                                .child("Caja, pagos y deudas"),
                        )
                        .child(
                            div()
                                .text_sm()
                                .text_color(theme.muted_foreground)
                                .child("Pagos completos o parciales, tickets físicos y arqueo simulado"),
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
                    "La caja está cerrada. Abre la caja para registrar pagos o tickets.",
                )
                .title("Caja cerrada"),
            )
        })
        .child(
            h_flex()
                .gap_3()
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
                    "Cobros en efectivo",
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
                ))
                .child(kpi_box(
                    "Saldo esperado en caja",
                    amount(expected_cash),
                    "Apertura + efectivo",
                    LucideIcon::ShieldCheck,
                    SemanticTone::Violet,
                    app,
                ))
                .child(kpi_box(
                    "Saldo total a favor",
                    amount(total_credit),
                    "Excedentes de pensionistas",
                    LucideIcon::Wallet,
                    SemanticTone::SoftGreen,
                    app,
                ))
                .child(kpi_box(
                    "Deuda total pendiente",
                    amount(total_debt),
                    "Por cobrar a pensionistas",
                    LucideIcon::Wallet,
                    SemanticTone::SoftRed,
                    app,
                )),
        )
        .child(
            h_flex()
                .gap_5()
                .items_start()
                .flex_wrap()
                .child(
                    GroupBox::new()
                        .outline()
                        .flex_1()
                        .min_w(px(340.))
                        .title("Pensionistas y saldos")
                        .child(
                            v_flex()
                                .gap_3()
                                .child(
                                    Input::new(&app_view.cash_search)
                                        .cleanable(true)
                                        .prefix(Icon::new(LucideIcon::Search).xsmall()),
                                )
                                .child(
                                    h_flex().gap_2().flex_wrap().children(
                                        [
                                            (CashFilter::All, "Todos"),
                                            (CashFilter::WithDebt, "Con deuda"),
                                            (CashFilter::Settled, "Al día"),
                                        ]
                                        .into_iter()
                                        .map(|(choice, lbl)| {
                                            Button::new(format!("cash-filter-{lbl}"))
                                                .small()
                                                .when(app_view.cash_filter == choice, |b| b.primary())
                                                .when(app_view.cash_filter != choice, |b| b.outline())
                                                .label(lbl)
                                                .on_click(cx.listener(move |this, _, _, cx| {
                                                    this.cash_filter = choice;
                                                    cx.notify();
                                                }))
                                        }),
                                    ),
                                )
                                .child(
                                    v_flex()
                                        .gap_2()
                                        .children(filtered_pensioners.into_iter().map(|p| {
                                            let pid = p.id;
                                            let consumed = data.consumed_total(pid);
                                            let bal = data.net_balance(pid);
                                            let is_sel = app_view.cash_selected_pensioner == Some(pid);
                                            h_flex()
                                                .justify_between()
                                                .gap_2()
                                                .p_2p5()
                                                .rounded(theme.radius)
                                                .border_1()
                                                .border_color(if is_sel {
                                                    theme.primary
                                                } else {
                                                    theme.border
                                                })
                                                .child(
                                                    v_flex()
                                                        .gap_0p5()
                                                        .child(
                                                            div()
                                                                .text_sm()
                                                                .font_weight(FontWeight::MEDIUM)
                                                                .child(p.name()),
                                                        )
                                                        .child(
                                                            div()
                                                                .text_xs()
                                                                .text_color(theme.muted_foreground)
                                                                .child(format!("Consumos acumulados: {}", amount(consumed))),
                                                        ),
                                                )
                                                .child(
                                                    h_flex()
                                                        .gap_2()
                                                        .child(if bal > 0 {
                                                            Tag::warning().small().child(amount(bal))
                                                        } else if bal < 0 {
                                                            Tag::info().small().child(format!("A favor {}", amount(-bal)))
                                                        } else {
                                                            Tag::success().outline().small().child("S/ 0.00")
                                                        })
                                                        .child(
                                                            Button::new(format!("cash-select-{pid}"))
                                                                .small()
                                                                .when(is_sel, |b| b.primary())
                                                                .when(!is_sel, |b| b.outline())
                                                                .label(if is_sel { "Activo" } else { "Gestionar" })
                                                                .on_click(cx.listener(
                                                                    move |this, _, window, cx| {
                                                                        this.select_cash_pensioner(
                                                                            pid, window, cx,
                                                                        );
                                                                    },
                                                                )),
                                                        ),
                                                )
                                        })),
                                ),
                        ),
                )
                .child(
                    v_flex()
                        .flex_1()
                        .min_w(px(360.))
                        .gap_4()
                        .child(
                            GroupBox::new()
                                .outline()
                                .title("Operación de cobro / abono")
                                .child(if let Some(person) = selected_person {
                                    let pid = person.id;
                                    let consumed = data.consumed_total(pid);
                                    let paid = data.paid_total(pid);
                                    let debt = data.balance(pid);
                                    let credit = data.credit_balance(pid);
                                    let (p_tkt_count, p_tkt_cents) = data.tickets_for_pensioner(pid);
                                    let ticket_total = ticket_unit
                                        .saturating_mul(app_view.cash_ticket_count.max(1) as i64);

                                    v_flex()
                                        .gap_3()
                                        .child(
                                            h_flex()
                                                .justify_between()
                                                .p_3()
                                                .rounded(theme.radius)
                                                .bg(theme.muted)
                                                .child(
                                                    v_flex()
                                                        .gap_0p5()
                                                        .child(
                                                            div()
                                                                .text_sm()
                                                                .font_weight(FontWeight::BOLD)
                                                                .child(person.name()),
                                                        )
                                                        .child(
                                                            div()
                                                                .text_xs()
                                                                .text_color(theme.muted_foreground)
                                                                .child(format!(
                                                                    "Consumido: {} · Abonado: {} (Tickets: {} und. = {})",
                                                                    amount(consumed),
                                                                    amount(paid),
                                                                    p_tkt_count,
                                                                    amount(p_tkt_cents)
                                                                )),
                                                        ),
                                                )
                                                .child(
                                                    v_flex()
                                                        .items_end()
                                                        .child(
                                                            div()
                                                                .text_xs()
                                                                .text_color(theme.muted_foreground)
                                                                .child(if credit > 0 {
                                                                    "Saldo a favor"
                                                                } else {
                                                                    "Deuda pendiente"
                                                                }),
                                                        )
                                                        .child(
                                                            div()
                                                                .text_xl()
                                                                .font_weight(FontWeight::BOLD)
                                                                .text_color(if credit > 0 {
                                                                    theme.primary
                                                                } else if debt > 0 {
                                                                    theme.warning
                                                                } else {
                                                                    theme.foreground
                                                                })
                                                                .child(if credit > 0 {
                                                                    amount(credit)
                                                                } else {
                                                                    amount(debt)
                                                                }),
                                                        ),
                                                ),
                                        )
                                        .child(
                                            h_flex().gap_2().flex_wrap().children(
                                                [
                                                    (CashOpKind::Payment, "Pago"),
                                                    (CashOpKind::Tickets, "Tickets físicos"),
                                                ]
                                                .into_iter()
                                                .map(|(kind, lbl)| {
                                                    Button::new(format!("cash-op-tab-{lbl}"))
                                                        .small()
                                                        .when(app_view.cash_op_kind == kind, |b| b.primary())
                                                        .when(app_view.cash_op_kind != kind, |b| b.outline())
                                                        .label(lbl)
                                                        .on_click(cx.listener(move |this, _, _, cx| {
                                                            this.cash_op_kind = kind;
                                                            cx.notify();
                                                        }))
                                                }),
                                            ),
                                        )
                                        .when(app_view.cash_op_kind == CashOpKind::Payment, |this| {
                                            this.child(
                                                v_flex()
                                                    .gap_2()
                                                    .child(
                                                        h_flex()
                                                            .justify_between()
                                                            .child(
                                                                div()
                                                                    .text_xs()
                                                                    .text_color(theme.muted_foreground)
                                                                    .child("Importe a pagar (S/) *"),
                                                            )
                                                            .child(
                                                                Button::new("cash-full-debt")
                                                                    .ghost()
                                                                    .xsmall()
                                                                    .disabled(debt == 0)
                                                                    .label("Usar pago completo")
                                                                    .on_click(cx.listener(
                                                                        move |this, _, window, cx| {
                                                                            this.cash_amount_input.update(
                                                                                cx,
                                                                                |input, cx| {
                                                                                    input.set_value(
                                                                                        format_decimal(debt),
                                                                                        window,
                                                                                        cx,
                                                                                    );
                                                                                },
                                                                            );
                                                                        },
                                                                    )),
                                                            ),
                                                    )
                                                    .child(Input::new(&app_view.cash_amount_input)),
                                            )
                                        })
                                        .when(app_view.cash_op_kind == CashOpKind::Tickets, |this| {
                                            this.child(
                                                h_flex()
                                                    .justify_between()
                                                    .gap_3()
                                                    .child(
                                                        h_flex()
                                                            .gap_2()
                                                            .child(div().text_sm().child("Tickets:"))
                                                            .child(
                                                                Button::new("ticket-dec")
                                                                    .outline()
                                                                    .small()
                                                                    .disabled(app_view.cash_ticket_count <= 1)
                                                                    .label("-")
                                                                    .on_click(cx.listener(|this, _, _, cx| {
                                                                        this.cash_ticket_count =
                                                                            this.cash_ticket_count.saturating_sub(1).max(1);
                                                                        cx.notify();
                                                                    })),
                                                            )
                                                            .child(
                                                                div()
                                                                    .w(px(28.))
                                                                    .text_center()
                                                                    .text_sm()
                                                                    .font_weight(FontWeight::BOLD)
                                                                    .child(app_view.cash_ticket_count.to_string()),
                                                            )
                                                            .child(
                                                                Button::new("ticket-inc")
                                                                    .outline()
                                                                    .small()
                                                                    .label("+")
                                                                    .on_click(cx.listener(|this, _, _, cx| {
                                                                        this.cash_ticket_count =
                                                                            this.cash_ticket_count.saturating_add(1);
                                                                        cx.notify();
                                                                    })),
                                                            ),
                                                    )
                                                    .child(
                                                        div()
                                                            .text_sm()
                                                            .font_weight(FontWeight::SEMIBOLD)
                                                            .child(format!("Equivalente: {}", amount(ticket_total))),
                                                    ),
                                            )
                                        })
                                        .child(
                                            h_flex().justify_end().child(
                                                Button::new("cash-register-op-btn")
                                                    .primary()
                                                    .small()
                                                    .disabled(
                                                        !can_operate
                                                            || !is_open
                                                            || (app_view.cash_op_kind != CashOpKind::Tickets
                                                                && debt == 0),
                                                    )
                                                    .icon(LucideIcon::Check)
                                                    .label("Registrar operación")
                                                    .on_click(cx.listener(|this, _, window, cx| {
                                                        this.open_cash_operation_dialog(window, cx);
                                                    })),
                                            ),
                                        )
                                        .into_any_element()
                                } else {
                                    div()
                                        .py_4()
                                        .text_sm()
                                        .text_color(theme.muted_foreground)
                                        .child("Selecciona un pensionista para registrar pagos o tickets.")
                                        .into_any_element()
                                }),
                        )
                        .when_some(selected_person, |this, person| {
                            let monthly = data.monthly_summary(person.id);
                            this.child(
                                GroupBox::new()
                                    .outline()
                                    .title(format!("Resumen mensual · {}", person.name()))
                                    .child(if monthly.is_empty() {
                                        div()
                                            .text_sm()
                                            .text_color(theme.muted_foreground)
                                            .child("Sin movimientos mensuales registrados.")
                                            .into_any_element()
                                    } else {
                                        v_flex()
                                            .gap_2()
                                            .children(monthly.into_iter().map(|m| {
                                                h_flex()
                                                    .justify_between()
                                                    .gap_2()
                                                    .py_1p5()
                                                    .border_b_1()
                                                    .border_color(theme.border)
                                                    .child(
                                                        v_flex()
                                                            .gap_0p5()
                                                            .child(
                                                                div()
                                                                    .text_xs()
                                                                    .font_weight(FontWeight::BOLD)
                                                                    .child(format!("Mes {}", m.month_label)),
                                                            )
                                                            .child(
                                                                div()
                                                                    .text_xs()
                                                                    .text_color(theme.muted_foreground)
                                                                    .child(format!(
                                                                        "Consumos: {} · Pagos: {} · Tickets: {}",
                                                                        amount(m.consumed_cents),
                                                                        amount(m.cash_paid_cents),
                                                                        amount(m.tickets_cents)
                                                                    )),
                                                            ),
                                                    )
                                                    .child(
                                                        div()
                                                            .text_xs()
                                                            .font_weight(FontWeight::SEMIBOLD)
                                                            .child(format!("Neto: {}", amount(m.net_balance_cents))),
                                                    )
                                            }))
                                            .into_any_element()
                                    }),
                            )
                        }),
                ),
        )
        .child(
            GroupBox::new()
                .outline()
                .title("Historial de movimientos de caja")
                .child(if data.cash_register.movements.is_empty() {
                    div()
                        .text_sm()
                        .text_color(theme.muted_foreground)
                        .child("Sin movimientos de caja registrados.")
                        .into_any_element()
                } else {
                    v_flex()
                        .gap_2()
                        .children(data.cash_register.movements.iter().rev().take(12).map(|m| {
                            let mov_id_btn = m.id.clone();
                            h_flex()
                                .justify_between()
                                .gap_3()
                                .py_1p5()
                                .border_b_1()
                                .border_color(theme.border)
                                .child(
                                    h_flex()
                                        .gap_2()
                                        .flex_wrap()
                                        .child(Tag::secondary().small().child(m.id.clone()))
                                        .child(movement_tag(m.kind))
                                        .child(div().text_xs().child(format!(
                                            "{} {} · {} · Por: {}",
                                            m.date.format("%d/%m/%Y"),
                                            m.time,
                                            m.description,
                                            m.operator_name
                                        ))),
                                )
                                .child(
                                    h_flex()
                                        .gap_2()
                                        .items_center()
                                        .child(
                                            div()
                                                .text_xs()
                                                .font_weight(FontWeight::SEMIBOLD)
                                                .child(amount(m.cents)),
                                        )
                                        .child(
                                            Button::new(format!("cash-receipt-{}", m.id))
                                                .ghost()
                                                .xsmall()
                                                .icon(LucideIcon::FileText)
                                                .tooltip("Ver comprobante simulado")
                                                .on_click(cx.listener(move |this, _, window, cx| {
                                                    this.open_receipt_preview_dialog(
                                                        &mov_id_btn,
                                                        window,
                                                        cx,
                                                    );
                                                })),
                                        ),
                                )
                        }))
                        .into_any_element()
                }),
        )
        .into_any_element()
}
