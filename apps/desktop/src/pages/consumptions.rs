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
    mock_data::{
        DraftConsumptionItem, Meal, MenuKind, parse_price_cents,
    },
    state::{AppState, DataStatus, Page},
};

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

impl DesktopApp {
    fn has_consumption_permission(&self, permission: &str) -> bool {
        self.state.auth.can_view(Page::Consumos) && allowed(&self.state, permission)
    }

    pub fn select_consumption_pensioner(&mut self, id: Option<u32>, cx: &mut Context<Self>) {
        self.pending_consumption_pensioner = id;
        cx.notify();
    }

    pub fn add_consumption_item(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !self.has_consumption_permission("consumptions.create") {
            window.push_notification(
                Notification::error("No tienes permiso para registrar consumos."),
                cx,
            );
            return;
        }
        let Some(pid) = self.pending_consumption_pensioner else {
            window.push_notification(
                Notification::error("Selecciona primero un pensionista."),
                cx,
            );
            return;
        };
        let active = match &self.state.data {
            DataStatus::Ready(data) => data
                .pensioners
                .iter()
                .find(|p| p.id == pid)
                .map(|p| p.active)
                .unwrap_or(false),
            _ => false,
        };
        if !active {
            window.push_notification(
                Notification::error("El pensionista seleccionado está inactivo."),
                cx,
            );
            return;
        }

        let unit_price_cents = match self.consumption_menu_kind {
            MenuKind::Economic => self.consumption_pricing.economic_for(self.consumption_meal),
            MenuKind::Special => {
                let raw = self.consumption_special_price.read(cx).value().to_string();
                match parse_price_cents(&raw) {
                    Ok(cents) => cents,
                    Err(err) => {
                        window.push_notification(Notification::error(err), cx);
                        return;
                    }
                }
            }
        };

        let description = if self.consumption_menu_kind == MenuKind::Special {
            let desc = self.consumption_special_desc.read(cx).value().trim().to_string();
            if desc.is_empty() {
                "Plato especial".to_string()
            } else {
                desc
            }
        } else {
            "Ración económica".to_string()
        };

        let portions = self.consumption_portions.max(1);
        if let Some(existing) = self.consumption_draft.iter_mut().find(|item| {
            item.meal == self.consumption_meal
                && item.menu_kind == self.consumption_menu_kind
                && item.unit_price_cents == unit_price_cents
                && item.description == description
        }) {
            existing.portions = existing.portions.saturating_add(portions);
        } else {
            self.consumption_next_draft_id += 1;
            self.consumption_draft.push(DraftConsumptionItem {
                id: self.consumption_next_draft_id,
                meal: self.consumption_meal,
                menu_kind: self.consumption_menu_kind,
                description,
                portions,
                unit_price_cents,
            });
        }

        self.consumption_portions = 1;
        if self.consumption_menu_kind == MenuKind::Special {
            self.consumption_special_desc.update(cx, |input, cx| {
                input.set_value("", window, cx);
            });
        }
        cx.notify();
    }

    pub fn update_draft_portions(&mut self, item_id: u64, delta: i32, cx: &mut Context<Self>) {
        if let Some(pos) = self.consumption_draft.iter().position(|i| i.id == item_id) {
            let current = self.consumption_draft[pos].portions as i32;
            let next = current + delta;
            if next <= 0 {
                self.consumption_draft.remove(pos);
            } else {
                self.consumption_draft[pos].portions = next as u32;
            }
            cx.notify();
        }
    }

    pub fn remove_draft_item(&mut self, item_id: u64, cx: &mut Context<Self>) {
        self.consumption_draft.retain(|i| i.id != item_id);
        cx.notify();
    }

    pub fn clear_consumption_draft(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.consumption_draft.clear();
        self.consumption_third_party = false;
        self.consumption_third_party_name.update(cx, |input, cx| {
            input.set_value("", window, cx);
        });
        cx.notify();
    }

    pub fn open_pricing_dialog(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let b_input = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder("7.00")
                .default_value(format_decimal(self.consumption_pricing.economic_breakfast_cents))
        });
        let l_input = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder("7.00")
                .default_value(format_decimal(self.consumption_pricing.economic_lunch_cents))
        });
        let s_input = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder("12.00")
                .default_value(format_decimal(self.consumption_pricing.special_default_cents))
        });
        let err_state: Entity<Option<String>> = cx.new(|_| None);
        let app = cx.entity().downgrade();

        window.open_dialog(cx, move |dialog, _, cx| {
            let b_view = b_input.clone();
            let l_view = l_input.clone();
            let s_view = s_input.clone();
            let b_submit = b_input.clone();
            let l_submit = l_input.clone();
            let s_submit = s_input.clone();
            let err_view = err_state.clone();
            let err_submit = err_state.clone();
            let app = app.clone();
            let current_err = err_view.read(cx).clone();

            dialog
                .title("Configurar precios predeterminados")
                .child(
                    v_flex()
                        .gap_3()
                        .when_some(current_err, |this, err| {
                            this.child(Alert::error("pricing-form-error", err))
                        })
                        .child(
                            v_flex()
                                .gap_1()
                                .child(div().text_sm().font_weight(FontWeight::MEDIUM).child("Desayuno económico (S/)"))
                                .child(Input::new(&b_view)),
                        )
                        .child(
                            v_flex()
                                .gap_1()
                                .child(div().text_sm().font_weight(FontWeight::MEDIUM).child("Almuerzo económico (S/)"))
                                .child(Input::new(&l_view)),
                        )
                        .child(
                            v_flex()
                                .gap_1()
                                .child(div().text_sm().font_weight(FontWeight::MEDIUM).child("Especial sugerido (S/)"))
                                .child(Input::new(&s_view)),
                        ),
                )
                .footer(
                    h_flex()
                        .gap_3()
                        .justify_end()
                        .child(
                            Button::new("pricing-cancel")
                                .outline()
                                .label("Cancelar")
                                .on_click(|_, window, cx| window.close_dialog(cx)),
                        )
                        .child(
                            Button::new("pricing-save")
                                .primary()
                                .label("Guardar precios")
                                .on_click(move |_, window, cx| {
                                    let b_raw = b_submit.read(cx).value().to_string();
                                    let l_raw = l_submit.read(cx).value().to_string();
                                    let s_raw = s_submit.read(cx).value().to_string();

                                    let parsed = (|| -> Result<(i64, i64, i64), String> {
                                        Ok((
                                            parse_price_cents(&b_raw)?,
                                            parse_price_cents(&l_raw)?,
                                            parse_price_cents(&s_raw)?,
                                        ))
                                    })();

                                    match parsed {
                                        Ok((b_cents, l_cents, s_cents)) => {
                                            let _ = app.update(cx, |this, cx| {
                                                this.consumption_pricing.economic_breakfast_cents =
                                                    b_cents;
                                                this.consumption_pricing.economic_lunch_cents =
                                                    l_cents;
                                                this.consumption_pricing.special_default_cents =
                                                    s_cents;
                                                this.consumption_special_price.update(
                                                    cx,
                                                    |input, cx| {
                                                        input.set_value(
                                                            format_decimal(s_cents),
                                                            window,
                                                            cx,
                                                        );
                                                    },
                                                );
                                                cx.notify();
                                            });
                                            err_submit.update(cx, |e, _| *e = None);
                                            window.close_dialog(cx);
                                            window.push_notification(
                                                Notification::success(
                                                    "Precios predeterminados actualizados.",
                                                ),
                                                cx,
                                            );
                                        }
                                        Err(err) => {
                                            err_submit.update(cx, |e, cx| {
                                                *e = Some(err.clone());
                                                cx.notify();
                                            });
                                        }
                                    }
                                }),
                        ),
                )
        });
    }

    pub fn open_confirm_consumption_dialog(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.consumption_submitting {
            return;
        }
        if !self.has_consumption_permission("consumptions.create") {
            window.push_notification(
                Notification::error("No tienes permiso para confirmar consumos."),
                cx,
            );
            return;
        }
        let Some(pid) = self.pending_consumption_pensioner else {
            window.push_notification(
                Notification::error("Selecciona un pensionista antes de confirmar."),
                cx,
            );
            return;
        };
        if self.consumption_draft.is_empty() {
            window.push_notification(
                Notification::error("Agrega al menos una ración a la atención."),
                cx,
            );
            return;
        }
        let pickup_name = if self.consumption_third_party {
            let name = self
                .consumption_third_party_name
                .read(cx)
                .value()
                .trim()
                .to_string();
            if name.chars().count() < 2 {
                window.push_notification(
                    Notification::error("Ingresa el nombre de quien recoge el consumo."),
                    cx,
                );
                return;
            }
            Some(name)
        } else {
            None
        };

        let person_name = match &self.state.data {
            DataStatus::Ready(data) => data
                .pensioners
                .iter()
                .find(|p| p.id == pid && p.active)
                .map(|p| p.name()),
            _ => None,
        };
        let Some(person_name) = person_name else {
            window.push_notification(
                Notification::error("El pensionista debe estar activo para registrar consumo."),
                cx,
            );
            return;
        };

        let total_portions: u32 = self.consumption_draft.iter().map(|i| i.portions).sum();
        let total_cents: i64 = self.consumption_draft.iter().map(|i| i.subtotal_cents()).sum();
        let token = self.consumption_confirm_token;
        let app = cx.entity().downgrade();

        window.open_dialog(cx, move |dialog, _, cx| {
            let app = app.clone();
            let pickup_display = pickup_name.clone();
            let pickup_submit = pickup_name.clone();
            let person_display = person_name.clone();

            dialog
                .title("Confirmar registro de consumo")
                .child(
                    v_flex()
                        .gap_3()
                        .child(
                            div()
                                .text_sm()
                                .child(format!("Pensionista: {person_display} (ID #{pid})")),
                        )
                        .when_some(pickup_display, |this, name| {
                            this.child(
                                Alert::info(
                                    "third-party-confirm-info",
                                    format!("Recogido por tercero: {name}"),
                                ),
                            )
                        })
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
                                        .text_sm()
                                        .child(format!("Total de raciones: {total_portions}")),
                                )
                                .child(
                                    div()
                                        .text_base()
                                        .font_weight(FontWeight::BOLD)
                                        .child(amount(total_cents)),
                                ),
                        ),
                )
                .footer(
                    h_flex()
                        .gap_3()
                        .justify_end()
                        .child(
                            Button::new("consumption-confirm-cancel")
                                .outline()
                                .label("Cancelar")
                                .on_click(|_, window, cx| window.close_dialog(cx)),
                        )
                        .child(
                            Button::new("consumption-confirm-submit")
                                .primary()
                                .label("Confirmar y guardar")
                                .on_click(move |_, window, cx| {
                                    let pickup = pickup_submit.clone();
                                    let res = app.update(cx, |this, cx| {
                                        if this.consumption_submitting {
                                            return Err(
                                                "Registro en curso; espera un momento."
                                                    .to_string(),
                                            );
                                        }
                                        this.consumption_submitting = true;
                                        let date = this.state.dashboard_date;
                                        let items = this.consumption_draft.clone();
                                        let outcome = match &mut this.state.data {
                                            DataStatus::Ready(data) => data.record_consumptions(
                                                date,
                                                "13:15",
                                                pid,
                                                &items,
                                                pickup.as_deref(),
                                                token,
                                                &mut this.consumption_processed_tokens,
                                            ),
                                            _ => Err("Los datos no están disponibles.".into()),
                                        };
                                        this.consumption_submitting = false;
                                        if outcome.is_ok() {
                                            this.consumption_draft.clear();
                                            this.consumption_third_party = false;
                                            this.consumption_third_party_name.update(
                                                cx,
                                                |input, cx| input.set_value("", window, cx),
                                            );
                                            this.consumption_confirm_token =
                                                this.consumption_confirm_token.wrapping_add(1);
                                            cx.notify();
                                        }
                                        outcome
                                    });

                                    match res {
                                        Ok(Ok(count)) => {
                                            window.close_dialog(cx);
                                            window.push_notification(
                                                Notification::success(format!(
                                                    "Consumo confirmado ({count} línea(s) registradas)."
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
                                            window.push_notification(
                                                Notification::error(
                                                    "La pantalla ya no está disponible.",
                                                ),
                                                cx,
                                            );
                                        }
                                    }
                                }),
                        ),
                )
        });
    }
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
        DataStatus::Loading => return div().child("Cargando módulo de consumos…").into_any_element(),
        DataStatus::Error(err) => {
            return Alert::error("consumptions-load-err", err.clone()).into_any_element();
        }
    };

    let can_create = allowed(state, "consumptions.create");
    let search_query = app_view.consumption_search.read(app).value().trim().to_lowercase();

    let selected_person = app_view
        .pending_consumption_pensioner
        .and_then(|id| data.pensioners.iter().find(|p| p.id == id));

    let matching_pensioners: Vec<_> = data
        .pensioners
        .iter()
        .filter(|p| {
            if search_query.is_empty() {
                p.active
            } else {
                format!(
                    "{} {} {} {}",
                    p.id,
                    p.name(),
                    p.phone.as_deref().unwrap_or(""),
                    p.biometric_id.as_deref().unwrap_or("")
                )
                .to_lowercase()
                .contains(&search_query)
            }
        })
        .take(6)
        .collect();

    let draft_portions: u32 = app_view.consumption_draft.iter().map(|i| i.portions).sum();
    let draft_total_cents: i64 = app_view
        .consumption_draft
        .iter()
        .map(|i| i.subtotal_cents())
        .sum();

    let current_unit_cents = match app_view.consumption_menu_kind {
        MenuKind::Economic => app_view.consumption_pricing.economic_for(app_view.consumption_meal),
        MenuKind::Special => parse_price_cents(&app_view.consumption_special_price.read(app).value())
            .unwrap_or(0),
    };
    let preview_subtotal = current_unit_cents.saturating_mul(app_view.consumption_portions as i64);

    let today_consumptions: Vec<_> = data
        .consumptions
        .iter()
        .filter(|c| c.date == state.dashboard_date)
        .rev()
        .take(8)
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
                                .child("Atención y consumos"),
                        )
                        .child(
                            div()
                                .text_sm()
                                .text_color(theme.muted_foreground)
                                .child("Registro rápido de desayunos y almuerzos · datos compartidos en memoria"),
                        ),
                )
                .child(
                    h_flex()
                        .gap_2()
                        .flex_wrap()
                        .child(
                            Button::new("consumptions-pricing-btn")
                                .outline()
                                .small()
                                .icon(LucideIcon::Settings)
                                .label("Precios predeterminados")
                                .on_click(cx.listener(|this, _, window, cx| {
                                    this.open_pricing_dialog(window, cx);
                                })),
                        )
                        .child(
                            Button::new("consumptions-go-biometric")
                                .outline()
                                .small()
                                .icon(LucideIcon::FingerprintPattern)
                                .label("Identificar por huella")
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.navigate_to(Page::Identificacion, cx);
                                })),
                        ),
                ),
        )
        .child(
            h_flex()
                .gap_5()
                .items_start()
                .flex_wrap()
                .child(
                    v_flex()
                        .flex_1()
                        .min_w(px(350.))
                        .gap_4()
                        .child(
                            GroupBox::new()
                                .outline()
                                .title("1. Pensionista atendido")
                                .child(
                                    v_flex()
                                        .gap_3()
                                        .child(
                                            Input::new(&app_view.consumption_search)
                                                .cleanable(true)
                                                .prefix(Icon::new(LucideIcon::Search).xsmall()),
                                        )
                                        .when_some(selected_person, |this, person| {
                                            let balance = data.balance(person.id);
                                            this.child(
                                                h_flex()
                                                    .justify_between()
                                                    .gap_3()
                                                    .p_3()
                                                    .rounded(theme.radius)
                                                    .border_1()
                                                    .border_color(theme.primary)
                                                    .bg(theme.muted)
                                                    .child(
                                                        v_flex()
                                                            .gap_1()
                                                            .child(
                                                                h_flex()
                                                                    .gap_2()
                                                                    .child(
                                                                        div()
                                                                            .text_sm()
                                                                            .font_weight(FontWeight::BOLD)
                                                                            .child(person.name()),
                                                                    )
                                                                    .child(Tag::secondary().small().child(format!("#{}", person.id)))
                                                                    .child(if person.active {
                                                                        Tag::success().outline().small().child("Activo")
                                                                    } else {
                                                                        Tag::danger().small().child("Inactivo")
                                                                    }),
                                                            )
                                                            .child(
                                                                div()
                                                                    .text_xs()
                                                                    .text_color(theme.muted_foreground)
                                                                    .child(format!(
                                                                        "Saldo pendiente actual: {} · Huella: {}",
                                                                        amount(balance),
                                                                        person.biometric_status()
                                                                    )),
                                                            ),
                                                    )
                                                    .child(
                                                        Button::new("clear-selected-pensioner")
                                                            .ghost()
                                                            .small()
                                                            .label("Cambiar")
                                                            .on_click(cx.listener(|this, _, _, cx| {
                                                                this.select_consumption_pensioner(None, cx);
                                                            })),
                                                    ),
                                            )
                                        })
                                        .child(
                                            v_flex()
                                                .gap_1p5()
                                                .children(matching_pensioners.into_iter().map(|p| {
                                                    let pid = p.id;
                                                    let selected = app_view.pending_consumption_pensioner == Some(pid);
                                                    let bal = data.balance(pid);
                                                    h_flex()
                                                        .justify_between()
                                                        .gap_2()
                                                        .px_3()
                                                        .py_2()
                                                        .rounded(theme.radius)
                                                        .border_1()
                                                        .border_color(if selected {
                                                            theme.primary
                                                        } else {
                                                            theme.border
                                                        })
                                                        .child(
                                                            h_flex()
                                                                .gap_2()
                                                                .child(
                                                                    div()
                                                                        .text_sm()
                                                                        .font_weight(FontWeight::MEDIUM)
                                                                        .child(format!("#{pid} · {}", p.name())),
                                                                )
                                                                .when(!p.active, |this| {
                                                                    this.child(Tag::secondary().small().child("Inactivo"))
                                                                }),
                                                        )
                                                        .child(
                                                            h_flex()
                                                                .gap_2()
                                                                .child(
                                                                    div()
                                                                        .text_xs()
                                                                        .text_color(theme.muted_foreground)
                                                                        .child(amount(bal)),
                                                                )
                                                                .child(
                                                                    Button::new(format!("pick-pensioner-{pid}"))
                                                                        .small()
                                                                        .when(selected, |b| b.primary())
                                                                        .when(!selected, |b| b.outline())
                                                                        .label(if selected { "Elegido" } else { "Elegir" })
                                                                        .on_click(cx.listener(
                                                                            move |this, _, _, cx| {
                                                                                this.select_consumption_pensioner(
                                                                                    Some(pid),
                                                                                    cx,
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
                            GroupBox::new()
                                .outline()
                                .title("2. Configurar ración")
                                .child(
                                    v_flex()
                                        .gap_3()
                                        .child(
                                            h_flex()
                                                .gap_2()
                                                .child(
                                                    Button::new("meal-breakfast")
                                                        .small()
                                                        .when(app_view.consumption_meal == Meal::Breakfast, |b| b.primary())
                                                        .when(app_view.consumption_meal != Meal::Breakfast, |b| b.outline())
                                                        .label("Desayuno")
                                                        .on_click(cx.listener(|this, _, _, cx| {
                                                            this.consumption_meal = Meal::Breakfast;
                                                            cx.notify();
                                                        })),
                                                )
                                                .child(
                                                    Button::new("meal-lunch")
                                                        .small()
                                                        .when(app_view.consumption_meal == Meal::Lunch, |b| b.primary())
                                                        .when(app_view.consumption_meal != Meal::Lunch, |b| b.outline())
                                                        .label("Almuerzo")
                                                        .on_click(cx.listener(|this, _, _, cx| {
                                                            this.consumption_meal = Meal::Lunch;
                                                            cx.notify();
                                                        })),
                                                ),
                                        )
                                        .child(
                                            h_flex()
                                                .gap_2()
                                                .flex_wrap()
                                                .child(
                                                    Button::new("menu-economic")
                                                        .small()
                                                        .when(app_view.consumption_menu_kind == MenuKind::Economic, |b| b.primary())
                                                        .when(app_view.consumption_menu_kind != MenuKind::Economic, |b| b.outline())
                                                        .label(format!(
                                                            "Económico ({})",
                                                            amount(app_view.consumption_pricing.economic_for(app_view.consumption_meal))
                                                        ))
                                                        .on_click(cx.listener(|this, _, _, cx| {
                                                            this.consumption_menu_kind = MenuKind::Economic;
                                                            cx.notify();
                                                        })),
                                                )
                                                .child(
                                                    Button::new("menu-special")
                                                        .small()
                                                        .when(app_view.consumption_menu_kind == MenuKind::Special, |b| b.primary())
                                                        .when(app_view.consumption_menu_kind != MenuKind::Special, |b| b.outline())
                                                        .label("Especial (precio variable)")
                                                        .on_click(cx.listener(|this, _, _, cx| {
                                                            this.consumption_menu_kind = MenuKind::Special;
                                                            cx.notify();
                                                        })),
                                                ),
                                        )
                                        .when(app_view.consumption_menu_kind == MenuKind::Special, |this| {
                                            this.child(
                                                h_flex()
                                                    .gap_3()
                                                    .flex_wrap()
                                                    .child(
                                                        v_flex()
                                                            .w(px(140.))
                                                            .gap_1()
                                                            .child(div().text_xs().text_color(theme.muted_foreground).child("Precio unitario (S/) *"))
                                                            .child(Input::new(&app_view.consumption_special_price)),
                                                    )
                                                    .child(
                                                        v_flex()
                                                            .flex_1()
                                                            .min_w(px(170.))
                                                            .gap_1()
                                                            .child(div().text_xs().text_color(theme.muted_foreground).child("Plato especial (opcional)"))
                                                            .child(Input::new(&app_view.consumption_special_desc)),
                                                    ),
                                            )
                                        })
                                        .child(
                                            h_flex()
                                                .justify_between()
                                                .gap_3()
                                                .flex_wrap()
                                                .child(
                                                    h_flex()
                                                        .gap_2()
                                                        .child(div().text_sm().child("Raciones:"))
                                                        .child(
                                                            Button::new("portions-dec")
                                                                .outline()
                                                                .small()
                                                                .disabled(app_view.consumption_portions <= 1)
                                                                .label("-")
                                                                .on_click(cx.listener(|this, _, _, cx| {
                                                                    this.consumption_portions =
                                                                        this.consumption_portions.saturating_sub(1).max(1);
                                                                    cx.notify();
                                                                })),
                                                        )
                                                        .child(
                                                            div()
                                                                .w(px(32.))
                                                                .text_center()
                                                                .text_sm()
                                                                .font_weight(FontWeight::BOLD)
                                                                .child(app_view.consumption_portions.to_string()),
                                                        )
                                                        .child(
                                                            Button::new("portions-inc")
                                                                .outline()
                                                                .small()
                                                                .label("+")
                                                                .on_click(cx.listener(|this, _, _, cx| {
                                                                    this.consumption_portions =
                                                                        this.consumption_portions.saturating_add(1);
                                                                    cx.notify();
                                                                })),
                                                        ),
                                                )
                                                .child(
                                                    h_flex()
                                                        .gap_3()
                                                        .child(
                                                            div()
                                                                .text_sm()
                                                                .font_weight(FontWeight::SEMIBOLD)
                                                                .child(format!("Subtotal: {}", amount(preview_subtotal))),
                                                        )
                                                        .child(
                                                            Button::new("add-consumption-item")
                                                                .primary()
                                                                .small()
                                                                .disabled(!can_create || selected_person.is_none())
                                                                .icon(LucideIcon::Plus)
                                                                .label("Agregar")
                                                                .on_click(cx.listener(|this, _, window, cx| {
                                                                    this.add_consumption_item(window, cx);
                                                                })),
                                                        ),
                                                ),
                                        ),
                                ),
                        ),
                )
                .child(
                    v_flex()
                        .flex_1()
                        .min_w(px(350.))
                        .gap_4()
                        .child(
                            GroupBox::new()
                                .outline()
                                .title("3. Detalle de la atención")
                                .child(
                                    v_flex()
                                        .gap_3()
                                        .child(if app_view.consumption_draft.is_empty() {
                                            Empty::new()
                                                .py_4()
                                                .header(
                                                    EmptyHeader::new()
                                                        .media(
                                                            EmptyMedia::new()
                                                                .with_variant(EmptyMediaVariant::Icon)
                                                                .child(Icon::new(LucideIcon::Utensils).size_5()),
                                                        )
                                                        .title(EmptyTitle::new().child("Sin raciones agregadas"))
                                                        .description(
                                                            EmptyDescription::new()
                                                                .child("Agrega desayunos o almuerzos antes de confirmar."),
                                                        ),
                                                )
                                                .into_any_element()
                                        } else {
                                            v_flex()
                                                .gap_2()
                                                .children(app_view.consumption_draft.iter().map(|item| {
                                                    let item_id = item.id;
                                                    h_flex()
                                                        .justify_between()
                                                        .gap_2()
                                                        .p_2p5()
                                                        .rounded(theme.radius)
                                                        .border_1()
                                                        .border_color(theme.border)
                                                        .child(
                                                            v_flex()
                                                                .gap_0p5()
                                                                .child(
                                                                    h_flex()
                                                                        .gap_1p5()
                                                                        .child(
                                                                            div()
                                                                                .text_sm()
                                                                                .font_weight(FontWeight::MEDIUM)
                                                                                .child(format!(
                                                                                    "{} · {}",
                                                                                    item.meal.label(),
                                                                                    item.description
                                                                                )),
                                                                        )
                                                                        .child(
                                                                            Tag::secondary()
                                                                                .small()
                                                                                .child(item.menu_kind.label()),
                                                                        ),
                                                                )
                                                                .child(
                                                                    div()
                                                                        .text_xs()
                                                                        .text_color(theme.muted_foreground)
                                                                        .child(format!(
                                                                            "{} × {}",
                                                                            item.portions,
                                                                            amount(item.unit_price_cents)
                                                                        )),
                                                                ),
                                                        )
                                                        .child(
                                                            h_flex()
                                                                .gap_1p5()
                                                                .child(
                                                                    Button::new(format!("draft-dec-{item_id}"))
                                                                        .ghost()
                                                                        .xsmall()
                                                                        .label("-")
                                                                        .on_click(cx.listener(
                                                                            move |this, _, _, cx| {
                                                                                this.update_draft_portions(
                                                                                    item_id,
                                                                                    -1,
                                                                                    cx,
                                                                                );
                                                                            },
                                                                        )),
                                                                )
                                                                .child(
                                                                    Button::new(format!("draft-inc-{item_id}"))
                                                                        .ghost()
                                                                        .xsmall()
                                                                        .label("+")
                                                                        .on_click(cx.listener(
                                                                            move |this, _, _, cx| {
                                                                                this.update_draft_portions(
                                                                                    item_id,
                                                                                    1,
                                                                                    cx,
                                                                                );
                                                                            },
                                                                        )),
                                                                )
                                                                .child(
                                                                    div()
                                                                        .text_sm()
                                                                        .font_weight(FontWeight::SEMIBOLD)
                                                                        .child(amount(item.subtotal_cents())),
                                                                )
                                                                .child(
                                                                    Button::new(format!("draft-del-{item_id}"))
                                                                        .ghost()
                                                                        .xsmall()
                                                                        .icon(LucideIcon::X)
                                                                        .tooltip("Quitar")
                                                                        .on_click(cx.listener(
                                                                            move |this, _, _, cx| {
                                                                                this.remove_draft_item(
                                                                                    item_id,
                                                                                    cx,
                                                                                );
                                                                            },
                                                                        )),
                                                                ),
                                                        )
                                                }))
                                                .into_any_element()
                                        })
                                        .child(
                                            h_flex()
                                                .justify_between()
                                                .pt_2()
                                                .border_t_1()
                                                .border_color(theme.border)
                                                .child(
                                                    div()
                                                        .text_sm()
                                                        .font_weight(FontWeight::MEDIUM)
                                                        .child("Recogido por tercero"),
                                                )
                                                .child(
                                                    Switch::new("consumption-third-party")
                                                        .checked(app_view.consumption_third_party)
                                                        .on_change(cx.listener(
                                                            |this, checked: &bool, _, cx| {
                                                                this.consumption_third_party =
                                                                    *checked;
                                                                cx.notify();
                                                            },
                                                        )),
                                                ),
                                        )
                                        .when(app_view.consumption_third_party, |this| {
                                            this.child(
                                                v_flex()
                                                    .gap_1()
                                                    .child(
                                                        div()
                                                            .text_xs()
                                                            .text_color(theme.muted_foreground)
                                                            .child("Nombre de quien recoge *"),
                                                    )
                                                    .child(Input::new(&app_view.consumption_third_party_name)),
                                            )
                                        })
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
                                                                .text_xs()
                                                                .text_color(theme.muted_foreground)
                                                                .child(format!("Raciones totales: {draft_portions}")),
                                                        )
                                                        .when_some(selected_person, |this, p| {
                                                            let projected =
                                                                data.balance(p.id) + draft_total_cents;
                                                            this.child(
                                                                div()
                                                                    .text_xs()
                                                                    .text_color(theme.muted_foreground)
                                                                    .child(format!(
                                                                        "Saldo proyectado: {}",
                                                                        amount(projected)
                                                                    )),
                                                            )
                                                        }),
                                                )
                                                .child(
                                                    div()
                                                        .text_xl()
                                                        .font_weight(FontWeight::BOLD)
                                                        .child(amount(draft_total_cents)),
                                                ),
                                        )
                                        .child(
                                            h_flex()
                                                .justify_end()
                                                .gap_2()
                                                .when(!app_view.consumption_draft.is_empty(), |this| {
                                                    this.child(
                                                        Button::new("clear-draft-btn")
                                                            .outline()
                                                            .small()
                                                            .label("Limpiar")
                                                            .on_click(cx.listener(|this, _, window, cx| {
                                                                this.clear_consumption_draft(window, cx);
                                                            })),
                                                    )
                                                })
                                                .child(
                                                    Button::new("confirm-consumption-btn")
                                                        .primary()
                                                        .small()
                                                        .disabled(
                                                            !can_create
                                                                || app_view.consumption_submitting
                                                                || app_view.consumption_draft.is_empty()
                                                                || selected_person.is_none(),
                                                        )
                                                        .icon(LucideIcon::Check)
                                                        .label("Confirmar atención")
                                                        .on_click(cx.listener(|this, _, window, cx| {
                                                            this.open_confirm_consumption_dialog(
                                                                window, cx,
                                                            );
                                                        })),
                                                ),
                                        ),
                                ),
                        )
                        .child(
                            GroupBox::new()
                                .outline()
                                .title("Últimos consumos del día")
                                .child(if today_consumptions.is_empty() {
                                    div()
                                        .text_sm()
                                        .text_color(theme.muted_foreground)
                                        .child("No hay consumos registrados en esta fecha.")
                                        .into_any_element()
                                } else {
                                    v_flex()
                                        .gap_2()
                                        .children(today_consumptions.into_iter().map(|c| {
                                            let person_name: SharedString = data
                                                .pensioners
                                                .iter()
                                                .find(|p| p.id == c.pensioner_id)
                                                .map(|p| p.name().into())
                                                .unwrap_or_else(|| {
                                                    format!("Pensionista #{}", c.pensioner_id).into()
                                                });
                                            let extra = match (&c.detail, &c.picked_up_by) {
                                                (Some(d), Some(p)) => format!(" · {d} · Recoge: {p}"),
                                                (Some(d), None) => format!(" · {d}"),
                                                (None, Some(p)) => format!(" · Recoge: {p}"),
                                                (None, None) => String::new(),
                                            };
                                            h_flex()
                                                .justify_between()
                                                .gap_2()
                                                .py_1p5()
                                                .border_b_1()
                                                .border_color(theme.border)
                                                .child(
                                                    div().text_xs().child(format!(
                                                        "{} · {} · {} ({}) × {}{}",
                                                        c.time,
                                                        person_name,
                                                        c.meal.label(),
                                                        c.menu_kind.label(),
                                                        c.portions,
                                                        extra
                                                    )),
                                                )
                                                .child(
                                                    div()
                                                        .text_xs()
                                                        .font_weight(FontWeight::SEMIBOLD)
                                                        .child(amount(c.cents)),
                                                )
                                        }))
                                        .into_any_element()
                                }),
                        ),
                ),
        )
        .into_any_element()
}
