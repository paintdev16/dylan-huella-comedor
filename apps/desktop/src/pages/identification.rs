use std::{
    sync::{Arc, Mutex, mpsc},
    time::Duration,
};

use gpui_kit::{
    App, AppContext as _, Context, FontWeight, IntoElement, ParentElement, SharedString,
    Styled, Window,
    assets::IconName as LucideIcon,
    component::{
        ActiveTheme as _, Disableable as _, Icon, Sizable as _, WindowExt as _,
        alert::Alert,
        button::{Button, ButtonVariants as _},
        group_box::{GroupBox, GroupBoxVariants as _},
        h_flex,
        input::{Input, InputState},
        notification::Notification,
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
    biometric_service::{
        BiometricError, BiometricEvent, BiometricGateway as _, BiometricReaderState,
        ContinuousBiometricSignal, IdentificationOutcome, SimulatedBiometricGateway,
    },
    layout::DesktopApp,
    mock_data::{DraftConsumptionItem, Meal, MenuKind, parse_price_cents},
    state::{AppState, DataStatus, Page},
};

#[derive(Clone, Copy)]
pub enum SimulationMode {
    MatchActive,
    NoMatch,
    DeviceError,
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

fn state_tag(reader_state: &BiometricReaderState) -> Tag {
    match reader_state {
        BiometricReaderState::Disconnected => Tag::secondary().small().child("Desconectado"),
        BiometricReaderState::Connected => Tag::info().outline().small().child("Conectado"),
        BiometricReaderState::WaitingFingerprint => {
            Tag::warning().outline().small().child("Esperando huella")
        }
        BiometricReaderState::Identifying => Tag::info().small().child("Identificando"),
        BiometricReaderState::Identified { .. } => Tag::success().small().child("Identificado"),
        BiometricReaderState::Unrecognized => Tag::warning().small().child("No reconocido"),
        BiometricReaderState::Error(_) => Tag::danger().small().child("Error"),
    }
}

struct AutoModalState {
    meal: Meal,
    menu_kind: MenuKind,
    portions: u32,
    third_party: bool,
    draft: Vec<DraftConsumptionItem>,
    next_id: u64,
    error: Option<String>,
    submitting: bool,
    token: u64,
}

impl DesktopApp {
    pub fn push_biometric_event(&mut self, event: BiometricEvent) {
        let label: SharedString = match &event {
            BiometricEvent::ReaderStateChanged(s) => format!("Estado -> {}", s.label()).into(),
            BiometricEvent::FingerPlaced => "Dedo colocado en el lector".into(),
            BiometricEvent::FingerRemoved => "Dedo retirado del lector".into(),
            BiometricEvent::Enrolled {
                pensioner_id,
                biometric_id,
            } => format!("Enrolado #{pensioner_id} ({biometric_id})").into(),
            BiometricEvent::OperationCancelled => "Operación cancelada -> Esperando huella".into(),
        };
        self.biometric_event_log.insert(0, label);
        self.biometric_event_log.truncate(8);
    }

    /// Activa el servicio biométrico continuo al iniciar sesión si el usuario tiene permiso.
    pub fn activate_continuous_biometrics(&mut self, cx: &mut Context<Self>) {
        let can_use = self.state.auth.user().is_some_and(|u| {
            u.can("biometrics.identify") || u.can("biometrics.view")
        });
        if !can_use {
            return;
        }
        if !self.biometric_gateway.state().is_connected() {
            if let Ok(evt) = self.biometric_gateway.connect() {
                self.push_biometric_event(evt);
            }
        }
        if let Ok(wait_evt) = self.biometric_gateway.start_waiting() {
            self.push_biometric_event(wait_evt);
        }
        self.start_biometric_signal_loop(cx);
        cx.notify();
    }

    fn start_biometric_signal_loop(&mut self, cx: &mut Context<Self>) {
        let (tx, rx) = mpsc::channel::<ContinuousBiometricSignal>();
        self.biometric_signal_tx = Some(tx);
        let rx = Arc::new(Mutex::new(rx));

        self.biometric_listener_task = Some(cx.spawn(async move |this, cx| {
            loop {
                let rx_clone = rx.clone();
                let signal = cx
                    .background_spawn(async move {
                        let guard = rx_clone.lock().ok()?;
                        guard.recv().ok()
                    })
                    .await;
                let Some(sig) = signal else {
                    break;
                };
                let ok = this.update(cx, |app, cx| {
                    app.handle_continuous_signal(sig, cx);
                });
                if ok.is_err() {
                    break;
                }
            }
        }));
    }

    pub fn emit_biometric_signal(&mut self, signal: ContinuousBiometricSignal, cx: &mut Context<Self>) {
        self.handle_continuous_signal(signal, cx);
    }

    fn handle_continuous_signal(
        &mut self,
        signal: ContinuousBiometricSignal,
        cx: &mut Context<Self>,
    ) {
        if !self.state.auth.can_view(Page::Identificacion)
            && !allowed(&self.state, "biometrics.identify")
        {
            return;
        }
        if !self.biometric_gateway.state().is_connected() {
            self.pending_biometric_notice = Some((
                true,
                "Conecta el lector simulado para recibir lecturas.".into(),
            ));
            cx.notify();
            return;
        }

        match self.biometric_gateway.begin_identifying() {
            Ok(evt) => {
                self.push_biometric_event(BiometricEvent::FingerPlaced);
                self.push_biometric_event(evt);
            }
            Err(BiometricError::ModalLocked | BiometricError::FingerStillPresent) => {
                // Evita identificaciones repetidas mientras el modal esté abierto o el dedo siga ahí.
                return;
            }
            Err(err) => {
                self.pending_biometric_notice = Some((true, err.to_string().into()));
                cx.notify();
                return;
            }
        }

        let candidates = match &self.state.data {
            DataStatus::Ready(data) => data.biometric_candidates(),
            _ => Vec::new(),
        };
        self.biometric_op_id = self.biometric_op_id.wrapping_add(1);
        let op_id = self.biometric_op_id;
        cx.notify();

        let bg_task = cx.background_spawn(async move {
            std::thread::sleep(Duration::from_millis(240));
            match signal {
                ContinuousBiometricSignal::FingerMatch { pensioner_id } => {
                    Ok(SimulatedBiometricGateway::simulate_match(&candidates, pensioner_id))
                }
                ContinuousBiometricSignal::FingerUnrecognized => Ok(IdentificationOutcome::NoMatch),
                ContinuousBiometricSignal::DeviceError(msg) => Err(BiometricError::DeviceError(msg)),
            }
        });

        self.biometric_task = Some(cx.spawn(async move |this, cx| {
            let outcome = bg_task.await;
            let _ = this.update(cx, |app, cx| {
                if app.biometric_op_id != op_id {
                    return;
                }
                let evt = app.biometric_gateway.finish_identification(outcome.clone());
                app.push_biometric_event(evt);
                app.biometric_task = None;

                match outcome {
                    Ok(IdentificationOutcome::Match { pensioner_id, .. }) => {
                        if allowed(&app.state, "consumptions.create") && !app.biometric_modal_open {
                            app.biometric_modal_open = true;
                            app.biometric_gateway.set_modal_locked(true);
                            app.pending_auto_modal_pensioner = Some(pensioner_id);
                        } else {
                            let evt_r = app.biometric_gateway.release_finger();
                            app.push_biometric_event(evt_r);
                            app.pending_biometric_notice = Some((
                                false,
                                format!("Pensionista #{pensioner_id} identificado.").into(),
                            ));
                        }
                    }
                    Ok(IdentificationOutcome::NoMatch) => {
                        app.pending_biometric_notice = Some((
                            true,
                            "Huella no reconocida. El lector continúa esperando.".into(),
                        ));
                        if let Ok(wait_evt) = app.biometric_gateway.start_waiting() {
                            app.push_biometric_event(wait_evt);
                        }
                    }
                    Err(err) => {
                        app.pending_biometric_notice = Some((true, err.to_string().into()));
                    }
                }
                cx.notify();
            });
        }));
    }

    pub fn resume_waiting_after_modal(&mut self, cx: &mut Context<Self>) {
        self.biometric_modal_open = false;
        self.biometric_gateway.set_modal_locked(false);
        let rel = self.biometric_gateway.release_finger();
        self.push_biometric_event(rel);
        if self.biometric_gateway.state().is_connected() {
            if let Ok(wait_evt) = self.biometric_gateway.start_waiting() {
                self.push_biometric_event(wait_evt);
            }
        }
        cx.notify();
    }

    pub fn biometric_connect(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        match self.biometric_gateway.connect() {
            Ok(evt) => {
                self.push_biometric_event(evt);
                if let Ok(wait_evt) = self.biometric_gateway.start_waiting() {
                    self.push_biometric_event(wait_evt);
                }
                window.push_notification(
                    Notification::info("Lector simulado conectado en escucha continua."),
                    cx,
                );
            }
            Err(err) => {
                window.push_notification(Notification::error(err.to_string()), cx);
            }
        }
        cx.notify();
    }

    pub fn biometric_disconnect(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.biometric_task = None;
        self.biometric_op_id = self.biometric_op_id.wrapping_add(1);
        self.biometric_modal_open = false;
        let evt = self.biometric_gateway.disconnect();
        self.push_biometric_event(evt);
        window.push_notification(Notification::info("Lector simulado desconectado."), cx);
        cx.notify();
    }

    pub fn biometric_cancel(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.biometric_task = None;
        self.biometric_op_id = self.biometric_op_id.wrapping_add(1);
        self.biometric_modal_open = false;
        let evt = self.biometric_gateway.cancel();
        self.push_biometric_event(evt);
        window.push_notification(
            Notification::info("Operación cancelada. Lector en espera."),
            cx,
        );
        cx.notify();
    }

    pub fn biometric_simulate(
        &mut self,
        mode: SimulationMode,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let sig = match mode {
            SimulationMode::MatchActive => {
                ContinuousBiometricSignal::FingerMatch { pensioner_id: None }
            }
            SimulationMode::NoMatch => ContinuousBiometricSignal::FingerUnrecognized,
            SimulationMode::DeviceError => ContinuousBiometricSignal::DeviceError(
                "Lectura interrumpida en el sensor simulado.".into(),
            ),
        };
        self.emit_biometric_signal(sig, cx);
    }

    /// Abre automáticamente el modal de consumo cuando se identifica a un pensionista.
    pub fn open_auto_consumption_modal(
        &mut self,
        pensioner_id: u32,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some((person_name, bio_id, active, balance)) = (match &self.state.data {
            DataStatus::Ready(data) => data
                .pensioners
                .iter()
                .find(|p| p.id == pensioner_id)
                .map(|p| {
                    (
                        p.name(),
                        p.biometric_id.clone().unwrap_or_else(|| "—".into()),
                        p.active,
                        data.balance(p.id),
                    )
                }),
            _ => None,
        }) else {
            self.resume_waiting_after_modal(cx);
            return;
        };

        if !active {
            window.push_notification(
                Notification::error(format!(
                    "El pensionista {person_name} (#{pensioner_id}) está inactivo."
                )),
                cx,
            );
            self.resume_waiting_after_modal(cx);
            return;
        }

        let pricing = self.consumption_pricing;
        let special_price_input = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder("12.00")
                .default_value(format_decimal(pricing.special_default_cents))
        });
        let special_desc_input =
            cx.new(|cx| InputState::new(window, cx).placeholder("Ej. Seco a la norteña"));
        let third_party_input =
            cx.new(|cx| InputState::new(window, cx).placeholder("Nombre de quien recoge"));

        let initial_token = self.consumption_confirm_token;
        self.consumption_confirm_token = self.consumption_confirm_token.wrapping_add(1);

        let modal_state = cx.new(|_| AutoModalState {
            meal: Meal::Lunch,
            menu_kind: MenuKind::Economic,
            portions: 1,
            third_party: false,
            draft: vec![DraftConsumptionItem {
                id: 1,
                meal: Meal::Lunch,
                menu_kind: MenuKind::Economic,
                description: "Ración económica".into(),
                portions: 1,
                unit_price_cents: pricing.economic_for(Meal::Lunch),
            }],
            next_id: 2,
            error: None,
            submitting: false,
            token: initial_token,
        });

        let app_weak = cx.entity().downgrade();
        let app_on_cancel = app_weak.clone();

        window.open_dialog(cx, move |dialog, _, cx| {
            let st = modal_state.read(cx);
            let total_portions: u32 = st.draft.iter().map(|i| i.portions).sum();
            let total_cents: i64 = st.draft.iter().map(|i| i.subtotal_cents()).sum();
            let meal = st.meal;
            let menu_kind = st.menu_kind;
            let portions = st.portions;
            let third_party = st.third_party;
            let error_msg = st.error.clone();
            let submitting = st.submitting;
            let draft_items = st.draft.clone();

            let modal_for_meal_b = modal_state.clone();
            let modal_for_meal_l = modal_state.clone();
            let modal_for_econ = modal_state.clone();
            let modal_for_spec = modal_state.clone();
            let modal_for_dec = modal_state.clone();
            let modal_for_inc = modal_state.clone();
            let modal_for_add = modal_state.clone();
            let modal_for_switch = modal_state.clone();
            let modal_for_submit = modal_state.clone();

            let sp_price_add = special_price_input.clone();
            let sp_desc_add = special_desc_input.clone();
            let third_input_submit = third_party_input.clone();
            let app_submit = app_weak.clone();
            let app_cancel_btn = app_weak.clone();
            let app_close_cb = app_on_cancel.clone();

            dialog
                .width(px(540.))
                .title(format!("Atención automática · {person_name}"))
                .on_cancel(move |_, _, cx| {
                    let _ = app_close_cb.update(cx, |app, cx| {
                        app.resume_waiting_after_modal(cx);
                    });
                    true
                })
                .child(
                    v_flex()
                        .gap_3()
                        .child(
                            h_flex()
                                .justify_between()
                                .p_3()
                                .rounded(cx.theme().radius)
                                .border_1()
                                .border_color(cx.theme().primary)
                                .bg(cx.theme().muted)
                                .child(
                                    v_flex()
                                        .gap_0p5()
                                        .child(
                                            h_flex()
                                                .gap_2()
                                                .child(
                                                    div()
                                                        .text_sm()
                                                        .font_weight(FontWeight::BOLD)
                                                        .child(person_name.clone()),
                                                )
                                                .child(
                                                    Tag::secondary()
                                                        .small()
                                                        .child(format!("ID #{pensioner_id}")),
                                                )
                                                .child(
                                                    Tag::info()
                                                        .outline()
                                                        .small()
                                                        .child(bio_id.clone()),
                                                ),
                                        )
                                        .child(
                                            div()
                                                .text_xs()
                                                .text_color(cx.theme().muted_foreground)
                                                .child("Estado: Activo · Huella verificada 1:N"),
                                        ),
                                )
                                .child(
                                    v_flex()
                                        .items_end()
                                        .child(
                                            div()
                                                .text_xs()
                                                .text_color(cx.theme().muted_foreground)
                                                .child("Saldo pendiente"),
                                        )
                                        .child(
                                            div()
                                                .text_base()
                                                .font_weight(FontWeight::BOLD)
                                                .child(amount(balance)),
                                        ),
                                ),
                        )
                        .when_some(error_msg, |this, err| {
                            this.child(Alert::error("auto-modal-err", err))
                        })
                        .child(
                            h_flex()
                                .gap_2()
                                .flex_wrap()
                                .child(
                                    Button::new("auto-meal-breakfast")
                                        .xsmall()
                                        .when(meal == Meal::Breakfast, |b| b.primary())
                                        .when(meal != Meal::Breakfast, |b| b.outline())
                                        .label("Desayuno")
                                        .on_click(move |_, _, cx| {
                                            modal_for_meal_b.update(cx, |s, cx| {
                                                s.meal = Meal::Breakfast;
                                                cx.notify();
                                            });
                                        }),
                                )
                                .child(
                                    Button::new("auto-meal-lunch")
                                        .xsmall()
                                        .when(meal == Meal::Lunch, |b| b.primary())
                                        .when(meal != Meal::Lunch, |b| b.outline())
                                        .label("Almuerzo")
                                        .on_click(move |_, _, cx| {
                                            modal_for_meal_l.update(cx, |s, cx| {
                                                s.meal = Meal::Lunch;
                                                cx.notify();
                                            });
                                        }),
                                )
                                .child(
                                    Button::new("auto-menu-econ")
                                        .xsmall()
                                        .when(menu_kind == MenuKind::Economic, |b| b.primary())
                                        .when(menu_kind != MenuKind::Economic, |b| b.outline())
                                        .label(format!(
                                            "Económico ({})",
                                            amount(pricing.economic_for(meal))
                                        ))
                                        .on_click(move |_, _, cx| {
                                            modal_for_econ.update(cx, |s, cx| {
                                                s.menu_kind = MenuKind::Economic;
                                                cx.notify();
                                            });
                                        }),
                                )
                                .child(
                                    Button::new("auto-menu-spec")
                                        .xsmall()
                                        .when(menu_kind == MenuKind::Special, |b| b.primary())
                                        .when(menu_kind != MenuKind::Special, |b| b.outline())
                                        .label("Especial")
                                        .on_click(move |_, _, cx| {
                                            modal_for_spec.update(cx, |s, cx| {
                                                s.menu_kind = MenuKind::Special;
                                                cx.notify();
                                            });
                                        }),
                                ),
                        )
                        .when(menu_kind == MenuKind::Special, |this| {
                            this.child(
                                h_flex()
                                    .gap_2()
                                    .child(
                                        div()
                                            .w(px(120.))
                                            .child(Input::new(&special_price_input)),
                                    )
                                    .child(
                                        div()
                                            .flex_1()
                                            .child(Input::new(&special_desc_input)),
                                    ),
                            )
                        })
                        .child(
                            h_flex()
                                .justify_between()
                                .gap_2()
                                .child(
                                    h_flex()
                                        .gap_1p5()
                                        .child(div().text_xs().child("Raciones:"))
                                        .child(
                                            Button::new("auto-dec")
                                                .outline()
                                                .xsmall()
                                                .disabled(portions <= 1)
                                                .label("-")
                                                .on_click(move |_, _, cx| {
                                                    modal_for_dec.update(cx, |s, cx| {
                                                        s.portions = s.portions.saturating_sub(1).max(1);
                                                        cx.notify();
                                                    });
                                                }),
                                        )
                                        .child(
                                            div()
                                                .w(px(24.))
                                                .text_center()
                                                .text_xs()
                                                .font_weight(FontWeight::BOLD)
                                                .child(portions.to_string()),
                                        )
                                        .child(
                                            Button::new("auto-inc")
                                                .outline()
                                                .xsmall()
                                                .label("+")
                                                .on_click(move |_, _, cx| {
                                                    modal_for_inc.update(cx, |s, cx| {
                                                        s.portions = s.portions.saturating_add(1);
                                                        cx.notify();
                                                    });
                                                }),
                                        ),
                                )
                                .child(
                                    Button::new("auto-add-line")
                                        .outline()
                                        .xsmall()
                                        .icon(LucideIcon::Plus)
                                        .label("Agregar ración")
                                        .on_click(move |_, _, cx| {
                                            let raw_price = sp_price_add.read(cx).value().to_string();
                                            let raw_desc = sp_desc_add.read(cx).value().trim().to_string();
                                            modal_for_add.update(cx, |s, cx| {
                                                let unit_cents = match s.menu_kind {
                                                    MenuKind::Economic => pricing.economic_for(s.meal),
                                                    MenuKind::Special => match parse_price_cents(&raw_price) {
                                                        Ok(c) => c,
                                                        Err(err) => {
                                                            s.error = Some(err);
                                                            cx.notify();
                                                            return;
                                                        }
                                                    },
                                                };
                                                let desc = if s.menu_kind == MenuKind::Special {
                                                    if raw_desc.is_empty() {
                                                        "Plato especial".to_string()
                                                    } else {
                                                        raw_desc
                                                    }
                                                } else {
                                                    "Ración económica".to_string()
                                                };
                                                if let Some(ex) = s.draft.iter_mut().find(|i| {
                                                    i.meal == s.meal
                                                        && i.menu_kind == s.menu_kind
                                                        && i.unit_price_cents == unit_cents
                                                        && i.description == desc
                                                }) {
                                                    ex.portions += s.portions;
                                                } else {
                                                    let id = s.next_id;
                                                    s.next_id += 1;
                                                    s.draft.push(DraftConsumptionItem {
                                                        id,
                                                        meal: s.meal,
                                                        menu_kind: s.menu_kind,
                                                        description: desc,
                                                        portions: s.portions,
                                                        unit_price_cents: unit_cents,
                                                    });
                                                }
                                                s.portions = 1;
                                                s.error = None;
                                                cx.notify();
                                            });
                                        }),
                                ),
                        )
                        .child(
                            v_flex().gap_1p5().children(draft_items.into_iter().map(|item| {
                                let item_id = item.id;
                                let modal_rm = modal_state.clone();
                                h_flex()
                                    .justify_between()
                                    .px_2p5()
                                    .py_1p5()
                                    .rounded(cx.theme().radius)
                                    .border_1()
                                    .border_color(cx.theme().border)
                                    .child(
                                        div().text_xs().child(format!(
                                            "{} · {} ({}) × {}",
                                            item.meal.label(),
                                            item.description,
                                            item.menu_kind.label(),
                                            item.portions
                                        )),
                                    )
                                    .child(
                                        h_flex()
                                            .gap_1p5()
                                            .child(
                                                div()
                                                    .text_xs()
                                                    .font_weight(FontWeight::SEMIBOLD)
                                                    .child(amount(item.subtotal_cents())),
                                            )
                                            .child(
                                                Button::new(format!("auto-rm-{item_id}"))
                                                    .ghost()
                                                    .xsmall()
                                                    .icon(LucideIcon::X)
                                                    .on_click(move |_, _, cx| {
                                                        modal_rm.update(cx, |s, cx| {
                                                            s.draft.retain(|i| i.id != item_id);
                                                            cx.notify();
                                                        });
                                                    }),
                                            ),
                                    )
                            })),
                        )
                        .child(
                            h_flex()
                                .justify_between()
                                .child(div().text_xs().child("Recogido por tercero"))
                                .child(
                                    Switch::new("auto-third-party")
                                        .small()
                                        .checked(third_party)
                                        .on_change(move |checked: &bool, _, cx| {
                                            modal_for_switch.update(cx, |s, cx| {
                                                s.third_party = *checked;
                                                cx.notify();
                                            });
                                        }),
                                ),
                        )
                        .when(third_party, |this| {
                            this.child(Input::new(&third_party_input))
                        })
                        .child(
                            h_flex()
                                .justify_between()
                                .p_2p5()
                                .rounded(cx.theme().radius)
                                .bg(cx.theme().muted)
                                .child(
                                    div()
                                        .text_xs()
                                        .child(format!("Total raciones: {total_portions}")),
                                )
                                .child(
                                    div()
                                        .text_base()
                                        .font_weight(FontWeight::BOLD)
                                        .child(format!("Total: {}", amount(total_cents))),
                                ),
                        ),
                )
                .footer(
                    h_flex()
                        .gap_3()
                        .justify_end()
                        .child(
                            Button::new("auto-modal-cancel")
                                .outline()
                                .label("Cancelar")
                                .on_click(move |_, window, cx| {
                                    let _ = app_cancel_btn.update(cx, |app, cx| {
                                        app.resume_waiting_after_modal(cx);
                                    });
                                    window.close_dialog(cx);
                                }),
                        )
                        .child(
                            Button::new("auto-modal-confirm")
                                .primary()
                                .disabled(submitting)
                                .label("Confirmar consumo")
                                .on_click(move |_, window, cx| {
                                    let pickup_name = third_input_submit.read(cx).value().trim().to_string();
                                    let state_snap = modal_for_submit.read(cx);
                                    if state_snap.submitting {
                                        return;
                                    }
                                    if state_snap.draft.is_empty() {
                                        modal_for_submit.update(cx, |s, cx| {
                                            s.error = Some("Agrega al menos una ración.".into());
                                            cx.notify();
                                        });
                                        return;
                                    }
                                    if state_snap.third_party && pickup_name.chars().count() < 2 {
                                        modal_for_submit.update(cx, |s, cx| {
                                            s.error = Some("Ingresa el nombre de quien recoge.".into());
                                            cx.notify();
                                        });
                                        return;
                                    }
                                    let items = state_snap.draft.clone();
                                    let token = state_snap.token;
                                    let pickup = state_snap.third_party.then_some(pickup_name);

                                    modal_for_submit.update(cx, |s, _| s.submitting = true);

                                    let res = app_submit.update(cx, |app, cx| {
                                        let date = app.state.dashboard_date;
                                        let out = match &mut app.state.data {
                                            DataStatus::Ready(data) => data.record_consumptions(
                                                date,
                                                "13:20",
                                                pensioner_id,
                                                &items,
                                                pickup.as_deref(),
                                                token,
                                                &mut app.consumption_processed_tokens,
                                            ),
                                            _ => Err("Los datos no están disponibles.".into()),
                                        };
                                        if out.is_ok() {
                                            app.resume_waiting_after_modal(cx);
                                        }
                                        out
                                    });

                                    match res {
                                        Ok(Ok(count)) => {
                                            window.close_dialog(cx);
                                            window.push_notification(
                                                Notification::success(format!(
                                                    "Consumo confirmado ({count} línea(s)). Lector listo para la siguiente huella."
                                                )),
                                                cx,
                                            );
                                        }
                                        Ok(Err(err)) => {
                                            modal_for_submit.update(cx, |s, cx| {
                                                s.submitting = false;
                                                s.error = Some(err);
                                                cx.notify();
                                            });
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
}

pub fn render(
    state: &AppState,
    reader_state: &BiometricReaderState,
    event_log: &[SharedString],
    cx: &Context<DesktopApp>,
    app: &App,
) -> impl IntoElement {
    let theme = app.theme();
    let is_connected = reader_state.is_connected();
    let is_identifying = matches!(reader_state, BiometricReaderState::Identifying);
    let can_identify = allowed(state, "biometrics.identify");

    let (ring_color, icon_color) = match reader_state {
        BiometricReaderState::Disconnected => (theme.border, theme.muted_foreground),
        BiometricReaderState::Connected => (theme.info, theme.info),
        BiometricReaderState::WaitingFingerprint => (theme.primary, theme.primary),
        BiometricReaderState::Identifying => (theme.info, theme.info),
        BiometricReaderState::Identified { .. } => (theme.success, theme.success),
        BiometricReaderState::Unrecognized => (theme.warning, theme.warning),
        BiometricReaderState::Error(_) => (theme.danger, theme.danger),
    };

    let identified_person = match (reader_state, &state.data) {
        (
            BiometricReaderState::Identified {
                pensioner_id,
                biometric_id,
            },
            DataStatus::Ready(data),
        ) => data
            .pensioners
            .iter()
            .find(|p| p.id == *pensioner_id)
            .map(|p| (p, biometric_id.clone(), data.balance(p.id))),
        _ => None,
    };

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
                                .child("Identificación biométrica continua"),
                        )
                        .child(
                            div()
                                .text_sm()
                                .text_color(theme.muted_foreground)
                                .child("Modo automático: al detectar huella se abre el modal de consumo desde cualquier pantalla autorizada"),
                        ),
                )
                .child(
                    h_flex()
                        .gap_2()
                        .child(state_tag(reader_state))
                        .when(!is_connected, |this| {
                            this.child(
                                Button::new("bio-connect")
                                    .primary()
                                    .small()
                                    .icon(LucideIcon::ShieldCheck)
                                    .label("Conectar lector")
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        this.biometric_connect(window, cx);
                                    })),
                            )
                        })
                        .when(is_connected, |this| {
                            this.child(
                                Button::new("bio-disconnect")
                                    .outline()
                                    .small()
                                    .label("Desconectar")
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        this.biometric_disconnect(window, cx);
                                    })),
                            )
                        }),
                ),
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
                        .title("Estado visual del lector (escucha continua)")
                        .child(
                            v_flex()
                                .gap_4()
                                .items_center()
                                .py_4()
                                .child(
                                    div()
                                        .size(px(104.))
                                        .rounded_full()
                                        .border_2()
                                        .border_color(ring_color)
                                        .bg(theme.muted)
                                        .flex()
                                        .items_center()
                                        .justify_center()
                                        .child(if is_identifying {
                                            Spinner::new().large().into_any_element()
                                        } else {
                                            Icon::new(LucideIcon::FingerprintPattern)
                                                .size(px(46.))
                                                .text_color(icon_color)
                                                .into_any_element()
                                        }),
                                )
                                .child(
                                    v_flex()
                                        .gap_1()
                                        .items_center()
                                        .child(
                                            div()
                                                .text_lg()
                                                .font_weight(FontWeight::BOLD)
                                                .child(reader_state.label()),
                                        )
                                        .child(
                                            div()
                                                .text_sm()
                                                .text_color(theme.muted_foreground)
                                                .child(reader_state.description()),
                                        ),
                                )
                                .child(
                                    h_flex()
                                        .gap_2()
                                        .flex_wrap()
                                        .justify_center()
                                        .child(
                                            Button::new("bio-sim-match")
                                                .primary()
                                                .small()
                                                .disabled(!is_connected || is_identifying || !can_identify)
                                                .icon(LucideIcon::FingerprintPattern)
                                                .label("Simular dedo en lector")
                                                .on_click(cx.listener(|this, _, window, cx| {
                                                    this.biometric_simulate(
                                                        SimulationMode::MatchActive,
                                                        window,
                                                        cx,
                                                    );
                                                })),
                                        )
                                        .child(
                                            Button::new("bio-sim-nomatch")
                                                .outline()
                                                .small()
                                                .disabled(!is_connected || is_identifying || !can_identify)
                                                .label("Simular no reconocido")
                                                .on_click(cx.listener(|this, _, window, cx| {
                                                    this.biometric_simulate(
                                                        SimulationMode::NoMatch,
                                                        window,
                                                        cx,
                                                    );
                                                })),
                                        )
                                        .child(
                                            Button::new("bio-sim-error")
                                                .outline()
                                                .small()
                                                .disabled(!is_connected || is_identifying || !can_identify)
                                                .label("Simular error")
                                                .on_click(cx.listener(|this, _, window, cx| {
                                                    this.biometric_simulate(
                                                        SimulationMode::DeviceError,
                                                        window,
                                                        cx,
                                                    );
                                                })),
                                        )
                                        .when(is_connected, |this| {
                                            this.child(
                                                Button::new("bio-cancel")
                                                    .ghost()
                                                    .small()
                                                    .label("Cancelar / Reactivar espera")
                                                    .on_click(cx.listener(|this, _, window, cx| {
                                                        this.biometric_cancel(window, cx);
                                                    })),
                                            )
                                        })
                                        .child(
                                            Button::new("bio-manual-search")
                                                .outline()
                                                .small()
                                                .icon(LucideIcon::Search)
                                                .label("Búsqueda manual")
                                                .on_click(cx.listener(|this, _, _, cx| {
                                                    this.navigate_to(Page::Consumos, cx);
                                                })),
                                        ),
                                ),
                        ),
                )
                .child(
                    v_flex()
                        .flex_1()
                        .min_w(px(320.))
                        .gap_4()
                        .child(
                            GroupBox::new()
                                .outline()
                                .title("Última identificación")
                                .child(if let Some((person, bio_id, balance)) = identified_person {
                                    let pid = person.id;
                                    v_flex()
                                        .gap_3()
                                        .child(
                                            h_flex()
                                                .gap_2()
                                                .flex_wrap()
                                                .child(
                                                    div()
                                                        .text_lg()
                                                        .font_weight(FontWeight::BOLD)
                                                        .child(person.name()),
                                                )
                                                .child(Tag::secondary().small().child(format!("ID #{pid}")))
                                                .child(Tag::info().outline().small().child(bio_id)),
                                        )
                                        .child(
                                            div()
                                                .text_sm()
                                                .text_color(theme.muted_foreground)
                                                .child(format!(
                                                    "Estado: {} · Saldo pendiente: {}",
                                                    if person.active { "Activo" } else { "Inactivo" },
                                                    amount(balance)
                                                )),
                                        )
                                        .child(
                                            h_flex()
                                                .gap_2()
                                                .child(
                                                    Button::new("bio-open-modal-again")
                                                        .primary()
                                                        .small()
                                                        .icon(LucideIcon::Utensils)
                                                        .label("Abrir modal de consumo")
                                                        .on_click(cx.listener(move |this, _, window, cx| {
                                                            this.open_auto_consumption_modal(pid, window, cx);
                                                        })),
                                                )
                                                .child(
                                                    Button::new("bio-view-pensioner")
                                                        .outline()
                                                        .small()
                                                        .icon(LucideIcon::Eye)
                                                        .label("Ver ficha")
                                                        .on_click(cx.listener(move |this, _, _, cx| {
                                                            this.selected_pensioner = Some(pid);
                                                            this.navigate_to(Page::Pensionistas, cx);
                                                        })),
                                                ),
                                        )
                                        .into_any_element()
                                } else {
                                    div()
                                        .py_4()
                                        .text_sm()
                                        .text_color(theme.muted_foreground)
                                        .child("Esperando lectura biométrica automática.")
                                        .into_any_element()
                                }),
                        )
                        .child(
                            GroupBox::new()
                                .outline()
                                .title("Eventos internos del servicio")
                                .child(if event_log.is_empty() {
                                    div()
                                        .text_xs()
                                        .text_color(theme.muted_foreground)
                                        .child("Sin eventos emitidos todavía.")
                                        .into_any_element()
                                } else {
                                    v_flex()
                                        .gap_1p5()
                                        .children(event_log.iter().map(|entry| {
                                            div()
                                                .text_xs()
                                                .text_color(theme.muted_foreground)
                                                .child(entry.clone())
                                        }))
                                        .into_any_element()
                                }),
                        ),
                ),
        )
}
