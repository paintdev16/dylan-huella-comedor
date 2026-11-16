use std::time::Duration;

use crate::{
    biometric_service::{
        BiometricEvent, EnrollmentStep, generate_unique_biometric_id,
    },
    components::common::{SemanticTone, metric_card},
    layout::DesktopApp,
    mock_data::{MockData, PaymentKind, Pensioner},
    state::{AppState, DataStatus, Page},
};
use gpui_kit::{
    App, AppContext as _, Context, Entity, FontWeight, IntoElement, ParentElement, Styled,
    WeakEntity, Window,
    assets::IconName as LucideIcon,
    component::{
        ActiveTheme as _, Disableable as _, Icon, Sizable as _, Size, WindowExt as _,
        alert::Alert,
        button::{Button, ButtonVariants as _},
        checkbox::Checkbox,
        empty::{Empty, EmptyDescription, EmptyHeader, EmptyMedia, EmptyMediaVariant, EmptyTitle},
        group_box::{GroupBox, GroupBoxVariants as _},
        h_flex,
        input::{Input, InputState},
        notification::Notification,
        spinner::Spinner,
        switch::Switch,
        table::{Column, ColumnSort, DataTable, TableDelegate, TableState},
        tag::Tag,
        v_flex,
    },
    div,
    prelude::FluentBuilder as _,
    px,
};

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Filter {
    All,
    Active,
    Inactive,
}

fn allowed(state: &AppState, permission: &str) -> bool {
    state.auth.user().is_some_and(|user| user.can(permission))
}

fn amount(cents: i64) -> String {
    let sign = if cents < 0 { "-" } else { "" };
    let abs = cents.abs();
    format!("{sign}S/ {}.{:02}", abs / 100, abs % 100)
}

fn balance_cell(
    id: u32,
    is_credit: bool,
    cents: i64,
    revealed: bool,
    app_weak: &WeakEntity<DesktopApp>,
    theme_cx: &App,
) -> impl IntoElement {
    let category = if is_credit { "saldo a favor" } else { "saldo en contra" };
    let action = if revealed { "Ocultar" } else { "Mostrar" };
    let label = if revealed { amount(cents) } else { "••••••".to_string() };
    let color = if cents == 0 {
        theme_cx.theme().muted_foreground
    } else if is_credit {
        theme_cx.theme().success
    } else {
        theme_cx.theme().danger
    };
    let app_btn = app_weak.clone();
    div().w(px(110.)).flex_shrink_0().child(
        Button::new(format!("pensioner-balance-{id}-{is_credit}"))
            .ghost().small().icon(LucideIcon::Eye)
            .text_color(color)
            .label(label)
            .accessibility_label(format!("{action} {category}"))
            .tooltip(format!("{action} {category}"))
            .on_click(move |_, _, cx| {
                let _ = app_btn.update(cx, |this, cx| {
                    if !this.state.auth.can_view(Page::Pensionistas) || this.state.page != Page::Pensionistas {
                        return;
                    }
                    let key = (id, is_credit);
                    if !this.revealed_pensioner_balances.remove(&key) {
                        this.revealed_pensioner_balances.insert(key);
                    }
                    cx.notify();
                });
            }),
    )
}

fn status_tag(active: bool) -> Tag {
    if active {
        Tag::success().outline().small().child("Activo")
    } else {
        Tag::secondary().small().child("Inactivo")
    }
}

fn biometric_tag(status: &str) -> Tag {
    match status {
        "Registrado" => Tag::success().small().child("Registrado"),
        "Pendiente" => Tag::warning().small().child("Pendiente"),
        _ => Tag::secondary().small().child("Inactivo"),
    }
}

#[derive(Clone)]
struct NewEnrollFormState {
    enabled: bool,
    consent_given: bool,
    step: EnrollmentStep,
    op_seq: u64,
}

fn enrollment_tag(step: &EnrollmentStep) -> Tag {
    match step {
        EnrollmentStep::Idle => Tag::secondary().small().child("Pendiente"),
        EnrollmentStep::WaitingFinger => Tag::warning().outline().small().child("Esperando dedo"),
        EnrollmentStep::Capturing => Tag::info().small().child("Capturando"),
        EnrollmentStep::Verifying => Tag::info().small().child("Verificando"),
        EnrollmentStep::Registered { .. } => Tag::success().small().child("Registrada (simulada)"),
        EnrollmentStep::Error(_) => Tag::danger().small().child("Error"),
    }
}

/// Simula secuencialmente las etapas: esperando dedo → capturando → verificando → registrada.
fn start_simulated_enrollment_capture(
    enroll_state: Entity<NewEnrollFormState>,
    app: WeakEntity<DesktopApp>,
    initial_wait_ms: u64,
    cx: &mut App,
) {
    let seq = enroll_state.update(cx, |s, cx| {
        s.op_seq = s.op_seq.wrapping_add(1);
        s.step = if initial_wait_ms > 0 {
            EnrollmentStep::WaitingFinger
        } else {
            EnrollmentStep::Capturing
        };
        cx.notify();
        s.op_seq
    });
    let enroll_weak = enroll_state.downgrade();
    cx.spawn(async move |cx| {
        if initial_wait_ms > 0 {
            cx.background_spawn(async move {
                std::thread::sleep(Duration::from_millis(initial_wait_ms));
            })
            .await;
            let still_valid = enroll_weak
                .update(cx, |s, cx| {
                    if s.op_seq != seq || !s.enabled || !s.consent_given {
                        return false;
                    }
                    s.step = EnrollmentStep::Capturing;
                    cx.notify();
                    true
                })
                .unwrap_or(false);
            if !still_valid {
                return;
            }
        }
        cx.background_spawn(async move {
            std::thread::sleep(Duration::from_millis(220));
        })
        .await;
        let still_valid = enroll_weak
            .update(cx, |s, cx| {
                if s.op_seq != seq || !s.enabled || !s.consent_given {
                    return false;
                }
                s.step = EnrollmentStep::Verifying;
                cx.notify();
                true
            })
            .unwrap_or(false);
        if !still_valid {
            return;
        }
        cx.background_spawn(async move {
            std::thread::sleep(Duration::from_millis(220));
        })
        .await;
        let candidates = app
            .read_with(cx, |app, _| match &app.state.data {
                DataStatus::Ready(d) => d.biometric_candidates(),
                _ => Vec::new(),
            })
            .unwrap_or_default();
        let generated = generate_unique_biometric_id(&candidates);
        let _ = enroll_weak.update(cx, |s, cx| {
            if s.op_seq != seq || !s.enabled || !s.consent_given {
                return;
            }
            s.step = EnrollmentStep::Registered {
                biometric_id: generated,
            };
            cx.notify();
        });
    })
    .detach();
}

impl DesktopApp {
    fn has_pensioner_permission(&self, permission: &str) -> bool {
        self.state.auth.can_view(Page::Pensionistas) && allowed(&self.state, permission)
    }

    fn pensioner_form(&mut self, id: Option<u32>, window: &mut Window, cx: &mut Context<Self>) {
        let permission = if id.is_some() {
            "pensioners.update"
        } else {
            "pensioners.create"
        };
        if !self.has_pensioner_permission(permission) {
            window.push_notification(
                Notification::error("No tienes permiso para realizar esta acción."),
                cx,
            );
            return;
        }
        if self.biometric_modal_open || window.has_active_dialog(cx) {
            window.push_notification(Notification::warning("Ya hay una operación en curso."), cx);
            return;
        }
        let person = match (&self.state.data, id) {
            (DataStatus::Ready(data), Some(id)) => {
                data.pensioners.iter().find(|p| p.id == id).cloned()
            }
            _ => None,
        };
        if id.is_some() && person.is_none() {
            window.push_notification(Notification::error("Pensionista no encontrado."), cx);
            return;
        }

        let is_new = id.is_none();
        let can_enroll = self.has_pensioner_permission("biometrics.enroll");
        let reader_available = self.biometric_gateway.state().is_connected();

        // Suspende la identificación de consumos durante el enrolamiento de un nuevo pensionista.
        if is_new && reader_available {
            self.biometric_modal_open = true;
            self.biometric_gateway.set_modal_locked(true);
        }

        let enroll_state = cx.new(|_| NewEnrollFormState {
            enabled: false,
            consent_given: false,
            step: EnrollmentStep::Idle,
            op_seq: 0,
        });

        let first = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder("Ej. Ana María")
                .default_value(person.as_ref().map(|p| p.first_name.as_str()).unwrap_or(""))
        });
        let last = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder("Ej. Torres Quispe")
                .default_value(person.as_ref().map(|p| p.last_name.as_str()).unwrap_or(""))
        });
        let phone = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder("Ej. 987654321")
                .default_value(
                    person
                        .as_ref()
                        .and_then(|p| p.phone.as_deref())
                        .unwrap_or(""),
                )
        });
        let form_error: Entity<Option<String>> = cx.new(|_| None);
        let app = cx.entity().downgrade();
        let app_on_cancel_dlg = app.clone();
        let app_on_cancel_btn = app.clone();
        let app_on_save = app.clone();

        window.open_dialog(cx, move |dialog, _, cx| {
            let first_field = first.clone();
            let last_field = last.clone();
            let phone_field = phone.clone();
            let form_error_view = form_error.clone();
            let form_error_submit = form_error.clone();
            let enroll_view = enroll_state.clone();
            let enroll_switch = enroll_state.clone();
            let enroll_consent = enroll_state.clone();
            let enroll_capture = enroll_state.clone();
            let enroll_fail = enroll_state.clone();
            let enroll_submit = enroll_state.clone();

            let app_cap = app.clone();
            let app_save = app_on_save.clone();
            let app_cancel_dlg = app_on_cancel_dlg.clone();
            let app_cancel_btn = app_on_cancel_btn.clone();

            let current_error = form_error_view.read(cx).clone();
            let enr = enroll_view.read(cx).clone();
            let is_busy = matches!(
                enr.step,
                EnrollmentStep::Capturing | EnrollmentStep::Verifying
            );

            dialog
                .width(px(520.))
                .title(if id.is_some() {
                    "Editar pensionista".to_string()
                } else {
                    "Nuevo pensionista".to_string()
                })
                .on_cancel(move |_, _, cx| {
                    if is_new {
                        let _ = app_cancel_dlg.update(cx, |this, cx| {
                            this.resume_waiting_after_modal(cx);
                        });
                    }
                    true
                })
                .child(
                    v_flex()
                        .gap_3p5()
                        .when_some(current_error, |this, err| {
                            this.child(Alert::error("pensioner-form-error", err))
                        })
                        .child(
                            v_flex()
                                .gap_1()
                                .child(div().text_sm().font_weight(FontWeight::MEDIUM).child("Nombres *"))
                                .child(Input::new(&first)),
                        )
                        .child(
                            v_flex()
                                .gap_1()
                                .child(div().text_sm().font_weight(FontWeight::MEDIUM).child("Apellidos *"))
                                .child(Input::new(&last)),
                        )
                        .child(
                            v_flex()
                                .gap_1()
                                .child(
                                    div()
                                        .text_sm()
                                        .font_weight(FontWeight::MEDIUM)
                                        .child("Celular (opcional)"),
                                )
                                .child(Input::new(&phone)),
                        )
                        .when(is_new, |this| {
                            this.child(
                                v_flex()
                                    .gap_2p5()
                                    .p_3()
                                    .rounded(cx.theme().radius)
                                    .border_1()
                                    .border_color(cx.theme().border)
                                    .bg(cx.theme().muted)
                                    .child(
                                        h_flex()
                                            .justify_between()
                                            .gap_2()
                                            .child(
                                                v_flex()
                                                    .gap_0p5()
                                                    .child(
                                                        div()
                                                            .text_sm()
                                                            .font_weight(FontWeight::SEMIBOLD)
                                                            .child("Registrar huella biométrica"),
                                                    )
                                                    .child(
                                                        div()
                                                            .text_xs()
                                                            .text_color(cx.theme().muted_foreground)
                                                            .child(if !can_enroll {
                                                                "Sin permiso de enrolamiento; se guardará como Pendiente."
                                                            } else if !reader_available {
                                                                "Lector no conectado; se guardará como Pendiente (alternativa manual)."
                                                            } else {
                                                                "Opcional: activa esta opción para iniciar el registro simulado."
                                                            }),
                                                    ),
                                            )
                                            .child(
                                                Switch::new("new-pensioner-enroll-switch")
                                                    .checked(enr.enabled)
                                                    .disabled(!can_enroll || !reader_available || is_busy)
                                                    .on_change(move |checked: &bool, _, cx| {
                                                        enroll_switch.update(cx, |s, cx| {
                                                            s.enabled = *checked;
                                                            s.op_seq = s.op_seq.wrapping_add(1);
                                                            if !*checked { s.consent_given = false; }
                                                            s.step = if *checked {
                                                                EnrollmentStep::WaitingFinger
                                                            } else {
                                                                EnrollmentStep::Idle
                                                            };
                                                            cx.notify();
                                                        });
                                                    }),
                                            ),
                                    )
                                    .when(enr.enabled, |this| {
                                        this.child(
                                            Checkbox::new("new-pensioner-consent")
                                                .checked(enr.consent_given)
                                                .label("Consentimiento informado otorgado")
                                                .on_click(move |checked: &bool, _, cx| {
                                                    enroll_consent.update(cx, |s, cx| {
                                                        s.consent_given = *checked;
                                                        if !*checked {
                                                            s.op_seq = s.op_seq.wrapping_add(1);
                                                            s.step = EnrollmentStep::WaitingFinger;
                                                        }
                                                        cx.notify();
                                                    });
                                                }),
                                        )
                                        .child(
                                            v_flex()
                                                .gap_2()
                                                .child(
                                                    h_flex()
                                                        .gap_2()
                                                        .flex_wrap()
                                                        .when(is_busy, |this| {
                                                            this.child(Spinner::new().small())
                                                        })
                                                        .child(if !enr.consent_given {
                                                            Tag::secondary().small().child("Pendiente de consentimiento")
                                                        } else if matches!(enr.step, EnrollmentStep::WaitingFinger) {
                                                            Tag::info().outline().small().child("Listo para simular")
                                                        } else {
                                                            enrollment_tag(&enr.step)
                                                        })
                                                        .when_some(
                                                            match &enr.step {
                                                                EnrollmentStep::Registered { biometric_id } => {
                                                                    Some(format!("Código asignado: {biometric_id}"))
                                                                }
                                                                EnrollmentStep::Error(msg) => Some(msg.clone()),
                                                                EnrollmentStep::WaitingFinger if enr.consent_given => Some("Pulsa «Simular captura» para continuar.".to_string()),
                                                                _ => None,
                                                            },
                                                            |this, text| {
                                                                this.child(
                                                                    div()
                                                                        .text_xs()
                                                                        .text_color(cx.theme().muted_foreground)
                                                                        .child(text),
                                                                )
                                                            },
                                                        ),
                                                )
                                                .child(
                                                    h_flex()
                                                        .gap_1p5()
                                                        .flex_wrap()
                                                        .child(
                                                            Button::new("new-enroll-sim-capture")
                                                                .outline()
                                                                .xsmall()
                                                                .disabled(!enr.consent_given || is_busy || matches!(enr.step, EnrollmentStep::Registered { .. }))
                                                                .icon(LucideIcon::FingerprintPattern)
                                                                .label(match &enr.step {
                                                                    EnrollmentStep::Registered { .. } => "Captura completada",
                                                                    EnrollmentStep::Error(_) => "Reintentar",
                                                                    _ => "Simular captura",
                                                                })
                                                                .on_click(move |_, _, cx| {
                                                                    start_simulated_enrollment_capture(
                                                                        enroll_capture.clone(),
                                                                        app_cap.clone(),
                                                                        0,
                                                                        cx,
                                                                    );
                                                                }),
                                                        )
                                                        .child(
                                                            Button::new("new-enroll-sim-err")
                                                                .ghost()
                                                                .xsmall()
                                                                .disabled(!enr.consent_given || is_busy || matches!(enr.step, EnrollmentStep::Registered { .. }))
                                                                .label("Simular error")
                                                                .on_click(move |_, _, cx| {
                                                                    enroll_fail.update(cx, |s, cx| {
                                                                        s.op_seq = s.op_seq.wrapping_add(1);
                                                                        s.step = EnrollmentStep::Error(
                                                                            "Lectura simulada incompleta; reintenta o guarda como Pendiente.".into(),
                                                                        );
                                                                        cx.notify();
                                                                    });
                                                                }),
                                                        ),
                                                ),
                                        )
                                        .child(
                                            div()
                                                .text_xs()
                                                .text_color(cx.theme().muted_foreground)
                                                .child("Simulación: solo verifica códigos, no compara ni registra huellas reales."),
                                        )
                                    })
                                    .when(!enr.enabled, |this| {
                                        this.child(
                                            div()
                                                .text_xs()
                                                .text_color(cx.theme().muted_foreground)
                                                .child("Se guardará con estado biométrico «Pendiente» y podrá usar identificación manual."),
                                        )
                                    }),
                            )
                        }),
                )
                .footer(
                    h_flex()
                        .gap_3()
                        .justify_end()
                        .child(
                            Button::new("pensioner-cancel")
                                .outline()
                                .label("Cancelar")
                                .on_click(move |_, window, cx| {
                                    if is_new {
                                        let _ = app_cancel_btn.update(cx, |this, cx| {
                                            this.resume_waiting_after_modal(cx);
                                        });
                                    }
                                    window.close_dialog(cx);
                                }),
                        )
                        .child(
                            Button::new("pensioner-save")
                                .primary()
                                .disabled(is_busy)
                                .label("Guardar")
                                .on_click(move |_, window, cx| {
                                    let first = first_field.read(cx).value().to_string();
                                    let last = last_field.read(cx).value().to_string();
                                    let phone = phone_field.read(cx).value().to_string();
                                    let enr_snap = enroll_submit.read(cx).clone();

                                    if is_new && enr_snap.enabled {
                                        if !enr_snap.consent_given {
                                            let msg = "Marca el consentimiento informado o desactiva «Registrar huella biométrica» para guardar como Pendiente.".to_string();
                                            form_error_submit.update(cx, |err, cx| {
                                                *err = Some(msg);
                                                cx.notify();
                                            });
                                            return;
                                        }
                                        if !matches!(enr_snap.step, EnrollmentStep::Registered { .. }) {
                                            let msg = "Completa la captura de huella o desactiva «Registrar huella biométrica» para guardar como Pendiente.".to_string();
                                            form_error_submit.update(cx, |err, cx| {
                                                *err = Some(msg);
                                                cx.notify();
                                            });
                                            return;
                                        }
                                    }

                                    let captured_bio = match &enr_snap.step {
                                        EnrollmentStep::Registered { biometric_id }
                                            if is_new && enr_snap.enabled =>
                                        {
                                            Some(biometric_id.clone())
                                        }
                                        _ => None,
                                    };

                                    let result = app_save.update(cx, |this, cx| {
                                        if !this.has_pensioner_permission(permission) {
                                            return Err(
                                                "No tienes permiso para guardar.".to_string()
                                            );
                                        }
                                        if captured_bio.is_some() && !this.has_pensioner_permission("biometrics.enroll") {
                                            return Err("No tienes permiso para registrar huellas.".to_string());
                                        }
                                        let DataStatus::Ready(data) = &mut this.state.data else {
                                            return Err(
                                                "Los datos no están disponibles.".to_string()
                                            );
                                        };
                                        // Commit all changes together; a duplicate leaves no partial person behind.
                                        let mut draft = data.clone();
                                        let saved_id = draft.save_pensioner(id, &first, &last, &phone)?;
                                        if let Some(bio_code) = &captured_bio {
                                            this.biometric_gateway.validate_enrollment(saved_id, bio_code, &data.biometric_candidates())
                                                .map_err(|err| err.to_string())?;
                                            let registered =
                                                draft.register_fingerprint_with_id(saved_id, bio_code)?;
                                            *data = draft;
                                            this.push_biometric_event(BiometricEvent::Enrolled {
                                                pensioner_id: saved_id,
                                                biometric_id: registered,
                                            });
                                        } else { *data = draft; }
                                        this.selected_pensioner = Some(saved_id);
                                        if is_new {
                                            this.resume_waiting_after_modal(cx);
                                        }
                                        cx.notify();
                                        Ok(captured_bio.is_some())
                                    });
                                    match result {
                                        Ok(Ok(with_bio)) => {
                                            form_error_submit.update(cx, |err, _| *err = None);
                                            window.close_dialog(cx);
                                            window.push_notification(
                                                Notification::success(if with_bio {
                                                    "Pensionista guardado con huella simulada."
                                                } else if is_new {
                                                    "Pensionista guardado en estado Pendiente."
                                                } else {
                                                    "Pensionista actualizado."
                                                }),
                                                cx,
                                            );
                                        }
                                        Ok(Err(error)) => {
                                            form_error_submit.update(cx, |err, cx| {
                                                *err = Some(error.clone());
                                                cx.notify();
                                            });
                                            window.push_notification(Notification::error(error), cx);
                                        }
                                        Err(_) => window.push_notification(
                                            Notification::error(
                                                "La pantalla ya no está disponible.",
                                            ),
                                            cx,
                                        ),
                                    }
                                }),
                        ),
                )
        });
    }

    fn open_biometric_enroll_modal(
        &mut self,
        id: u32,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self.has_pensioner_permission("biometrics.enroll") {
            window.push_notification(
                Notification::error("No tienes permiso para registrar o gestionar huellas."),
                cx,
            );
            return;
        }
        let Some(person) = (match &self.state.data {
            DataStatus::Ready(data) => data.pensioners.iter().find(|p| p.id == id).cloned(),
            _ => None,
        }) else {
            window.push_notification(Notification::error("Pensionista no encontrado."), cx);
            return;
        };
        if person.fingerprint_registered || person.biometric_id.is_some() {
            window.push_notification(Notification::warning("Este pensionista ya tiene una huella registrada"), cx);
            return;
        }
        if self.biometric_modal_open || window.has_active_dialog(cx) {
            window.push_notification(Notification::warning("Ya hay un enrolamiento o diálogo en curso."), cx);
            return;
        }
        if !person.active {
            window.push_notification(
                Notification::error("Activa al pensionista antes de gestionar su huella."),
                cx,
            );
            return;
        }

        let reader_connected = self.biometric_gateway.state().is_connected();
        if reader_connected {
            self.biometric_modal_open = true;
            self.biometric_gateway.set_modal_locked(true);
        }

        let already_registered = person.fingerprint_registered;
        let existing_bio_id = person.biometric_id.clone();
        let name = person.name();

        let modal_state = cx.new(|_| NewEnrollFormState {
            enabled: reader_connected,
            consent_given: true,
            step: if reader_connected {
                EnrollmentStep::WaitingFinger
            } else {
                EnrollmentStep::Error(
                    "Lector biométrico desconectado. Conecta el lector para iniciar la captura."
                        .into(),
                )
            },
            op_seq: 0,
        });

        let app = cx.entity().downgrade();
        if reader_connected {
            start_simulated_enrollment_capture(modal_state.clone(), app.clone(), 280, cx);
        }
        let app_cancel_dlg = app.clone();
        let app_cancel_btn = app.clone();
        let app_cap = app.clone();
        let app_save = app.clone();

        window.open_dialog(cx, move |dialog, _, cx| {
            let st = modal_state.read(cx).clone();
            let is_busy = matches!(
                st.step,
                EnrollmentStep::Capturing | EnrollmentStep::Verifying
            );
            let modal_cap = modal_state.clone();
            let modal_err = modal_state.clone();
            let modal_save = modal_state.clone();
            let app_cap_cb = app_cap.clone();
            let app_save_cb = app_save.clone();
            let app_dlg_close = app_cancel_dlg.clone();
            let app_btn_close = app_cancel_btn.clone();

            let captured_id = match &st.step {
                EnrollmentStep::Registered { biometric_id } => Some(biometric_id.clone()),
                _ => None,
            };

            dialog
                .width(px(480.))
                .title(if already_registered {
                    format!("Gestionar huella · {name}")
                } else {
                    format!("Registrar huella · {name}")
                })
                .on_cancel(move |_, _, cx| {
                    let _ = app_dlg_close.update(cx, |this, cx| {
                        this.resume_waiting_after_modal(cx);
                    });
                    true
                })
                .child(
                    v_flex()
                        .gap_3()
                        .child(
                            h_flex()
                                .justify_between()
                                .p_2p5()
                                .rounded(cx.theme().radius)
                                .border_1()
                                .border_color(cx.theme().border)
                                .bg(cx.theme().muted)
                                .child(
                                    v_flex()
                                        .gap_0p5()
                                        .child(
                                            div()
                                                .text_sm()
                                                .font_weight(FontWeight::SEMIBOLD)
                                                .child(format!("Pensionista · {name}")),
                                        )
                                        .child(
                                            div()
                                                .text_xs()
                                                .text_color(cx.theme().muted_foreground)
                                                .child(if let Some(bio) = &existing_bio_id {
                                                    format!("Plantilla actual: {bio}")
                                                } else {
                                                    "Estado actual: Pendiente de enrolamiento"
                                                        .to_string()
                                                }),
                                        ),
                                )
                                .child(enrollment_tag(&st.step)),
                        )
                        .when_some(
                            match &st.step {
                                EnrollmentStep::Error(msg) => Some(msg.clone()),
                                _ => None,
                            },
                            |this, msg| this.child(Alert::error("bio-modal-err", msg)),
                        )
                        .child(
                            h_flex()
                                .gap_3()
                                .p_3()
                                .rounded(cx.theme().radius)
                                .border_1()
                                .border_color(cx.theme().border)
                                .child(if is_busy {
                                    Spinner::new().small().into_any_element()
                                } else {
                                    Icon::new(LucideIcon::FingerprintPattern)
                                        .size_6()
                                        .text_color(cx.theme().primary)
                                        .into_any_element()
                                })
                                .child(
                                    v_flex()
                                        .gap_0p5()
                                        .child(
                                            div()
                                                .text_sm()
                                                .font_weight(FontWeight::MEDIUM)
                                                .child(st.step.label()),
                                        )
                                        .child(
                                            div()
                                                .text_xs()
                                                .text_color(cx.theme().muted_foreground)
                                                .child(if let Some(bio) = &captured_id {
                                                    format!("Identificador único generado: {bio} (simulación sin lector real).")
                                                } else {
                                                    "Simulación: solo verifica códigos, no compara huellas reales.".to_string()
                                                }),
                                        ),
                                ),
                        )
                        .child(
                            h_flex()
                                .gap_2()
                                .child(
                                    Button::new("modal-enroll-capture")
                                        .outline()
                                        .small()
                                        .disabled(!reader_connected || is_busy || captured_id.is_some())
                                        .icon(LucideIcon::FingerprintPattern)
                                        .label(if captured_id.is_some() { "Captura simulada completada" } else { "Simular captura" })
                                        .on_click(move |_, _, cx| {
                                            start_simulated_enrollment_capture(
                                                modal_cap.clone(),
                                                app_cap_cb.clone(),
                                                0,
                                                cx,
                                            );
                                        }),
                                )
                                .child(
                                    Button::new("modal-enroll-error")
                                        .ghost()
                                        .small()
                                        .disabled(!reader_connected || is_busy)
                                        .label("Simular error")
                                        .on_click(move |_, _, cx| {
                                            modal_err.update(cx, |s, cx| {
                                                s.op_seq = s.op_seq.wrapping_add(1);
                                                s.step = EnrollmentStep::Error(
                                                    "Lectura simulada incompleta. Reintenta la captura."
                                                        .into(),
                                                );
                                                cx.notify();
                                            });
                                        }),
                                ),
                        ),
                )
                .footer(
                    h_flex()
                        .gap_3()
                        .justify_end()
                        .child(
                            Button::new("bio-modal-cancel")
                                .outline()
                                .label("Cancelar")
                                .on_click(move |_, window, cx| {
                                    let _ = app_btn_close.update(cx, |this, cx| {
                                        this.resume_waiting_after_modal(cx);
                                    });
                                    window.close_dialog(cx);
                                }),
                        )
                        .child(
                            Button::new("bio-modal-save")
                                .primary()
                                .disabled(captured_id.is_none() || is_busy)
                                .label("Confirmar registro")
                                .on_click(move |_, window, cx| {
                                    let bio_code = match &modal_save.read(cx).step {
                                        EnrollmentStep::Registered { biometric_id } => {
                                            biometric_id.clone()
                                        }
                                        _ => return,
                                    };
                                    let res = app_save_cb.update(cx, |this, cx| {
                                        if !this.has_pensioner_permission("biometrics.enroll") {
                                            return Err("No tienes permiso para registrar huellas.".to_string());
                                        }
                                        let DataStatus::Ready(data) = &mut this.state.data else {
                                            return Err(
                                                "Los datos no están disponibles.".to_string(),
                                            );
                                        };
                                        this.biometric_gateway.validate_enrollment(id, &bio_code, &data.biometric_candidates())
                                            .map_err(|err| err.to_string())?;
                                        let registered =
                                            data.register_fingerprint_with_id(id, &bio_code)?;
                                        this.push_biometric_event(BiometricEvent::Enrolled {
                                            pensioner_id: id,
                                            biometric_id: registered,
                                        });
                                        this.resume_waiting_after_modal(cx);
                                        Ok(())
                                    });
                                    match res {
                                        Ok(Ok(())) => {
                                            window.close_dialog(cx);
                                            window.push_notification(
                                                Notification::success(
                                                    "Huella simulada asociada correctamente.",
                                                ),
                                                cx,
                                            );
                                        }
                                        Ok(Err(err)) => {
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

    fn confirm_change(
        &mut self,
        id: u32,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self.has_pensioner_permission("pensioners.update") {
            window.push_notification(
                Notification::error("No tienes permiso para realizar esta acción."),
                cx,
            );
            return;
        }
        let Some(person) = (match &self.state.data {
            DataStatus::Ready(data) => data.pensioners.iter().find(|p| p.id == id),
            _ => None,
        }) else {
            window.push_notification(Notification::error("Pensionista no encontrado."), cx);
            return;
        };
        let was_active = person.active;
        let name = person.name();
        let app = cx.entity().downgrade();
        let description = format!(
            "¿Deseas {} a {name}?",
            if was_active { "desactivar" } else { "activar" }
        );
        window.open_dialog(cx, move |dialog, _, _| {
            let app = app.clone();
            dialog
                .title("Confirmar cambio de estado")
                .child(div().text_sm().child(description.clone()))
                .footer(
                    h_flex()
                        .gap_3()
                        .justify_end()
                        .child(
                            Button::new("confirm-cancel")
                                .outline()
                                .label("Cancelar")
                                .on_click(|_, window, cx| window.close_dialog(cx)),
                        )
                        .child(
                            Button::new("confirm-apply")
                                .primary()
                                .label(if was_active { "Desactivar" } else { "Activar" })
                                .on_click(move |_, window, cx| {
                                    let result = app.update(cx, |this, cx| {
                                        if !this.has_pensioner_permission("pensioners.update") {
                                            return Err(
                                                "No tienes permiso para realizar esta acción."
                                                    .to_string(),
                                            );
                                        }
                                        let DataStatus::Ready(data) = &mut this.state.data else {
                                            return Err(
                                                "Los datos no están disponibles.".to_string(),
                                            );
                                        };
                                        data.set_active(id, was_active)
                                            .map_err(|e| e.to_string())?;
                                        cx.notify();
                                        Ok(())
                                    });
                                    match result {
                                        Ok(Ok(())) => {
                                            window.close_dialog(cx);
                                            window.push_notification(
                                                Notification::success("Estado actualizado."),
                                                cx,
                                            );
                                        }
                                        Ok(Err(error)) => {
                                            window.push_notification(Notification::error(error), cx);
                                        }
                                        Err(_) => window.close_dialog(cx),
                                    }
                                }),
                        ),
                )
        });
    }
}

fn summary_box_toned(
    title: &'static str,
    value: String,
    subtitle: impl Into<gpui_kit::SharedString>,
    icon: LucideIcon,
    tone: SemanticTone,
    app: &App,
) -> impl IntoElement {
    metric_card(title, value, Some(subtitle.into()), icon, tone, app)
}

fn details(
    person: &Pensioner,
    data: &MockData,
    state: &AppState,
    cx: &Context<DesktopApp>,
    app: &App,
) -> impl IntoElement {
    let theme = app.theme();
    let id = person.id;
    let active = person.active;
    let can_consumptions = allowed(state, "consumptions.view");
    let can_payments = allowed(state, "payments.view");

    let consumptions: Vec<_> = data
        .consumptions
        .iter()
        .filter(|c| c.pensioner_id == id)
        .rev()
        .collect();
    let payments: Vec<_> = data
        .payments
        .iter()
        .filter(|p| p.pensioner_id == id)
        .rev()
        .collect();

    let total_consumed: i64 = consumptions.iter().map(|c| c.cents).sum();
    let total_cash_paid: i64 = payments
        .iter()
        .filter(|p| matches!(p.kind, PaymentKind::CashPayment))
        .map(|p| p.cents)
        .sum();
    let (ticket_count, ticket_total) = data.tickets_for_pensioner(id);
    let net_balance = data.net_balance(id);
    let debt = net_balance.max(0);
    let credit = (-net_balance).max(0);
    let monthly = data.monthly_summary(id);
    let ticket_rows: Vec<_> = payments
        .iter()
        .filter_map(|p| match p.kind {
            PaymentKind::TicketCredit { count, unit_cents } => {
                let mov = data
                    .cash_register
                    .movements
                    .iter()
                    .find(|m| m.id == p.movement_id);
                Some((
                    p.date,
                    p.time.clone(),
                    p.movement_id.clone(),
                    count,
                    unit_cents,
                    p.cents,
                    mov.map(|m| m.operator_name.clone())
                        .unwrap_or_else(|| "Sistema".into()),
                    mov.and_then(|m| m.consumption_cents),
                    mov.and_then(|m| m.resulting_balance_cents),
                ))
            }
            _ => None,
        })
        .collect();

    v_flex()
        .gap_4()
        .child(
            h_flex().child(
                Button::new("pensioner-back")
                    .ghost()
                    .small()
                    .icon(LucideIcon::ArrowLeft)
                    .label("Volver al listado")
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.selected_pensioner = None;
                        cx.notify();
                    })),
            ),
        )
        .child(
            h_flex()
                .justify_between()
                .gap_4()
                .flex_wrap()
                .child(
                    v_flex()
                        .gap_1p5()
                        .child(
                            h_flex()
                                .gap_2()
                                .flex_wrap()
                                .child(
                                    div()
                                        .text_2xl()
                                        .font_weight(FontWeight::BOLD)
                                        .child(person.name()),
                                )
                                .child(status_tag(active))
                                .child(if credit > 0 {
                                    Tag::info().small().child(format!("A favor {}", amount(credit)))
                                } else if debt > 0 {
                                    Tag::warning().small().child(format!("Debe {}", amount(debt)))
                                } else {
                                    Tag::success().outline().small().child("Al día")
                                })
                                .child(biometric_tag(person.biometric_status()))
                                .when_some(person.biometric_id.clone(), |this, bio| {
                                    this.child(Tag::info().outline().small().child(bio))
                                }),
                        )
                        .child(
                            h_flex()
                                .gap_2()
                                .text_sm()
                                .text_color(theme.muted_foreground)
                                .child(Icon::new(LucideIcon::Phone).xsmall())
                                .child(format!(
                                    "Celular: {}",
                                    person.phone.as_deref().unwrap_or("No registrado")
                                )),
                        ),
                ),
        )
        .when(can_consumptions || can_payments, |this| {
            this.child(
                h_flex()
                    .gap_3()
                    .flex_wrap()
                    .child(summary_box_toned(
                        "Total consumido",
                        amount(total_consumed),
                        "Acumulado histórico",
                        LucideIcon::Utensils,
                        SemanticTone::Neutral,
                        app,
                    ))
                    .child(summary_box_toned(
                        "Efectivo pagado",
                        amount(total_cash_paid),
                        "Pagos en caja",
                        LucideIcon::Wallet,
                        SemanticTone::Green,
                        app,
                    ))
                    .child(summary_box_toned(
                        "Tickets entregados",
                        ticket_count.to_string(),
                        format!("Abono total: {}", amount(ticket_total)),
                        LucideIcon::Ticket,
                        SemanticTone::Blue,
                        app,
                    ))
                    .child(summary_box_toned(
                        "Saldo a favor",
                        amount(credit),
                        if credit > 0 {
                            "Estado activo: a favor"
                        } else {
                            "Sin excedente a favor"
                        },
                        LucideIcon::Wallet,
                        if credit > 0 {
                            SemanticTone::SoftGreen
                        } else {
                            SemanticTone::Neutral
                        },
                        app,
                    ))
                    .child(summary_box_toned(
                        "Deuda pendiente",
                        amount(debt),
                        if debt > 0 {
                            "Estado activo: debe"
                        } else {
                            "Sin deuda pendiente"
                        },
                        LucideIcon::Wallet,
                        if debt > 0 {
                            SemanticTone::SoftRed
                        } else {
                            SemanticTone::Neutral
                        },
                        app,
                    )),
            )
        })
        .when(can_payments, |this| {
            this.child(
                GroupBox::new()
                    .outline()
                    .title(
                        h_flex()
                            .gap_2()
                            .child(Icon::new(LucideIcon::Ticket).small().text_color(theme.primary))
                            .child(format!("Historial de tickets entregados ({ticket_count})")),
                    )
                    .child(if ticket_rows.is_empty() {
                        div()
                            .py_3()
                            .text_sm()
                            .text_color(theme.muted_foreground)
                            .child("Sin tickets físicos entregados.")
                            .into_any_element()
                    } else {
                        v_flex()
                            .gap_2()
                            .children(ticket_rows.into_iter().map(
                                |(
                                    date,
                                    time,
                                    mov_id,
                                    cnt,
                                    unit_cents,
                                    total_cents,
                                    operator,
                                    cons_opt,
                                    res_bal_opt,
                                )| {
                                    let extra = match (cons_opt, res_bal_opt) {
                                        (Some(c), Some(b)) => format!(
                                            " · Consumo: {} · Saldo: {}",
                                            amount(c),
                                            if b < 0 {
                                                format!("A favor {}", amount(-b))
                                            } else if b > 0 {
                                                format!("Deuda {}", amount(b))
                                            } else {
                                                "Al día".into()
                                            }
                                        ),
                                        (None, Some(b)) => format!(
                                            " · Saldo: {}",
                                            if b < 0 {
                                                format!("A favor {}", amount(-b))
                                            } else if b > 0 {
                                                format!("Deuda {}", amount(b))
                                            } else {
                                                "Al día".into()
                                            }
                                        ),
                                        _ => String::new(),
                                    };
                                    h_flex()
                                        .justify_between()
                                        .gap_3()
                                        .flex_wrap()
                                        .py_1p5()
                                        .border_b_1()
                                        .border_color(theme.border)
                                        .child(div().text_sm().child(format!(
                                            "{} {} · {mov_id} · {cnt} ticket(s) × {} · Por: {operator}{extra}",
                                            date.format("%d/%m/%Y"),
                                            time,
                                            amount(unit_cents)
                                        )))
                                        .child(
                                            div()
                                                .text_sm()
                                                .font_weight(FontWeight::SEMIBOLD)
                                                .child(amount(total_cents)),
                                        )
                                },
                            ))
                            .into_any_element()
                    }),
            )
        })
        .when(can_consumptions || can_payments, |this| {
            this.child(
                GroupBox::new()
                    .outline()
                    .title("Resumen mensual")
                    .child(if monthly.is_empty() {
                        div()
                            .py_3()
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
                                    .gap_3()
                                    .flex_wrap()
                                    .py_1p5()
                                    .border_b_1()
                                    .border_color(theme.border)
                                    .child(div().text_xs().child(format!(
                                        "{} · Consumos: {} · Efectivo: {} · Tickets: {}",
                                        m.month_label,
                                        amount(m.consumed_cents),
                                        amount(m.cash_paid_cents),
                                        amount(m.tickets_cents)
                                    )))
                                    .child(div().text_xs().font_weight(FontWeight::SEMIBOLD).child(
                                        if m.net_balance_cents < 0 {
                                            format!("A favor {}", amount(-m.net_balance_cents))
                                        } else {
                                            format!("Neto: {}", amount(m.net_balance_cents))
                                        },
                                    ))
                            }))
                            .into_any_element()
                    }),
            )
        })
        .when(can_consumptions, |this| {
            this.child(
                GroupBox::new()
                    .outline()
                    .title(
                        h_flex()
                            .gap_2()
                            .child(Icon::new(LucideIcon::ReceiptText).small().text_color(theme.primary))
                            .child("Historial de consumos"),
                    )
                    .child(if consumptions.is_empty() {
                        div()
                            .py_3()
                            .text_sm()
                            .text_color(theme.muted_foreground)
                            .child("Sin consumos registrados.")
                            .into_any_element()
                    } else {
                        v_flex()
                            .gap_2()
                            .children(consumptions.into_iter().map(|item| {
                                let detail_part = item
                                    .detail
                                    .as_deref()
                                    .map(|d| format!(" · {d}"))
                                    .unwrap_or_default();
                                let pickup_part = item
                                    .picked_up_by
                                    .as_deref()
                                    .map(|p| format!(" · Recoge: {p}"))
                                    .unwrap_or_default();
                                h_flex()
                                    .justify_between()
                                    .gap_3()
                                    .py_1p5()
                                    .border_b_1()
                                    .border_color(theme.border)
                                    .child(
                                        div().text_sm().child(format!(
                                            "{} {} · {} ({}) · {} ración(es){detail_part}{pickup_part}",
                                            item.date.format("%d/%m/%Y"),
                                            item.time,
                                            item.meal.label(),
                                            item.menu_kind.label(),
                                            item.portions
                                        )),
                                    )
                                    .child(
                                        div()
                                            .text_sm()
                                            .font_weight(FontWeight::SEMIBOLD)
                                            .child(amount(item.cents)),
                                    )
                            }))
                            .into_any_element()
                    }),
            )
        })
        .when(can_payments, |this| {
            this.child(
                GroupBox::new()
                    .outline()
                    .title(
                        h_flex()
                            .gap_2()
                            .child(Icon::new(LucideIcon::CreditCard).small().text_color(theme.primary))
                            .child("Historial de pagos y abonos"),
                    )
                    .child(if payments.is_empty() {
                        div()
                            .py_3()
                            .text_sm()
                            .text_color(theme.muted_foreground)
                            .child("Sin pagos registrados.")
                            .into_any_element()
                    } else {
                        v_flex()
                            .gap_2()
                            .children(payments.into_iter().map(|item| {
                                let kind_detail = match &item.kind {
                                    PaymentKind::TicketCredit { count, unit_cents } => {
                                        format!(" ({count} und. × {})", amount(*unit_cents))
                                    }
                                    PaymentKind::CashPayment => String::new(),
                                };
                                h_flex()
                                    .justify_between()
                                    .gap_3()
                                    .py_1p5()
                                    .border_b_1()
                                    .border_color(theme.border)
                                    .child(div().text_sm().child(format!(
                                        "{} {} · {} · {}{kind_detail}",
                                        item.date.format("%d/%m/%Y"),
                                        item.time,
                                        item.movement_id,
                                        item.kind.label()
                                    )))
                                    .child(
                                        div()
                                            .text_sm()
                                            .font_weight(FontWeight::SEMIBOLD)
                                            .child(amount(item.cents)),
                                    )
                            }))
                            .into_any_element()
                    }),
            )
        })
}

#[derive(Clone, PartialEq, Eq)]
pub struct PensionerTableRow {
    pub id: u32,
    pub name: String,
    pub phone: String,
    pub active: bool,
    pub biometric_status: &'static str,
    pub has_fp: bool,
    pub credit_cents: i64,
    pub debt_cents: i64,
    pub credit_revealed: bool,
    pub debt_revealed: bool,
}

pub struct PensionersTableDelegate {
    app: WeakEntity<DesktopApp>,
    columns: Vec<Column>,
    rows: Vec<PensionerTableRow>,
    can_update: bool,
    can_enroll: bool,
    enrollment_busy: bool,
    sort: Option<(usize, ColumnSort)>,
}

impl PensionersTableDelegate {
    pub fn new(app: WeakEntity<DesktopApp>) -> Self {
        Self {
            app,
            columns: vec![
                Column::new("name", "Nombre completo").width(210.).sortable(),
                Column::new("phone", "Celular").width(120.),
                Column::new("status", "Estado").width(95.).sortable(),
                Column::new("biometric", "Huella").width(115.).sortable(),
                Column::new("credit", "A favor").width(125.).sortable(),
                Column::new("debt", "En contra").width(125.).sortable(),
                Column::new("actions", "Acciones").width(220.).selectable(false),
            ],
            rows: Vec::new(),
            can_update: false,
            can_enroll: false,
            enrollment_busy: false,
            sort: None,
        }
    }

    pub fn sync(
        &mut self,
        state: &AppState,
        search_query: &str,
        filter: Filter,
        enrollment_busy: bool,
        revealed_balances: &std::collections::HashSet<(u32, bool)>,
    ) -> bool {
        let can_update = allowed(state, "pensioners.update");
        let can_enroll = allowed(state, "biometrics.enroll");
        let query = search_query.trim().to_lowercase();

        let mut new_rows = Vec::new();
        if let DataStatus::Ready(data) = &state.data {
            for person in &data.pensioners {
                if (filter == Filter::Active && !person.active)
                    || (filter == Filter::Inactive && person.active)
                {
                    continue;
                }
                let haystack = format!(
                    "{} {} {}",
                    person.id,
                    person.name(),
                    person.phone.as_deref().unwrap_or("")
                )
                .to_lowercase();
                if !haystack.contains(&query) {
                    continue;
                }
                let id = person.id;
                let net = data.net_balance(id);
                let debt_cents = net.max(0);
                let credit_cents = net.saturating_neg().max(0);
                new_rows.push(PensionerTableRow {
                    id,
                    name: person.name(),
                    phone: person.phone.clone().unwrap_or_else(|| "—".into()),
                    active: person.active,
                    biometric_status: person.biometric_status(),
                    has_fp: person.fingerprint_registered || person.biometric_id.is_some(),
                    credit_cents,
                    debt_cents,
                    credit_revealed: revealed_balances.contains(&(id, true)),
                    debt_revealed: revealed_balances.contains(&(id, false)),
                });
            }
        }

        if let Some((col_ix, sort)) = self.sort {
            Self::sort_rows(&mut new_rows, col_ix, sort);
        }

        if new_rows == self.rows
            && can_update == self.can_update
            && can_enroll == self.can_enroll
            && enrollment_busy == self.enrollment_busy
        {
            return false;
        }

        self.rows = new_rows;
        self.can_update = can_update;
        self.can_enroll = can_enroll;
        self.enrollment_busy = enrollment_busy;
        true
    }

    fn sort_rows(rows: &mut [PensionerTableRow], col_ix: usize, sort: ColumnSort) {
        if sort == ColumnSort::Default {
            rows.sort_by_key(|r| r.id);
            return;
        }
        rows.sort_by(|a, b| {
            let ord = match col_ix {
                0 => a.name.to_lowercase().cmp(&b.name.to_lowercase()),
                2 => b.active.cmp(&a.active),
                3 => a.biometric_status.cmp(b.biometric_status),
                4 => a.credit_cents.cmp(&b.credit_cents),
                5 => a.debt_cents.cmp(&b.debt_cents),
                _ => a.id.cmp(&b.id),
            };
            match sort {
                ColumnSort::Descending => ord.reverse(),
                _ => ord,
            }
        });
    }
}

impl TableDelegate for PensionersTableDelegate {
    fn columns_count(&self, _: &App) -> usize {
        self.columns.len()
    }

    fn rows_count(&self, _: &App) -> usize {
        self.rows.len()
    }

    fn column(&self, col_ix: usize, _: &App) -> Column {
        self.columns[col_ix].clone()
    }

    fn perform_sort(
        &mut self,
        col_ix: usize,
        sort: ColumnSort,
        _: &mut Window,
        cx: &mut Context<TableState<Self>>,
    ) {
        self.sort = if sort == ColumnSort::Default {
            None
        } else {
            Some((col_ix, sort))
        };
        for (ix, col) in self.columns.iter_mut().enumerate() {
            if ix == col_ix {
                col.sort = Some(sort);
            } else if col.sort.is_some() {
                col.sort = Some(ColumnSort::Default);
            }
        }
        Self::sort_rows(&mut self.rows, col_ix, sort);
        cx.notify();
    }

    fn render_td(
        &mut self,
        row_ix: usize,
        col_ix: usize,
        _: &mut Window,
        cx: &mut Context<TableState<Self>>,
    ) -> impl IntoElement {
        let Some(row) = self.rows.get(row_ix) else {
            return div().into_any_element();
        };
        let id = row.id;
        match col_ix {
            0 => div()
                .h_full()
                .flex()
                .items_center()
                .text_sm()
                .truncate()
                .child(row.name.clone())
                .into_any_element(),
            1 => div()
                .h_full()
                .flex()
                .items_center()
                .text_sm()
                .truncate()
                .child(row.phone.clone())
                .into_any_element(),
            2 => div()
                .h_full()
                .flex()
                .items_center()
                .child(status_tag(row.active))
                .into_any_element(),
            3 => div()
                .h_full()
                .flex()
                .items_center()
                .child(biometric_tag(row.biometric_status))
                .into_any_element(),
            4 => div()
                .h_full()
                .flex()
                .items_center()
                .child(balance_cell(
                    id,
                    true,
                    row.credit_cents,
                    row.credit_revealed,
                    &self.app,
                    cx,
                ))
                .into_any_element(),
            5 => div()
                .h_full()
                .flex()
                .items_center()
                .child(balance_cell(
                    id,
                    false,
                    row.debt_cents,
                    row.debt_revealed,
                    &self.app,
                    cx,
                ))
                .into_any_element(),
            6 => {
                let app_detail = self.app.clone();
                let app_edit = self.app.clone();
                let app_toggle = self.app.clone();
                let app_bio = self.app.clone();
                let is_active = row.active;
                let has_fp = row.has_fp;
                let enrollment_busy = self.enrollment_busy;
                h_flex()
                    .h_full()
                    .items_center()
                    .gap_1p5()
                    .child(
                        Button::new(format!("person-detail-{id}"))
                            .outline()
                            .small()
                            .icon(LucideIcon::Eye)
                            .label("Detalle")
                            .on_click(move |_, _, cx| {
                                let _ = app_detail.update(cx, |this, cx| {
                                    this.selected_pensioner = Some(id);
                                    cx.notify();
                                });
                            }),
                    )
                    .when(self.can_update, |this| {
                        this.child(
                            Button::new(format!("person-edit-{id}"))
                                .ghost()
                                .small()
                                .icon(LucideIcon::Pencil)
                                .tooltip("Editar")
                                .on_click(move |_, window, cx| {
                                    let _ = app_edit.update(cx, |this, cx| {
                                        this.pensioner_form(Some(id), window, cx);
                                    });
                                }),
                        )
                        .child(
                            Button::new(format!("person-toggle-{id}"))
                                .ghost()
                                .small()
                                .icon(if is_active {
                                    LucideIcon::UserX
                                } else {
                                    LucideIcon::UserCheck
                                })
                                .tooltip(if is_active {
                                    "Desactivar"
                                } else {
                                    "Activar"
                                })
                                .on_click(move |_, window, cx| {
                                    let _ = app_toggle.update(cx, |this, cx| {
                                        this.confirm_change(id, window, cx);
                                    });
                                }),
                        )
                    })
                    .when(self.can_enroll, |this| {
                        this.child(
                            Button::new(format!("person-bio-{id}"))
                                .ghost()
                                .small()
                                .disabled(!is_active || has_fp || enrollment_busy)
                                .icon(LucideIcon::FingerprintPattern)
                                .accessibility_label("Registrar huella")
                                .tooltip(if has_fp {
                                    "Este pensionista ya tiene una huella registrada"
                                } else if enrollment_busy {
                                    "Hay un enrolamiento en curso"
                                } else if !is_active {
                                    "Activa al pensionista para gestionar su huella"
                                } else {
                                    "Registrar huella"
                                })
                                .on_click(move |_, window, cx| {
                                    let _ = app_bio.update(cx, |this, cx| {
                                        this.open_biometric_enroll_modal(id, window, cx);
                                    });
                                }),
                        )
                    })
                    .into_any_element()
            }
            _ => div().into_any_element(),
        }
    }
}

pub fn render(
    state: &AppState,
    cx: &Context<DesktopApp>,
    app: &App,
    search: &Entity<InputState>,
    filter: Filter,
    selected: Option<u32>,
    table_state: &Entity<TableState<PensionersTableDelegate>>,
) -> impl IntoElement {
    let theme = app.theme();
    let data = match &state.data {
        DataStatus::Ready(data) => data,
        DataStatus::Loading => return div().child("Cargando pensionistas…").into_any_element(),
        DataStatus::Error(error) => {
            return v_flex()
                .gap_3()
                .child(format!("No se pudieron cargar los datos: {error}"))
                .child(
                    Button::new("pensioners-retry")
                        .outline()
                        .label("Reintentar")
                        .on_click(cx.listener(|this, _, _, cx| this.load_demo_data(cx))),
                )
                .into_any_element()
        }
    };
    if let Some(person) = selected.and_then(|id| data.pensioners.iter().find(|p| p.id == id)) {
        return details(person, data, state, cx, app).into_any_element();
    }
    let rows_count = table_state.read(app).delegate().rows_count(app);
    let mut filters = h_flex().gap_2().flex_wrap();
    for (choice, label) in [
        (Filter::All, "Todos"),
        (Filter::Active, "Activos"),
        (Filter::Inactive, "Inactivos"),
    ] {
        filters = filters.child(
            Button::new(label)
                .small()
                .label(label)
                .when(choice == filter, |b| b.primary())
                .when(choice != filter, |b| b.outline())
                .on_click(cx.listener(move |this, _, _, cx| {
                    this.pensioner_filter = choice;
                    cx.notify();
                })),
        );
    }
    v_flex()
        .gap_4()
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
                                .child("Pensionistas"),
                        )
                        .child(
                            div()
                                .text_sm()
                                .text_color(theme.muted_foreground)
                                .child("Administración de registros · datos compartidos"),
                        ),
                )
                .when(allowed(state, "pensioners.create"), |this| {
                    this.child(
                        Button::new("new-pensioner")
                            .primary()
                            .icon(LucideIcon::Plus)
                            .label("Nuevo pensionista")
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.pensioner_form(None, window, cx)
                            })),
                    )
                }),
        )
        .child(
            h_flex()
                .justify_between()
                .gap_3()
                .flex_wrap()
                .child(
                    div().w(px(360.)).max_w_full().child(
                        Input::new(search)
                            .cleanable(true)
                            .prefix(Icon::new(LucideIcon::Search).xsmall()),
                    ),
                )
                .child(filters),
        )
        .child(if rows_count == 0 {
            Empty::new()
                .py_8()
                .header(
                    EmptyHeader::new()
                        .media(
                            EmptyMedia::new()
                                .with_variant(EmptyMediaVariant::Icon)
                                .child(Icon::new(LucideIcon::Users).size_6()),
                        )
                        .title(EmptyTitle::new().child("Sin resultados"))
                        .description(
                            EmptyDescription::new()
                                .child("No se encontraron pensionistas con estos filtros."),
                        ),
                )
                .into_any_element()
        } else {
            let table_height = ((rows_count as f32) * 44.0 + 48.0).clamp(220.0, 520.0);
            div()
                .w_full()
                .h(px(table_height))
                .child(
                    DataTable::new(table_state)
                        .stripe(true)
                        .bordered(true)
                        .with_size(Size::Medium)
                        .accessibility_label("Tabla de pensionistas"),
                )
                .into_any_element()
        })
        .into_any_element()
}
