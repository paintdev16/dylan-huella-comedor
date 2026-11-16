use crate::{
    layout::DesktopApp,
    mock_data::{MockData, Pensioner},
    state::{AppState, DataStatus, Page},
};
use gpui_kit::{
    App, AppContext as _, Context, Entity, FontWeight, IntoElement, ParentElement, SharedString,
    Styled, Window,
    assets::IconName as LucideIcon,
    component::{
        ActiveTheme as _, Icon, Sizable as _, WindowExt as _,
        alert::Alert,
        button::{Button, ButtonVariants as _},
        empty::{Empty, EmptyDescription, EmptyHeader, EmptyMedia, EmptyMediaVariant, EmptyTitle},
        group_box::{GroupBox, GroupBoxVariants as _},
        h_flex,
        input::{Input, InputState},
        notification::Notification,
        scroll::ScrollableElement as _,
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
        let app_for_enroll_root = app.clone();
        window.open_dialog(cx, move |dialog, _, cx| {
            let first_field = first.clone();
            let last_field = last.clone();
            let phone_field = phone.clone();
            let form_error_view = form_error.clone();
            let form_error_submit = form_error.clone();
            let app = app.clone();
            let app_for_enroll = app_for_enroll_root.clone();
            let current_error = form_error_view.read(cx).clone();

            dialog
                .title(if let Some(pid) = id {
                    format!("Editar pensionista #{pid}")
                } else {
                    "Nuevo pensionista".to_string()
                })
                .child(
                    v_flex()
                        .gap_4()
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
                                .gap_1p5()
                                .child(div().text_sm().font_weight(FontWeight::MEDIUM).child("Nombres *"))
                                .child(Input::new(&first)),
                        )
                        .child(
                            v_flex()
                                .gap_1p5()
                                .child(div().text_sm().font_weight(FontWeight::MEDIUM).child("Apellidos *"))
                                .child(Input::new(&last)),
                        )
                        .child(
                            v_flex()
                                .gap_1p5()
                                .child(
                                    div()
                                        .text_sm()
                                        .font_weight(FontWeight::MEDIUM)
                                        .child("Celular (opcional)"),
                                )
                                .child(Input::new(&phone)),
                        )
                        .when(id.is_none(), |this| {
                            this.child(
                                div()
                                    .text_xs()
                                    .text_color(cx.theme().muted_foreground)
                                    .child("El identificador único se asigna automáticamente y no se puede modificar."),
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
                                .on_click(|_, window, cx| window.close_dialog(cx)),
                        )
                        .child(
                            Button::new("pensioner-save")
                                .primary()
                                .label("Guardar")
                                .on_click(move |_, window, cx| {
                                    let first = first_field.read(cx).value().to_string();
                                    let last = last_field.read(cx).value().to_string();
                                    let phone = phone_field.read(cx).value().to_string();
                                    let result = app.update(cx, |this, cx| {
                                        if !this.has_pensioner_permission(permission) {
                                            return Err(
                                                "No tienes permiso para guardar.".to_string()
                                            );
                                        }
                                        let DataStatus::Ready(data) = &mut this.state.data else {
                                            return Err(
                                                "Los datos no están disponibles.".to_string()
                                            );
                                        };
                                        let saved_id =
                                            data.save_pensioner(id, &first, &last, &phone)?;
                                        this.selected_pensioner = Some(saved_id);
                                        let should_enroll = id.is_none()
                                            && this.has_pensioner_permission("biometrics.enroll");
                                        cx.notify();
                                        Ok((saved_id, should_enroll))
                                    });
                                    match result {
                                        Ok(Ok((saved_id, should_enroll))) => {
                                            form_error_submit.update(cx, |err, _| *err = None);
                                            window.close_dialog(cx);
                                            window.push_notification(
                                                Notification::success("Pensionista guardado."),
                                                cx,
                                            );
                                            if should_enroll {
                                                let _ = app_for_enroll.update(cx, |this, cx| {
                                                    this.confirm_change(
                                                        saved_id, true, window, cx,
                                                    );
                                                });
                                            }
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

    fn confirm_change(
        &mut self,
        id: u32,
        biometric: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let permission = if biometric {
            "biometrics.enroll"
        } else {
            "pensioners.update"
        };
        if !self.has_pensioner_permission(permission) {
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
        if biometric && !person.active {
            window.push_notification(
                Notification::error("Activa al pensionista antes de registrar la huella."),
                cx,
            );
            return;
        }
        let was_active = person.active;
        let name = person.name();
        let suggested_bio = person
            .biometric_id
            .clone()
            .unwrap_or_else(|| format!("BIO-{id:04}"));
        let bio_input = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder("Ej. BIO-0001")
                .default_value(suggested_bio)
        });
        let dialog_error: Entity<Option<String>> = cx.new(|_| None);
        let app = cx.entity().downgrade();
        let description = if biometric {
            format!(
                "Simulación para {name} (ID #{id}). No se captura ni almacena ninguna huella."
            )
        } else {
            format!(
                "¿Deseas {} a {name} (ID #{id})?",
                if was_active { "desactivar" } else { "activar" }
            )
        };
        window.open_dialog(cx, move |dialog, _, cx| {
            let app = app.clone();
            let bio_field = bio_input.clone();
            let bio_field_submit = bio_input.clone();
            let err_view = dialog_error.clone();
            let err_submit = dialog_error.clone();
            let current_err = err_view.read(cx).clone();
            dialog
                .title(if biometric {
                    "Registrar huella (simulado)"
                } else {
                    "Confirmar cambio de estado"
                })
                .child(
                    v_flex()
                        .gap_3()
                        .when_some(current_err, |this, err| {
                            this.child(Alert::error("biometric-enroll-error", err))
                        })
                        .when(biometric, |this| {
                            this.child(
                                h_flex()
                                    .gap_3()
                                    .p_3()
                                    .rounded(cx.theme().radius)
                                    .border_1()
                                    .border_color(cx.theme().border)
                                    .bg(cx.theme().muted)
                                    .child(
                                        Icon::new(LucideIcon::FingerprintPattern)
                                            .size_6()
                                            .text_color(cx.theme().primary),
                                    )
                                    .child(
                                        v_flex()
                                            .gap_0p5()
                                            .child(
                                                div()
                                                    .text_sm()
                                                    .font_weight(FontWeight::SEMIBOLD)
                                                    .child("Lector biométrico simulado"),
                                            )
                                            .child(
                                                div()
                                                    .text_xs()
                                                    .text_color(cx.theme().muted_foreground)
                                                    .child("Al confirmar, se asociará un código de prueba al pensionista."),
                                            ),
                                    ),
                            )
                            .child(
                                v_flex()
                                    .gap_1p5()
                                    .child(
                                        div()
                                            .text_sm()
                                            .font_weight(FontWeight::MEDIUM)
                                            .child("Código de simulación único *"),
                                    )
                                    .child(Input::new(&bio_field)),
                            )
                        })
                        .child(div().text_sm().child(description.clone())),
                )
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
                                .label(if biometric {
                                    "Simular registro"
                                } else if was_active {
                                    "Desactivar"
                                } else {
                                    "Activar"
                                })
                                .on_click(move |_, window, cx| {
                                    let bio_code = bio_field_submit.read(cx).value().to_string();
                                    let result = app.update(cx, |this, cx| {
                                        if !this.has_pensioner_permission(permission) {
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
                                        if biometric {
                                            let registered =
                                                data.register_fingerprint_with_id(id, &bio_code)?;
                                            this.push_biometric_event(
                                                crate::biometric_service::BiometricEvent::Enrolled {
                                                    pensioner_id: id,
                                                    biometric_id: registered,
                                                },
                                            );
                                        } else {
                                            data.set_active(id, was_active)
                                                .map_err(|e| e.to_string())?;
                                        }
                                        cx.notify();
                                        Ok(())
                                    });
                                    match result {
                                        Ok(Ok(())) => {
                                            err_submit.update(cx, |err, _| *err = None);
                                            window.close_dialog(cx);
                                            window.push_notification(
                                                Notification::success(if biometric {
                                                    "Registro de huella simulado."
                                                } else {
                                                    "Estado actualizado."
                                                }),
                                                cx,
                                            );
                                        }
                                        Ok(Err(error)) => {
                                            err_submit.update(cx, |err, cx| {
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
                )
                .child(
                    h_flex()
                        .gap_2()
                        .flex_wrap()
                        .when(allowed(state, "pensioners.update"), |this| {
                            this.child(
                                Button::new("edit-person")
                                    .outline()
                                    .small()
                                    .icon(LucideIcon::Pencil)
                                    .label("Editar")
                                    .on_click(cx.listener(move |this, _, window, cx| {
                                        this.pensioner_form(Some(id), window, cx)
                                    })),
                            )
                        })
                        .when(allowed(state, "pensioners.update"), |this| {
                            this.child(
                                Button::new("toggle-person")
                                    .outline()
                                    .small()
                                    .icon(if active {
                                        LucideIcon::UserX
                                    } else {
                                        LucideIcon::UserCheck
                                    })
                                    .label(if active { "Desactivar" } else { "Activar" })
                                    .on_click(cx.listener(move |this, _, window, cx| {
                                        this.confirm_change(id, false, window, cx)
                                    })),
                            )
                        })
                        .when(allowed(state, "biometrics.enroll") && active, |this| {
                            this.child(
                                Button::new("enroll-person")
                                    .primary()
                                    .small()
                                    .icon(LucideIcon::FingerprintPattern)
                                    .label("Registrar huella")
                                    .on_click(cx.listener(move |this, _, window, cx| {
                                        this.confirm_change(id, true, window, cx)
                                    })),
                            )
                        }),
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
                        .min_w(px(830.))
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
                                .child(text_cell("Acciones", 120.)),
                        )
                        .child(rows),
                )
                .into_any_element()
        })
        .into_any_element()
}
