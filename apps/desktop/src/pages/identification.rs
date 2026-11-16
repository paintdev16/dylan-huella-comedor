use std::time::Duration;

use gpui_kit::{
    assets::IconName as LucideIcon,
    component::{
        alert::Alert,
        button::{Button, ButtonVariants as _},
        group_box::{GroupBox, GroupBoxVariants as _},
        h_flex,
        notification::Notification,
        spinner::Spinner,
        tag::Tag,
        v_flex, ActiveTheme as _, Disableable as _, Icon, Sizable as _, WindowExt as _,
    },
    div,
    prelude::FluentBuilder as _,
    px, App, AppContext as _, Context, FontWeight, IntoElement, ParentElement, SharedString,
    Styled, Window,
};

use crate::{
    biometric_service::{
        BiometricError, BiometricEvent, BiometricGateway as _, BiometricReaderState,
        IdentificationOutcome, SimulatedBiometricGateway,
    },
    layout::DesktopApp,
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

impl DesktopApp {
    pub fn push_biometric_event(&mut self, event: BiometricEvent) {
        let label: SharedString = match &event {
            BiometricEvent::ReaderStateChanged(s) => format!("Estado -> {}", s.label()).into(),
            BiometricEvent::Enrolled {
                pensioner_id,
                biometric_id,
            } => format!("Enrolado #{pensioner_id} ({biometric_id})").into(),
            BiometricEvent::OperationCancelled => "Operación cancelada -> Esperando huella".into(),
        };
        self.biometric_event_log.insert(0, label);
        self.biometric_event_log.truncate(6);
    }

    pub fn biometric_connect(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        match self.biometric_gateway.connect() {
            Ok(evt) => {
                self.push_biometric_event(evt);
                window.push_notification(Notification::info("Conexión simulada iniciada."), cx);
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
        self.pending_consumption_pensioner = None;
        let evt = self.biometric_gateway.disconnect();
        self.push_biometric_event(evt);
        window.push_notification(Notification::info("Lector simulado desconectado."), cx);
        cx.notify();
    }

    pub fn biometric_start_waiting(&mut self, cx: &mut Context<Self>) {
        self.biometric_task = None;
        self.biometric_op_id = self.biometric_op_id.wrapping_add(1);
        self.pending_consumption_pensioner = None;
        if let Ok(evt) = self.biometric_gateway.start_waiting() {
            self.push_biometric_event(evt);
        }
        cx.notify();
    }

    pub fn biometric_cancel(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.biometric_task = None;
        self.biometric_op_id = self.biometric_op_id.wrapping_add(1);
        self.pending_consumption_pensioner = None;
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
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self.state.auth.can_view(Page::Identificacion)
            || !allowed(&self.state, "biometrics.identify")
        {
            window.push_notification(
                Notification::error("No tienes permiso para identificar pensionistas."),
                cx,
            );
            return;
        }
        if !matches!(
            self.biometric_gateway.state(),
            BiometricReaderState::WaitingFingerprint
        ) {
            window.push_notification(
                Notification::error("Activa el modo de espera simulado antes de identificar."),
                cx,
            );
            return;
        }

        let candidates = match &self.state.data {
            DataStatus::Ready(data) => data.biometric_candidates(),
            _ => {
                window.push_notification(
                    Notification::error("Los datos de pensionistas no están disponibles."),
                    cx,
                );
                return;
            }
        };

        match self.biometric_gateway.begin_identifying() {
            Ok(evt) => self.push_biometric_event(evt),
            Err(err) => {
                window.push_notification(Notification::error(err.to_string()), cx);
                return;
            }
        }
        self.pending_consumption_pensioner = None;
        self.biometric_op_id = self.biometric_op_id.wrapping_add(1);
        let op_id = self.biometric_op_id;
        cx.notify();

        let bg_task = cx.background_spawn(async move {
            std::thread::sleep(Duration::from_millis(450));
            match mode {
                SimulationMode::MatchActive => {
                    Ok(SimulatedBiometricGateway::simulate_match(&candidates, None))
                }
                SimulationMode::NoMatch => Ok(IdentificationOutcome::NoMatch),
                SimulationMode::DeviceError => Err(BiometricError::DeviceError(
                    "Error de demostración del servicio.".into(),
                )),
            }
        });

        self.biometric_task = Some(cx.spawn(async move |this, cx| {
            let outcome = bg_task.await;
            let _ = this.update(cx, |app, cx| {
                if app.biometric_op_id != op_id || !app.state.auth.can_view(Page::Identificacion) || !allowed(&app.state, "biometrics.identify") {
                    return;
                }
                let outcome = match outcome {
                    Ok(IdentificationOutcome::Match { pensioner_id, biometric_id }) => {
                        let valid = matches!(&app.state.data, DataStatus::Ready(data) if data.pensioners.iter().any(|p| p.id == pensioner_id && p.active && p.fingerprint_registered && p.biometric_id.as_deref() == Some(biometric_id.as_str())));
                        if valid { Ok(IdentificationOutcome::Match { pensioner_id, biometric_id }) } else { Ok(IdentificationOutcome::NoMatch) }
                    },
                    other => other,
                };
                let evt = app.biometric_gateway.finish_identification(outcome);
                app.push_biometric_event(evt);
                app.biometric_task = None;
                cx.notify();
            });
        }));
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
    let is_waiting = matches!(reader_state, BiometricReaderState::WaitingFingerprint);
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
            .find(|p| p.id == *pensioner_id && p.active && p.fingerprint_registered && p.biometric_id.as_deref() == Some(biometric_id.as_str()))
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
                                .child("Identificación biométrica"),
                        )
                        .child(
                            div()
                                .text_sm()
                                .text_color(theme.muted_foreground)
                                .child("Modo de demostración · No se captura ninguna huella real"),
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
                                    .label("Conectar simulación")
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
                        .title("Estado visual del lector")
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
                                                .disabled(!is_waiting || !can_identify)
                                                .icon(LucideIcon::FingerprintPattern)
                                                .label("Simular identificación")
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
                                                .disabled(!is_waiting || !can_identify)
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
                                                .disabled(!is_waiting || !can_identify)
                                                .label("Simular error")
                                                .on_click(cx.listener(|this, _, window, cx| {
                                                    this.biometric_simulate(
                                                        SimulationMode::DeviceError,
                                                        window,
                                                        cx,
                                                    );
                                                })),
                                        )
                                        .when(
                                            matches!(
                                                reader_state,
                                                BiometricReaderState::Connected
                                                    | BiometricReaderState::Identified { .. }
                                                    | BiometricReaderState::Unrecognized
                                                    | BiometricReaderState::Error(_)
                                            ),
                                            |this| {
                                                this.child(
                                                    Button::new("bio-wait")
                                                        .ghost()
                                                        .small()
                                                        .label("Modo espera")
                                                        .on_click(cx.listener(|this, _, _, cx| {
                                                            this.biometric_start_waiting(cx);
                                                        })),
                                                )
                                            },
                                        )
                                        .when(is_connected, |this| {
                                            this.child(
                                                Button::new("bio-cancel")
                                                    .ghost()
                                                    .small()
                                                    .label("Cancelar operación")
                                                    .on_click(cx.listener(|this, _, window, cx| {
                                                        this.biometric_cancel(window, cx);
                                                    })),
                                            )
                                        }),
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
                                .title("Resultado de identificación")
                                .child(if let Some((person, bio_id, balance)) = identified_person {
                                    let pid = person.id;
                                    v_flex()
                                        .gap_3()
                                        .child(
                                            h_flex()
                                                .gap_2()
                                                .flex_wrap()
                                                .when(allowed(state, "payments.view") && allowed(state, "consumptions.view"), |this| this.child(
                                                    div()
                                                        .text_lg()
                                                        .font_weight(FontWeight::BOLD)
                                                        .child(person.name()),
                                                )
                                                .child(Tag::secondary().small().child(format!("ID #{pid}")))
                                                .child(Tag::info().outline().small().child(bio_id)),
                                        )
                                        .child(
                                            h_flex()
                                                .gap_2()
                                                .child(if person.active {
                                                    Tag::success().outline().small().child("Activo")
                                                } else {
                                                    Tag::secondary().small().child("Inactivo")
                                                })
                                                .child(
                                                    div()
                                                        .text_sm()
                                                        .text_color(theme.muted_foreground)
                                                        .child(format!(
                                                            "Saldo pendiente: {}",
                                                            amount(balance)
                                                        )),
                                                )),
                                        )
                                        .child(
                                            Alert::info(
                                                "no-auto-consumption",
                                                "No se registra ningún consumo automáticamente al identificar.",
                                            ),
                                        )
                                        .child(
                                            h_flex()
                                                .gap_2()
                                                .flex_wrap()
                                                .when(allowed(state, "consumptions.create") && state.auth.can_view(Page::Consumos) && person.active, |this| this.child(
                                                    Button::new("bio-continue-consumption")
                                                        .primary()
                                                        .small()
                                                        .icon(LucideIcon::Utensils)
                                                        .label("Continuar a registro de consumo")
                                                        .on_click(cx.listener(
                                                            move |this, _, _, cx| {
                                                                    let valid = matches!((&this.state.data, this.biometric_gateway.state()), (DataStatus::Ready(data), BiometricReaderState::Identified { pensioner_id, biometric_id }) if *pensioner_id == pid && data.pensioners.iter().any(|p| p.id == pid && p.active && p.fingerprint_registered && p.biometric_id.as_deref() == Some(biometric_id.as_str())));
                                                                if valid && this.state.auth.can_view(Page::Consumos) && allowed(&this.state, "consumptions.create") && matches!(this.biometric_gateway.state(), BiometricReaderState::Identified { pensioner_id, .. } if *pensioner_id == pid) {
                                                                    this.pending_consumption_pensioner = Some(pid);
                                                                    this.navigate_to(Page::Consumos, cx);
                                                                }
                                                            },
                                                        )),
                                                ))
                                                .when(state.auth.can_view(Page::Pensionistas), |this| this.child(
                                                    Button::new("bio-view-pensioner")
                                                        .outline()
                                                        .small()
                                                        .icon(LucideIcon::Eye)
                                                        .label("Ver ficha")
                                                        .on_click(cx.listener(
                                                            move |this, _, _, cx| {
                                                                if this.state.auth.can_view(Page::Pensionistas) {
                                                                    this.navigate_to(Page::Pensionistas, cx);
                                                                    this.selected_pensioner = Some(pid);
                                                                    cx.notify();
                                                                }
                                                            },
                                                        )),
                                                )),
                                        )
                                        .into_any_element()
                                } else {
                                    div()
                                        .py_4()
                                        .text_sm()
                                        .text_color(theme.muted_foreground)
                                        .child("Aún no hay un pensionista identificado en esta operación.")
                                        .into_any_element()
                                }),
                        )
                        .child(
                            GroupBox::new()
                                .outline()
                                .title("Actividad de la simulación")
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
