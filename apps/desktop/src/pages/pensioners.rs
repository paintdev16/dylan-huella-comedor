use std::time::Duration;

use crate::{
    biometric_service::{
        BiometricEvent, BiometricGateway as _, EnrollmentStep, generate_unique_biometric_id,
    },
    layout::DesktopApp,
    mock_data::{MockData, Pensioner},
    state::{AppState, DataStatus, Page},
};
use gpui_kit::{
    App, AppContext as _, Context, Entity, FontWeight, IntoElement, ParentElement, SharedString,
    Styled, Window,
    assets::IconName as LucideIcon,
    component::{
        ActiveTheme as _, Disableable as _, Icon, Sizable as _, WindowExt as _,
        alert::Alert,
        button::{Button, ButtonVariants as _},
        checkbox::Checkbox,
        empty::{Empty, EmptyDescription, EmptyHeader, EmptyMedia, EmptyMediaVariant, EmptyTitle},
        group_box::{GroupBox, GroupBoxVariants as _},
        h_flex,
        input::{Input, InputState},
        notification::Notification,
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

fn text_cell(value: impl Into<SharedString>, width: f32) -> impl IntoElement {
    div()
        .w(px(width))
        .flex_shrink_0()
        .text_sm()
        .truncate()
        .child(value.into())
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

        let initial_enroll = is_new && can_enroll && reader_available;
        let enroll_state = cx.new(|_| NewEnrollFormState {
            enabled: initial_enroll,
            consent_given: false,
            step: if initial_enroll {
                EnrollmentStep::WaitingFinger
            } else {
                EnrollmentStep::Idle
            },
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
                .title(if let Some(pid) = id {
                    format!("Editar pensionista #{pid}")
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
                        .when_some(id, |this, pid| {
                            this.child(
                                h_flex()
                                    .justify_between()
                                    .p_2p5()
                                    .rounded(cx.theme().radius)
                                    .border_1()
                                    .border_color(cx.theme().border)
                                    .bg(cx.theme().muted)
                                    .child(
                                        div()
                                            .text_xs()
                                            .text_color(cx.theme().muted_foreground)
                                            .child("Identificador único (inmutable)"),
                                    )
                                    .child(Tag::secondary().small().child(format!("#{pid}"))),
                            )
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
                                                                "Identificación de consumos suspendida durante la captura."
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
                                                .label("Consentimiento informado otorgado (alternativa: identificación manual)")
                                                .on_click(move |checked: &bool, _, cx| {
                                                    enroll_consent.update(cx, |s, cx| {
                                                        s.consent_given = *checked;
                                                        cx.notify();
                                                    });
                                                }),
                                        )
                                        .child(
                                            h_flex()
                                                .justify_between()
                                                .gap_2()
                                                .child(
                                                    h_flex()
                                                        .gap_2()
                                                        .when(is_busy, |this| {
                                                            this.child(Spinner::new().small())
                                                        })
                                                        .child(enrollment_tag(&enr.step))
                                                        .when_some(
                                                            match &enr.step {
                                                                EnrollmentStep::Registered { biometric_id } => {
                                                                    Some(format!("Código asignado: {biometric_id}"))
                                                                }
                                                                EnrollmentStep::Error(msg) => Some(msg.clone()),
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
                                                        .child(
                                                            Button::new("new-enroll-sim-capture")
                                                                .outline()
                                                                .xsmall()
                                                                .disabled(!enr.consent_given || is_busy || matches!(enr.step, EnrollmentStep::Registered { .. }))
                                                                .icon(LucideIcon::FingerprintPattern)
                                                                .label(match &enr.step {
                                                                    EnrollmentStep::Registered { .. } => "Captura simulada completada",
                                                                    EnrollmentStep::Error(_) => "Reintentar",
                                                                    _ => "Simular captura",
                                                                })
                                                                .on_click(move |_, _, cx| {
                                                                    let seq = enroll_capture.update(cx, |s, cx| {
                                                                        s.op_seq = s.op_seq.wrapping_add(1);
                                                                        s.step = EnrollmentStep::Capturing;
                                                                        cx.notify();
                                                                        s.op_seq
                                                                    });
                                                                    let enroll_weak = enroll_capture.downgrade();
                                                                    let app_for_dup = app_cap.clone();
                                                                    cx.spawn(async move |cx| {
                                                                        cx.background_spawn(async move {
                                                                            std::thread::sleep(Duration::from_millis(200));
                                                                        }).await;
                                                                        let still_valid = enroll_weak.update(cx, |s, cx| {
                                                                            if s.op_seq != seq {
                                                                                return false;
                                                                            }
                                                                            s.step = EnrollmentStep::Verifying;
                                                                            cx.notify();
                                                                            true
                                                                        }).unwrap_or(false);
                                                                        if !still_valid {
                                                                            return;
                                                                        }
                                                                        cx.background_spawn(async move {
                                                                            std::thread::sleep(Duration::from_millis(220));
                                                                        }).await;
                                                                        let candidates = app_for_dup
                                                                            .read_with(cx, |app, _| match &app.state.data {
                                                                                DataStatus::Ready(d) => d.biometric_candidates(),
                                                                                _ => Vec::new(),
                                                                            })
                                                                            .unwrap_or_default();
                                                                        let generated = generate_unique_biometric_id(&candidates);
                                                                        let _ = enroll_weak.update(cx, |s, cx| {
                                                                            if s.op_seq != seq {
                                                                                return;
                                                                            }
                                                                            s.step = EnrollmentStep::Registered {
                                                                                biometric_id: generated,
                                                                            };
                                                                            cx.notify();
                                                                        });
                                                                    }).detach();
                                                                }),
                                                        )
                                                        .child(
                                                            Button::new("new-enroll-sim-err")
                                                                .ghost()
                                                                .xsmall()
                                                                .disabled(!enr.consent_given || is_busy)
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
                                                .child(format!("Pensionista #{id} · {name}")),
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
                                            let seq = modal_cap.update(cx, |s, cx| {
                                                s.op_seq = s.op_seq.wrapping_add(1);
                                                s.step = EnrollmentStep::Capturing;
                                                cx.notify();
                                                s.op_seq
                                            });
                                            let weak_modal = modal_cap.downgrade();
                                            let app_ref = app_cap_cb.clone();
                                            cx.spawn(async move |cx| {
                                                cx.background_spawn(async move {
                                                    std::thread::sleep(Duration::from_millis(200));
                                                })
                                                .await;
                                                let ok = weak_modal
                                                    .update(cx, |s, cx| {
                                                        if s.op_seq != seq {
                                                            return false;
                                                        }
                                                        s.step = EnrollmentStep::Verifying;
                                                        cx.notify();
                                                        true
                                                    })
                                                    .unwrap_or(false);
                                                if !ok {
                                                    return;
                                                }
                                                cx.background_spawn(async move {
                                                    std::thread::sleep(Duration::from_millis(220));
                                                })
                                                .await;
                                                let candidates = app_ref
                                                    .read_with(cx, |app, _| match &app.state.data {
                                                        DataStatus::Ready(d) => {
                                                            d.biometric_candidates()
                                                        }
                                                        _ => Vec::new(),
                                                    })
                                                    .unwrap_or_default();
                                                let generated =
                                                    generate_unique_biometric_id(&candidates);
                                                let _ = weak_modal.update(cx, |s, cx| {
                                                    if s.op_seq != seq {
                                                        return;
                                                    }
                                                    s.step = EnrollmentStep::Registered {
                                                        biometric_id: generated,
                                                    };
                                                    cx.notify();
                                                });
                                            })
                                            .detach();
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
            "¿Deseas {} a {name} (ID #{id})?",
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

fn summary_box(title: &'static str, value: String, app: &App) -> impl IntoElement {
    let theme = app.theme();
    div()
        .flex_1()
        .min_w(px(170.))
        .p_3p5()
        .rounded(theme.radius_lg)
        .border_1()
        .border_color(theme.border)
        .bg(theme.colors.list)
        .child(
            v_flex()
                .gap_1()
                .child(
                    div()
                        .text_xs()
                        .text_color(theme.muted_foreground)
                        .child(title),
                )
                .child(
                    div()
                        .text_xl()
                        .font_weight(FontWeight::BOLD)
                        .text_color(theme.foreground)
                        .child(value),
                ),
        )
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
    let total_paid: i64 = payments.iter().map(|p| p.cents).sum();
    let balance = data.balance(id);

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
                                .child(Tag::secondary().small().child(format!("ID #{id}")))
                                .child(status_tag(active))
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
        .when(can_consumptions && can_payments, |this| {
            this.child(
                h_flex()
                    .gap_3()
                    .flex_wrap()
                    .child(summary_box("Total consumido", amount(total_consumed), app))
                    .child(summary_box("Total pagado", amount(total_paid), app))
                    .child(summary_box("Saldo pendiente", amount(balance), app)),
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
                                h_flex()
                                    .justify_between()
                                    .gap_3()
                                    .py_1p5()
                                    .border_b_1()
                                    .border_color(theme.border)
                                    .child(
                                        div().text_sm().child(format!(
                                            "{} {} · {} · {} ración(es)",
                                            item.date.format("%d/%m/%Y"),
                                            item.time,
                                            item.meal.label(),
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
                            .child("Historial de pagos"),
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
                                h_flex()
                                    .justify_between()
                                    .gap_3()
                                    .py_1p5()
                                    .border_b_1()
                                    .border_color(theme.border)
                                    .child(div().text_sm().child(format!(
                                        "{} {} · {} · {}",
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

pub fn render(
    state: &AppState,
    cx: &Context<DesktopApp>,
    app: &App,
    search: &Entity<InputState>,
    filter: Filter,
    selected: Option<u32>,
    enrollment_busy: bool,
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
    let query = search.read(app).value().to_lowercase();
    let mut rows = v_flex();
    let mut count = 0;
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
        if !haystack.contains(query.trim()) {
            continue;
        }
        count += 1;
        let id = person.id;
        let balance = data.balance(id);
        rows = rows.child(
            h_flex()
                .gap_3()
                .px_4()
                .py_3()
                .border_t_1()
                .border_color(theme.border)
                .child(text_cell(format!("#{id}"), 55.))
                .child(text_cell(person.name(), 200.))
                .child(text_cell(
                    person.phone.clone().unwrap_or_else(|| "—".into()),
                    115.,
                ))
                .child(div().w(px(90.)).flex_shrink_0().child(status_tag(person.active)))
                .child(
                    div()
                        .w(px(105.))
                        .flex_shrink_0()
                        .child(biometric_tag(person.biometric_status())),
                )
                .child(text_cell(amount(balance), 95.))
                .child(
                    h_flex()
                        .gap_1p5()
                        .child(
                            Button::new(format!("person-detail-{id}"))
                                .outline()
                                .small()
                                .icon(LucideIcon::Eye)
                                .label("Detalle")
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    this.selected_pensioner = Some(id);
                                    cx.notify();
                                })),
                        )
                        .when(allowed(state, "pensioners.update"), |this| {
                            let is_active = person.active;
                            this.child(
                                Button::new(format!("person-edit-{id}"))
                                    .ghost()
                                    .small()
                                    .icon(LucideIcon::Pencil)
                                    .tooltip("Editar")
                                    .on_click(cx.listener(move |this, _, window, cx| {
                                        this.pensioner_form(Some(id), window, cx)
                                    })),
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
                                    .on_click(cx.listener(move |this, _, window, cx| {
                                        this.confirm_change(id, window, cx)
                                    })),
                            )
                        })
                        .when(allowed(state, "biometrics.enroll"), |this| {
                            let has_fp = person.fingerprint_registered || person.biometric_id.is_some();
                            let is_active = person.active;
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
                                    .on_click(cx.listener(move |this, _, window, cx| {
                                        this.open_biometric_enroll_modal(id, window, cx)
                                    })),
                            )
                        }),
                ),
        );
    }
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
        .child(if count == 0 {
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
            div()
                .rounded(theme.radius_lg)
                .border_1()
                .border_color(theme.border)
                .bg(theme.colors.list)
                .overflow_x_scrollbar()
                .child(
                    v_flex()
                        .min_w(px(890.))
                        .child(
                            h_flex()
                                .gap_3()
                                .px_4()
                                .py_3()
                                .bg(theme.colors.table_head)
                                .text_xs()
                                .font_weight(FontWeight::SEMIBOLD)
                                .text_color(theme.muted_foreground)
                                .child(text_cell("ID", 55.))
                                .child(text_cell("Nombre completo", 200.))
                                .child(text_cell("Celular", 115.))
                                .child(text_cell("Estado", 90.))
                                .child(text_cell("Huella", 105.))
                                .child(text_cell("Saldo", 95.))
                                .child(text_cell("Acciones", 180.)),
                        )
                        .child(rows),
                )
                .into_any_element()
        })
        .into_any_element()
}
