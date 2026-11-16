use std::time::Duration;

use gpui_kit::{
    App, AppContext as _, Context, FontWeight, IntoElement, ParentElement, Styled, Window,
    assets::IconName as LucideIcon,
    component::{
        ActiveTheme as _, Disableable as _, Icon, Sizable as _, WindowExt as _,
        alert::Alert,
        button::{Button, ButtonVariants as _},
        group_box::{GroupBox, GroupBoxVariants as _},
        h_flex,
        input::Input,
        notification::Notification,
        separator::Separator,
        switch::Switch,
        tag::Tag,
        v_flex,
    },
    div,
    prelude::FluentBuilder as _,
    px,
};

use crate::{
    biometric_service::BiometricGateway as _,
    layout::DesktopApp,
    mock_data::parse_price_cents,
    settings_store::{
        BiometricBackupStatus, CredentialVault as _, ServerConnectionStatus, ThemePreference,
        default_preferences_path, save_preferences_to, validate_server_url,
    },
    state::{AppState, DataStatus, ServiceStatus},
};

impl DesktopApp {
    pub fn persist_preferences(&self) -> Result<(), String> {
        save_preferences_to(&default_preferences_path(), &self.preferences)
    }

    pub fn set_theme_preference(
        &mut self,
        pref: ThemePreference,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.preferences.theme_preference = pref;
        match pref {
            ThemePreference::System => {
                self.follow_system_theme = true;
                self.apply_theme_mode(window.appearance().into(), window, cx);
            }
            ThemePreference::Light => {
                self.follow_system_theme = false;
                self.apply_theme_mode(gpui_kit::component::ThemeMode::Light, window, cx);
            }
            ThemePreference::Dark => {
                self.follow_system_theme = false;
                self.apply_theme_mode(gpui_kit::component::ThemeMode::Dark, window, cx);
            }
        }
        let _ = self.persist_preferences();
    }

    pub fn save_general_settings(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let hall_name = self
            .cfg_dining_hall_input
            .read(cx)
            .value()
            .trim()
            .to_string();
        if hall_name.chars().count() < 2 {
            window.push_notification(
                Notification::error("El nombre del comedor debe tener al menos 2 caracteres."),
                cx,
            );
            return;
        }

        let b_raw = self.cfg_breakfast_input.read(cx).value().to_string();
        let l_raw = self.cfg_lunch_input.read(cx).value().to_string();
        let s_raw = self.cfg_special_input.read(cx).value().to_string();
        let t_raw = self.cfg_ticket_input.read(cx).value().to_string();

        let parsed = (|| -> Result<(i64, i64, i64, i64), String> {
            Ok((
                parse_price_cents(&b_raw)?,
                parse_price_cents(&l_raw)?,
                parse_price_cents(&s_raw)?,
                parse_price_cents(&t_raw)?,
            ))
        })();

        let (b_cents, l_cents, s_cents, t_cents) = match parsed {
            Ok(v) => v,
            Err(err) => {
                window.push_notification(Notification::error(err), cx);
                return;
            }
        };

        self.preferences.dining_hall_name = hall_name;
        self.preferences.currency_code = "PEN".to_string();
        self.preferences.economic_breakfast_cents = b_cents;
        self.preferences.economic_lunch_cents = l_cents;
        self.preferences.special_default_cents = s_cents;
        self.preferences.ticket_value_cents = t_cents;
        self.preferences.sidebar_collapsed = self.state.sidebar_collapsed;

        self.consumption_pricing.economic_breakfast_cents = b_cents;
        self.consumption_pricing.economic_lunch_cents = l_cents;
        self.consumption_pricing.special_default_cents = s_cents;
        if let DataStatus::Ready(data) = &mut self.state.data {
            data.cash_register.ticket_value_cents = t_cents;
        }

        match self.persist_preferences() {
            Ok(()) => {
                window.push_notification(
                    Notification::success("Configuración general guardada localmente."),
                    cx,
                );
            }
            Err(err) => {
                window.push_notification(Notification::error(err), cx);
            }
        }
        cx.notify();
    }

    pub fn save_server_settings(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let raw_url = self.cfg_server_url_input.read(cx).value().to_string();
        let url = match validate_server_url(&raw_url) {
            Ok(u) => u,
            Err(err) => {
                self.server_status = ServerConnectionStatus::Failed(err.clone());
                window.push_notification(Notification::error(err), cx);
                cx.notify();
                return;
            }
        };
        let station = self.cfg_station_id_input.read(cx).value().trim().to_string();
        if station.chars().count() < 2 {
            window.push_notification(
                Notification::error("El identificador de la estación debe tener al menos 2 caracteres."),
                cx,
            );
            return;
        }

        self.preferences.server_url = url.clone();
        self.preferences.station_id = station;
        self.cfg_server_url_input.update(cx, |input, cx| {
            input.set_value(url, window, cx);
        });

        match self.persist_preferences() {
            Ok(()) => window.push_notification(
                Notification::success("Configuración de servidor guardada (sin credenciales)."),
                cx,
            ),
            Err(err) => window.push_notification(Notification::error(err), cx),
        }
        cx.notify();
    }

    pub fn test_server_connection(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let raw_url = self.cfg_server_url_input.read(cx).value().to_string();
        let url = match validate_server_url(&raw_url) {
            Ok(u) => u,
            Err(err) => {
                self.server_status = ServerConnectionStatus::Failed(err.clone());
                window.push_notification(Notification::error(err), cx);
                cx.notify();
                return;
            }
        };
        let station = self.cfg_station_id_input.read(cx).value().trim().to_string();
        if station.is_empty() {
            window.push_notification(
                Notification::error("Indica el identificador de la estación."),
                cx,
            );
            return;
        }

        self.server_status = ServerConnectionStatus::Testing;
        cx.notify();

        let bg_task = cx.background_spawn(async move {
            std::thread::sleep(Duration::from_millis(320));
            if url.contains("error") || url.contains("fail") {
                Err(format!(
                    "No se pudo conectar con {url} (simulación de error)."
                ))
            } else {
                Ok(format!(
                    "Conexión simulada exitosa con {url} (Estación {station})."
                ))
            }
        });

        self.server_test_task = Some(cx.spawn(async move |this, cx| {
            let outcome = bg_task.await;
            let _ = this.update(cx, |app, cx| {
                app.server_status = match outcome {
                    Ok(msg) => ServerConnectionStatus::Connected(msg),
                    Err(err) => ServerConnectionStatus::Failed(err),
                };
                app.server_test_task = None;
                cx.notify();
            });
        }));
    }

    pub fn simulate_biometric_backup(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let count = match &self.state.data {
            DataStatus::Ready(d) => d
                .pensioners
                .iter()
                .filter(|p| p.fingerprint_registered)
                .count(),
            _ => 0,
        };
        self.biometric_backup_status = BiometricBackupStatus::BackedUp(format!(
            "Respaldo local simulado ({count} plantilla(s)) · sin claves en texto plano."
        ));
        window.push_notification(
            Notification::success("Respaldo biométrico simulado completado."),
            cx,
        );
        cx.notify();
    }

    pub fn simulate_sdk_check(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        window.push_notification(
            Notification::info(
                "SDK objetivo: SecuGen FDx SDK Pro + SecuSearch Pro (integración diferida a crates/biometric).",
            ),
            cx,
        );
    }
}

/// Página de configuración del sistema: general, servidor Axum, biometría y persistencia.
pub fn render(
    state: &AppState,
    app_view: &DesktopApp,
    cx: &Context<DesktopApp>,
    app_cx: &App,
) -> impl IntoElement {
    let theme = app_cx.theme();
    let is_dark = theme.is_dark();
    let is_loading = matches!(state.service_status, ServiceStatus::Loading);
    let is_testing_server = matches!(app_view.server_status, ServerConnectionStatus::Testing);

    let template_count = match &state.data {
        DataStatus::Ready(d) => d
            .pensioners
            .iter()
            .filter(|p| p.fingerprint_registered)
            .count(),
        _ => 0,
    };

    let reader_state = app_view.biometric_gateway.state();
    let reader_connected = reader_state.is_connected();

    let server_tag = match &app_view.server_status {
        ServerConnectionStatus::Untested => Tag::secondary().small().child("Sin probar"),
        ServerConnectionStatus::Testing => Tag::info().small().child("Probando..."),
        ServerConnectionStatus::Connected(_) => Tag::success().small().child("Conectado (simulado)"),
        ServerConnectionStatus::Failed(_) => Tag::danger().small().child("Error de conexión"),
    };

    let server_msg = match &app_view.server_status {
        ServerConnectionStatus::Untested => {
            "Aún no se ha ejecutado una prueba de conexión hacia el backend Axum.".to_string()
        }
        ServerConnectionStatus::Testing => "Ejecutando prueba de conexión simulada...".to_string(),
        ServerConnectionStatus::Connected(m) | ServerConnectionStatus::Failed(m) => m.clone(),
    };

    let backup_msg = match &app_view.biometric_backup_status {
        BiometricBackupStatus::Pending => "Respaldo simulado pendiente".to_string(),
        BiometricBackupStatus::BackedUp(msg) => msg.clone(),
    };

    let diagnostic = match &state.service_status {
        ServiceStatus::Ready(info) => format!(
            "{} · {} · {} · {} módulos pendientes",
            info.os_label, info.app_version, info.mode_label, info.pending_modules
        ),
        ServiceStatus::Failed(message) => format!("Último error: {message}"),
        ServiceStatus::Loading => "Verificando…".to_string(),
        ServiceStatus::Idle => "Pendiente de verificación".to_string(),
    };

    v_flex()
        .gap_5()
        .child(
            GroupBox::new()
                .outline()
                .title("Configuración general")
                .child(
                    v_flex()
                        .gap_4()
                        .child(
                            h_flex()
                                .gap_4()
                                .flex_wrap()
                                .child(
                                    v_flex()
                                        .flex_1()
                                        .min_w(px(240.))
                                        .gap_1()
                                        .child(
                                            div()
                                                .text_sm()
                                                .font_weight(FontWeight::MEDIUM)
                                                .child("Nombre del comedor *"),
                                        )
                                        .child(Input::new(&app_view.cfg_dining_hall_input)),
                                )
                                .child(
                                    v_flex()
                                        .w(px(170.))
                                        .gap_1()
                                        .child(
                                            div()
                                                .text_sm()
                                                .font_weight(FontWeight::MEDIUM)
                                                .child("Moneda"),
                                        )
                                        .child(
                                            h_flex()
                                                .h(px(34.))
                                                .px_3()
                                                .rounded(theme.radius)
                                                .border_1()
                                                .border_color(theme.border)
                                                .bg(theme.muted)
                                                .justify_between()
                                                .child(div().text_sm().child("PEN (S/)"))
                                                .child(Tag::secondary().small().child("Soles")),
                                        ),
                                ),
                        )
                        .child(Separator::horizontal())
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
                                                .text_sm()
                                                .font_weight(FontWeight::MEDIUM)
                                                .child("Preferencia de tema"),
                                        )
                                        .child(
                                            div()
                                                .text_xs()
                                                .text_color(theme.muted_foreground)
                                                .child(format!(
                                                    "Tema activo: {}",
                                                    if is_dark { "Oscuro" } else { "Claro" }
                                                )),
                                        ),
                                )
                                .child(
                                    h_flex().gap_2().children(
                                        [
                                            ThemePreference::System,
                                            ThemePreference::Light,
                                            ThemePreference::Dark,
                                        ]
                                        .into_iter()
                                        .map(|pref| {
                                            let active =
                                                app_view.preferences.theme_preference == pref;
                                            Button::new(format!("cfg-theme-{}", pref.label()))
                                                .small()
                                                .when(active, |b| b.primary())
                                                .when(!active, |b| b.outline())
                                                .label(pref.label())
                                                .on_click(cx.listener(move |this, _, window, cx| {
                                                    this.set_theme_preference(pref, window, cx);
                                                }))
                                        }),
                                    ),
                                ),
                        )
                        .child(
                            h_flex()
                                .justify_between()
                                .gap_4()
                                .child(
                                    v_flex()
                                        .gap_1()
                                        .child(
                                            div()
                                                .text_sm()
                                                .font_weight(FontWeight::MEDIUM)
                                                .child("Menú lateral compacto"),
                                        )
                                        .child(
                                            div()
                                                .text_xs()
                                                .text_color(theme.muted_foreground)
                                                .child("Contrae el sidebar izquierdo dejando visibles solo los iconos."),
                                        ),
                                )
                                .child(
                                    Switch::new("settings-sidebar-collapsed")
                                        .checked(state.sidebar_collapsed)
                                        .accessibility_label("Contraer menú lateral")
                                        .on_change(cx.listener(|this, checked: &bool, _, cx| {
                                            this.set_sidebar_collapsed(*checked, cx);
                                            this.preferences.sidebar_collapsed = *checked;
                                            let _ = this.persist_preferences();
                                        })),
                                ),
                        )
                        .child(Separator::horizontal())
                        .child(
                            v_flex()
                                .gap_2()
                                .child(
                                    div()
                                        .text_sm()
                                        .font_weight(FontWeight::MEDIUM)
                                        .child("Precios predeterminados de raciones y tickets (PEN)"),
                                )
                                .child(
                                    h_flex()
                                        .gap_3()
                                        .flex_wrap()
                                        .child(
                                            v_flex()
                                                .flex_1()
                                                .min_w(px(140.))
                                                .gap_1()
                                                .child(div().text_xs().text_color(theme.muted_foreground).child("Desayuno econ. (S/)"))
                                                .child(Input::new(&app_view.cfg_breakfast_input)),
                                        )
                                        .child(
                                            v_flex()
                                                .flex_1()
                                                .min_w(px(140.))
                                                .gap_1()
                                                .child(div().text_xs().text_color(theme.muted_foreground).child("Almuerzo econ. (S/)"))
                                                .child(Input::new(&app_view.cfg_lunch_input)),
                                        )
                                        .child(
                                            v_flex()
                                                .flex_1()
                                                .min_w(px(140.))
                                                .gap_1()
                                                .child(div().text_xs().text_color(theme.muted_foreground).child("Especial sug. (S/)"))
                                                .child(Input::new(&app_view.cfg_special_input)),
                                        )
                                        .child(
                                            v_flex()
                                                .flex_1()
                                                .min_w(px(140.))
                                                .gap_1()
                                                .child(div().text_xs().text_color(theme.muted_foreground).child("Ticket físico (S/)"))
                                                .child(Input::new(&app_view.cfg_ticket_input)),
                                        ),
                                ),
                        )
                        .child(
                            h_flex().justify_end().child(
                                Button::new("cfg-save-general")
                                    .primary()
                                    .small()
                                    .icon(LucideIcon::Check)
                                    .label("Guardar configuración general")
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        this.save_general_settings(window, cx);
                                    })),
                            ),
                        ),
                ),
        )
        .child(
            GroupBox::new()
                .outline()
                .title("Configuración de servidor (Backend Axum)")
                .child(
                    v_flex()
                        .gap_4()
                        .child(
                            h_flex()
                                .gap_4()
                                .flex_wrap()
                                .child(
                                    v_flex()
                                        .flex_1()
                                        .min_w(px(260.))
                                        .gap_1()
                                        .child(div().text_sm().font_weight(FontWeight::MEDIUM).child("URL del backend Axum *"))
                                        .child(Input::new(&app_view.cfg_server_url_input)),
                                )
                                .child(
                                    v_flex()
                                        .w(px(190.))
                                        .gap_1()
                                        .child(div().text_sm().font_weight(FontWeight::MEDIUM).child("ID de estación *"))
                                        .child(Input::new(&app_view.cfg_station_id_input)),
                                ),
                        )
                        .child(
                            h_flex()
                                .justify_between()
                                .gap_3()
                                .flex_wrap()
                                .p_3()
                                .rounded(theme.radius)
                                .border_1()
                                .border_color(theme.border)
                                .bg(theme.muted)
                                .child(
                                    v_flex()
                                        .gap_0p5()
                                        .child(h_flex().gap_2().child(div().text_xs().font_weight(FontWeight::SEMIBOLD).child("Estado de conexión")).child(server_tag))
                                        .child(div().text_xs().text_color(theme.muted_foreground).child(server_msg)),
                                )
                                .child(
                                    h_flex()
                                        .gap_2()
                                        .child(
                                            Button::new("cfg-test-server")
                                                .outline()
                                                .small()
                                                .loading(is_testing_server)
                                                .label("Probar conexión")
                                                .on_click(cx.listener(|this, _, window, cx| {
                                                    this.test_server_connection(window, cx);
                                                })),
                                        )
                                        .child(
                                            Button::new("cfg-save-server")
                                                .primary()
                                                .small()
                                                .label("Guardar servidor")
                                                .on_click(cx.listener(|this, _, window, cx| {
                                                    this.save_server_settings(window, cx);
                                                })),
                                        ),
                                ),
                        ),
                ),
        )
        .child(
            GroupBox::new()
                .outline()
                .title("Configuración biométrica")
                .child(
                    v_flex()
                        .gap_3()
                        .child(
                            h_flex()
                                .gap_3()
                                .flex_wrap()
                                .child(
                                    div()
                                        .flex_1()
                                        .min_w(px(200.))
                                        .p_3()
                                        .rounded(theme.radius)
                                        .border_1()
                                        .border_color(theme.border)
                                        .child(
                                            v_flex()
                                                .gap_1()
                                                .child(div().text_xs().text_color(theme.muted_foreground).child("Estado del lector"))
                                                .child(div().text_sm().font_weight(FontWeight::BOLD).child(reader_state.label())),
                                        ),
                                )
                                .child(
                                    div()
                                        .flex_1()
                                        .min_w(px(200.))
                                        .p_3()
                                        .rounded(theme.radius)
                                        .border_1()
                                        .border_color(theme.border)
                                        .child(
                                            v_flex()
                                                .gap_1()
                                                .child(div().text_xs().text_color(theme.muted_foreground).child("Plantillas registradas"))
                                                .child(div().text_sm().font_weight(FontWeight::BOLD).child(format!("{template_count} plantilla(s)"))),
                                        ),
                                )
                                .child(
                                    div()
                                        .flex_1()
                                        .min_w(px(220.))
                                        .p_3()
                                        .rounded(theme.radius)
                                        .border_1()
                                        .border_color(theme.border)
                                        .child(
                                            v_flex()
                                                .gap_1()
                                                .child(div().text_xs().text_color(theme.muted_foreground).child("SDK objetivo"))
                                                .child(div().text_xs().font_weight(FontWeight::SEMIBOLD).child("SecuGen FDx SDK Pro / SecuSearch Pro")),
                                        ),
                                ),
                        )
                        .child(
                            div()
                                .text_xs()
                                .text_color(theme.muted_foreground)
                                .child(format!("Estado del respaldo: {backup_msg}")),
                        )
                        .child(
                            h_flex()
                                .gap_2()
                                .flex_wrap()
                                .child(
                                    Button::new("cfg-bio-toggle-reader")
                                        .outline()
                                        .small()
                                        .icon(LucideIcon::FingerprintPattern)
                                        .label(if reader_connected {
                                            "Desconectar lector"
                                        } else {
                                            "Conectar lector simulado"
                                        })
                                        .on_click(cx.listener(move |this, _, window, cx| {
                                            if reader_connected {
                                                this.biometric_disconnect(window, cx);
                                            } else {
                                                this.biometric_connect(window, cx);
                                            }
                                        })),
                                )
                                .child(
                                    Button::new("cfg-bio-sdk")
                                        .outline()
                                        .small()
                                        .label("Verificar SDK (simulado)")
                                        .on_click(cx.listener(|this, _, window, cx| {
                                            this.simulate_sdk_check(window, cx);
                                        })),
                                )
                                .child(
                                    Button::new("cfg-bio-backup")
                                        .outline()
                                        .small()
                                        .label("Simular respaldo")
                                        .on_click(cx.listener(|this, _, window, cx| {
                                            this.simulate_biometric_backup(window, cx);
                                        })),
                                ),
                        ),
                ),
        )
        .child(
            GroupBox::new()
                .outline()
                .title("Persistencia y seguridad de credenciales")
                .child(
                    v_flex()
                        .gap_3()
                        .child(
                            Alert::info(
                                "cfg-security-note",
                                "Solo se guardan preferencias no sensibles en disco. Las contraseñas de PostgreSQL y claves biométricas nunca se almacenan en texto plano.",
                            ),
                        )
                        .child(
                            div()
                                .text_xs()
                                .text_color(theme.muted_foreground)
                                .child(format!(
                                    "Archivo local: {}",
                                    default_preferences_path().display()
                                )),
                        )
                        .child(
                            div()
                                .text_xs()
                                .text_color(theme.muted_foreground)
                                .child(format!(
                                    "Almacén de credenciales: {}",
                                    app_view.credential_vault.backend_name()
                                )),
                        )
                        .child(
                            div()
                                .text_xs()
                                .text_color(theme.muted_foreground)
                                .child(format!("Diagnóstico local: {diagnostic}")),
                        )
                        .child(
                            h_flex()
                                .gap_3()
                                .flex_wrap()
                                .child(
                                    Button::new("settings-run-diagnostics")
                                        .outline()
                                        .small()
                                        .loading(is_loading)
                                        .icon(Icon::new(LucideIcon::ShieldCheck))
                                        .label("Verificar entorno local")
                                        .on_click(cx.listener(|this, _, _, cx| {
                                            this.start_diagnostics(false, cx);
                                        })),
                                )
                                .child(
                                    Button::new("settings-test-error")
                                        .outline()
                                        .small()
                                        .disabled(is_loading)
                                        .icon(Icon::new(LucideIcon::CircleAlert))
                                        .label("Simular error recuperable")
                                        .on_click(cx.listener(|this, _, _, cx| {
                                            this.start_diagnostics(true, cx);
                                        })),
                                ),
                        ),
                ),
        )
}
