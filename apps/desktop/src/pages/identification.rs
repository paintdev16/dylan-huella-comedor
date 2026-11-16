use std::{
    sync::{mpsc, Arc, Mutex},
    time::Duration,
};

use chrono::Timelike as _;
use gpui_kit::{
    assets::IconName as LucideIcon,
    component::{
        alert::Alert,
        button::{Button, ButtonVariants as _},
        group_box::{GroupBox, GroupBoxVariants as _},
        h_flex,
        input::{Input, InputState},
        notification::Notification,
        searchable_list::SearchableListItem,
        select::{Select, SelectEvent, SelectState},
        spinner::Spinner,
        switch::Switch,
        tag::Tag,
        v_flex, ActiveTheme as _, Disableable as _, Icon, IndexPath, Sizable as _, WindowExt as _,
    },
    div,
    prelude::FluentBuilder as _,
    px, App, AppContext as _, Context, FontWeight, IntoElement, ParentElement, SharedString,
    Styled, Subscription, Window,
};
use unicode_normalization::{char::is_combining_mark, UnicodeNormalization};

use crate::{
    biometric_service::{
        BiometricError, BiometricEvent, BiometricReaderState, ContinuousBiometricSignal,
        IdentificationOutcome,
    },
    layout::DesktopApp,
    mock_data::{parse_price_cents, DraftConsumptionItem, Meal, MenuKind, MockData},
    state::{AppState, DataStatus, Page},
};

#[derive(Clone, Copy)]
pub enum SimulationMode {
    MatchActive,
    NoMatch,
    DeviceError,
}

pub(crate) fn allowed(state: &AppState, permission: &str) -> bool {
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

pub fn reader_state_tag(reader_state: &BiometricReaderState) -> Tag {
    let label = reader_state.label();
    match reader_state {
        BiometricReaderState::Disconnected => Tag::secondary().small().child(label),
        BiometricReaderState::Connected => Tag::info().outline().small().child(label),
        BiometricReaderState::WaitingFingerprint => Tag::warning().outline().small().child(label),
        BiometricReaderState::Identifying => Tag::info().small().child(label),
        BiometricReaderState::Identified { .. } => Tag::success().small().child(label),
        BiometricReaderState::Unrecognized => Tag::warning().small().child(label),
        BiometricReaderState::Error(_) => Tag::danger().small().child(label),
    }
}

pub(crate) fn state_tag(reader_state: &BiometricReaderState) -> Tag {
    reader_state_tag(reader_state)
}

/// Horario configurable para seleccionar inicialmente Desayuno o Almuerzo.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MealSchedule {
    pub breakfast_start_minutes: u32,
    pub breakfast_end_minutes: u32,
    pub lunch_start_minutes: u32,
    pub lunch_end_minutes: u32,
}

impl Default for MealSchedule {
    fn default() -> Self {
        Self {
            breakfast_start_minutes: 6 * 60,
            breakfast_end_minutes: 11 * 60,
            lunch_start_minutes: 11 * 60,
            lunch_end_minutes: 16 * 60,
        }
    }
}

impl MealSchedule {
    pub fn from_preferences(prefs: &crate::settings_store::AppPreferences) -> Self {
        Self {
            breakfast_start_minutes: prefs.breakfast_start_minutes,
            breakfast_end_minutes: prefs.breakfast_end_minutes,
            lunch_start_minutes: prefs.lunch_start_minutes,
            lunch_end_minutes: prefs.lunch_end_minutes,
        }
    }

    pub fn meal_for_time(self, hour: u32, minute: u32) -> Meal {
        let total = hour.saturating_mul(60).saturating_add(minute);
        if total >= self.breakfast_start_minutes && total < self.breakfast_end_minutes {
            Meal::Breakfast
        } else if total >= self.lunch_start_minutes && total < self.lunch_end_minutes {
            Meal::Lunch
        } else if total < self.lunch_start_minutes {
            Meal::Breakfast
        } else {
            Meal::Lunch
        }
    }

    #[allow(dead_code)]
    pub fn meal_for_hour(self, hour: u32) -> Meal {
        self.meal_for_time(hour, 0)
    }

    pub fn current_meal(self) -> Meal {
        let now = chrono::Local::now();
        self.meal_for_time(now.hour(), now.minute())
    }
}

struct AutoModalState {
    meal: Meal,
    menu_kind: MenuKind,
    portions: u32,
    third_party: bool,
    cart: Vec<DraftConsumptionItem>,
    next_item_id: u64,
    error: Option<String>,
    submitting: bool,
    token: u64,
    _meal_subscription: Option<Subscription>,
}

/// Normaliza texto para búsqueda insensible a mayúsculas y acentos.
pub fn normalize_search_text(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    for ch in input.trim().nfd().filter(|c| !is_combining_mark(*c)) {
        for lc in ch.to_lowercase() {
            let mapped = match lc {
                'á' | 'à' | 'ä' | 'â' | 'ã' | 'å' => 'a',
                'é' | 'è' | 'ë' | 'ê' => 'e',
                'í' | 'ì' | 'ï' | 'î' => 'i',
                'ó' | 'ò' | 'ö' | 'ô' | 'õ' => 'o',
                'ú' | 'ù' | 'ü' | 'û' => 'u',
                'ñ' => 'n',
                'ç' => 'c',
                other => other,
            };
            out.push(mapped);
        }
    }
    out
}

/// Elemento del Combobox global de búsqueda de pensionistas activos.
#[derive(Clone, PartialEq, Eq)]
pub struct GlobalPensionerChoice {
    pub id: u32,
    pub full_name: String,
    pub phone: Option<String>,
    pub biometric_id: Option<String>,
    pub balance_cents: i64,
    pub search_haystack: String,
}

impl SearchableListItem for GlobalPensionerChoice {
    type Value = u32;

    fn title(&self) -> SharedString {
        format!(
            "{} · ID #{} · Saldo: {}",
            self.full_name,
            self.id,
            amount(self.balance_cents)
        )
        .into()
    }

    fn render(&self, _: &mut Window, app: &mut App) -> impl IntoElement {
        let theme = app.theme();
        h_flex()
            .w_full()
            .justify_between()
            .items_center()
            .gap_2()
            .child(
                v_flex()
                    .gap_0p5()
                    .min_w_0()
                    .child(
                        div()
                            .text_xs()
                            .font_weight(FontWeight::SEMIBOLD)
                            .truncate()
                            .child(self.full_name.clone()),
                    )
                    .child(div().text_xs().text_color(theme.muted_foreground).child(
                        match &self.phone {
                            Some(ph) if !ph.trim().is_empty() => {
                                format!("ID #{} · {}", self.id, ph)
                            }
                            _ => format!("ID #{}", self.id),
                        },
                    )),
            )
            .child(
                div()
                    .text_xs()
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(if self.balance_cents > 0 {
                        theme.warning
                    } else {
                        theme.muted_foreground
                    })
                    .child(format!("Saldo: {}", amount(self.balance_cents))),
            )
    }

    fn value(&self) -> &Self::Value {
        &self.id
    }

    fn matches(&self, query: &str) -> bool {
        let norm_query = normalize_search_text(query);
        if norm_query.is_empty() {
            return true;
        }
        norm_query
            .split_whitespace()
            .all(|tok| self.search_haystack.contains(tok))
    }
}

/// Construye la lista de sugerencias del Combobox global solo con pensionistas activos.
pub fn build_global_pensioner_choices(data: &MockData) -> Vec<GlobalPensionerChoice> {
    let mut items: Vec<GlobalPensionerChoice> = data
        .pensioners
        .iter()
        .filter(|p| p.active)
        .map(|p| {
            let full_name = p.name();
            let phone_raw = p.phone.as_deref().unwrap_or("");
            let phone_digits: String = phone_raw.chars().filter(|c| c.is_ascii_digit()).collect();
            let search_haystack = normalize_search_text(&format!(
                "{} #{} {} {} {} {}",
                full_name,
                p.id,
                p.id,
                p.biometric_id.as_deref().unwrap_or(""),
                phone_raw,
                phone_digits
            ));
            GlobalPensionerChoice {
                id: p.id,
                full_name,
                phone: p.phone.clone(),
                biometric_id: p.biometric_id.clone(),
                balance_cents: data.balance(p.id),
                search_haystack,
            }
        })
        .collect();
    items.sort_by_key(|p| p.id);
    items
}

/// Agrega una selección al carrito mixto agrupando únicamente mismo turno, tipo y precio.
fn add_to_mixed_cart(
    cart: &mut Vec<DraftConsumptionItem>,
    next_id: &mut u64,
    meal: Meal,
    menu_kind: MenuKind,
    portions: u32,
    unit_price_cents: i64,
) -> Result<(), String> {
    if portions == 0 {
        return Err("La cantidad de raciones debe ser mayor a cero.".into());
    }
    if unit_price_cents <= 0 {
        return Err("El precio unitario debe ser mayor a S/ 0.00.".into());
    }

    if let Some(existing) = cart.iter_mut().find(|item| {
        item.meal == meal
            && item.menu_kind == menu_kind
            && item.unit_price_cents == unit_price_cents
    }) {
        existing.portions = existing.portions.saturating_add(portions);
    } else {
        let id = *next_id;
        *next_id += 1;
        let description = match menu_kind {
            MenuKind::Economic => "Ración económica".to_string(),
            MenuKind::Special => "Plato especial".to_string(),
        };
        cart.push(DraftConsumptionItem {
            id,
            meal,
            menu_kind,
            description,
            portions,
            unit_price_cents,
        });
    }
    Ok(())
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

    /// Activa el servicio biométrico continuo al iniciar sesión si el usuario tiene permiso
    /// y la identificación simulada está habilitada en Configuración.
    pub fn activate_continuous_biometrics(&mut self, cx: &mut Context<Self>) {
        let can_use = self
            .state
            .auth
            .user()
            .is_some_and(|u| u.can("biometrics.identify") && u.can("biometrics.view"));
        if !can_use || !self.preferences.biometric_identification_enabled {
            return;
        }
        if !self.biometric_gateway.state().is_connected() {
            if let Ok(evt) = self.biometric_gateway.connect() {
                self.push_biometric_event(evt);
            }
        }
        if !matches!(
            self.biometric_gateway.state(),
            BiometricReaderState::WaitingFingerprint
        ) {
            if let Ok(wait_evt) = self.biometric_gateway.start_waiting() {
                self.push_biometric_event(wait_evt);
            }
        }
        self.start_biometric_signal_loop(cx);
        cx.notify();
    }

    fn start_biometric_signal_loop(&mut self, cx: &mut Context<Self>) {
        let (tx, rx) = mpsc::channel::<ContinuousBiometricSignal>();
        self.biometric_signal_tx = Some(tx);
        let rx = Arc::new(Mutex::new(rx));

        self.biometric_listener_task = Some(cx.spawn(async move |this, cx| loop {
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
        }));
    }

    pub fn emit_biometric_signal(
        &mut self,
        signal: ContinuousBiometricSignal,
        cx: &mut Context<Self>,
    ) {
        self.handle_continuous_signal(signal, cx);
    }

    fn handle_continuous_signal(
        &mut self,
        signal: ContinuousBiometricSignal,
        cx: &mut Context<Self>,
    ) {
        if !self.state.auth.can_view(Page::Identificacion)
            || !allowed(&self.state, "biometrics.identify")
        {
            return;
        }
        if !self.preferences.biometric_identification_enabled {
            self.pending_biometric_notice = Some((
                true,
                "La identificación biométrica simulada está desactivada en Configuración.".into(),
            ));
            cx.notify();
            return;
        }
        if self.biometric_modal_open {
            // Bloquea nuevas identificaciones mientras exista una atención abierta.
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
                // Evita identificaciones repetidas mientras el modal esté abierto o el dedo siga colocado.
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
        let resolver = self.biometric_gateway.signal_resolver();
        self.biometric_op_id = self.biometric_op_id.wrapping_add(1);
        let op_id = self.biometric_op_id;
        cx.notify();

        let bg_task = cx.background_spawn(async move {
            std::thread::sleep(Duration::from_millis(240));
            resolver.resolve(&candidates, &signal)
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
                        if app.biometric_modal_open {
                            return;
                        }
                        if allowed(&app.state, "consumptions.create") {
                            app.biometric_modal_open = true;
                            app.biometric_gateway.set_modal_locked(true);
                            app.pending_auto_modal_pensioner = Some(pensioner_id);
                        } else {
                            let evt_r = app.biometric_gateway.release_finger();
                            app.push_biometric_event(evt_r);
                            app.pending_biometric_notice = Some((
                                false,
                                format!("Pensionista #{pensioner_id} reconocido.").into(),
                            ));
                        }
                    }
                    Ok(IdentificationOutcome::NoMatch) => {
                        app.pending_biometric_notice = Some((
                            true,
                            "Huella no reconocida. El lector volverá a estado de espera.".into(),
                        ));
                        let reset_delay = cx.background_spawn(async {
                            std::thread::sleep(Duration::from_millis(2500));
                        });
                        cx.spawn(async move |this, cx| {
                            reset_delay.await;
                            let _ = this.update(cx, |app, cx| {
                                if app.biometric_op_id == op_id
                                    && !app.biometric_modal_open
                                    && matches!(
                                        app.biometric_gateway.state(),
                                        BiometricReaderState::Unrecognized
                                    )
                                {
                                    if let Ok(wait_evt) = app.biometric_gateway.start_waiting() {
                                        app.push_biometric_event(wait_evt);
                                        cx.notify();
                                    }
                                }
                            });
                        })
                        .detach();
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
        if self.preferences.biometric_identification_enabled {
            let events = self.biometric_gateway.resume_waiting();
            for evt in events {
                self.push_biometric_event(evt);
            }
        } else {
            self.biometric_gateway.set_modal_locked(false);
            if self.biometric_gateway.is_finger_present() {
                let evt = self.biometric_gateway.release_finger();
                self.push_biometric_event(evt);
            }
        }
        self.pending_global_search_clear = true;
        if std::mem::take(&mut self.global_search_opened_modal) {
            self.pending_global_search_focus = true;
        }
        cx.notify();
    }

    pub fn biometric_release_finger(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.biometric_modal_open || !self.biometric_gateway.is_finger_present() {
            return;
        }
        let evt = self.biometric_gateway.release_finger();
        self.push_biometric_event(evt);
        if self.biometric_gateway.state().is_connected()
            && !matches!(
                self.biometric_gateway.state(),
                BiometricReaderState::WaitingFingerprint
            )
        {
            if let Ok(wait_evt) = self.biometric_gateway.start_waiting() {
                self.push_biometric_event(wait_evt);
            }
        }
        window.push_notification(
            Notification::info("Dedo retirado del sensor. Lector en espera."),
            cx,
        );
        cx.notify();
    }

    /// Abre el modal de consumo desde búsqueda manual, respetando el bloqueo único de modales
    /// y suspendiendo temporalmente nuevas identificaciones biométricas mientras esté abierto.
    pub fn open_manual_consumption_modal(
        &mut self,
        pensioner_id: u32,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        if !allowed(&self.state, "consumptions.create") {
            window.push_notification(
                Notification::error("No tienes permiso para registrar consumos."),
                cx,
            );
            return false;
        }
        if self.biometric_modal_open || window.has_active_dialog(cx) {
            window.push_notification(
                Notification::warning("Cierra el diálogo actual antes de iniciar otra atención."),
                cx,
            );
            self.pending_global_search_clear = true;
            cx.notify();
            return false;
        }
        let active_name = match &self.state.data {
            DataStatus::Ready(data) => data
                .pensioners
                .iter()
                .find(|p| p.id == pensioner_id)
                .map(|p| (p.active, p.name())),
            _ => None,
        };
        let Some((active, name)) = active_name else {
            window.push_notification(
                Notification::error("No se encontró el pensionista seleccionado."),
                cx,
            );
            self.pending_global_search_clear = true;
            cx.notify();
            return false;
        };
        if !active {
            window.push_notification(
                Notification::warning(format!(
                    "El pensionista {name} (#{pensioner_id}) está inactivo y no puede registrar consumos."
                )),
                cx,
            );
            self.pending_global_search_clear = true;
            cx.notify();
            return false;
        }

        self.biometric_task = None;
        self.biometric_op_id = self.biometric_op_id.wrapping_add(1);
        self.biometric_modal_open = true;
        self.biometric_gateway.set_modal_locked(true);
        self.open_auto_consumption_modal(pensioner_id, window, cx);
        true
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
        if window.has_active_dialog(cx) || !allowed(&self.state, "consumptions.create") {
            window.push_notification(Notification::warning("No se puede iniciar la atención mientras exista otro diálogo o falten permisos."), cx);
            self.resume_waiting_after_modal(cx);
            return;
        }
        let Some((person_name, bio_id, active, balance)) = (match &self.state.data {
            DataStatus::Ready(data) => {
                data.pensioners
                    .iter()
                    .find(|p| p.id == pensioner_id)
                    .map(|p| {
                        (
                            p.name(),
                            p.biometric_id.clone().unwrap_or_else(|| "—".into()),
                            p.active,
                            data.balance(p.id),
                        )
                    })
            }
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
        let initial_meal = MealSchedule::from_preferences(&self.preferences).current_meal();
        let initial_meal_idx = match initial_meal {
            Meal::Breakfast => 0,
            Meal::Lunch => 1,
        };
        let meal_select = cx.new(|cx| {
            SelectState::new(
                vec!["Desayuno", "Almuerzo"],
                Some(IndexPath::new(initial_meal_idx)),
                window,
                cx,
            )
        });
        let special_price_input = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder("12.00")
                .default_value(format_decimal(pricing.special_default_cents))
        });
        let third_party_input =
            cx.new(|cx| InputState::new(window, cx).placeholder("Nombre de quien recoge"));

        let initial_token = self.consumption_confirm_token;
        self.consumption_confirm_token = self.consumption_confirm_token.wrapping_add(1);

        let modal_state = cx.new(|_| AutoModalState {
            meal: initial_meal,
            menu_kind: MenuKind::Economic,
            portions: 1,
            third_party: false,
            cart: Vec::new(),
            next_item_id: 1,
            error: None,
            submitting: false,
            token: initial_token,
            _meal_subscription: None,
        });

        let modal_for_meal = modal_state.clone();
        let meal_sub = cx.subscribe(
            &meal_select,
            move |_this, _select, event: &SelectEvent<Vec<&'static str>>, cx| {
                if let SelectEvent::Confirm(Some(value)) = event {
                    let selected_meal = if *value == "Desayuno" {
                        Meal::Breakfast
                    } else {
                        Meal::Lunch
                    };
                    modal_for_meal.update(cx, |s, cx| {
                        s.meal = selected_meal;
                        s.error = None;
                        cx.notify();
                    });
                }
            },
        );
        modal_state.update(cx, |s, _| {
            s._meal_subscription = Some(meal_sub);
        });

        self.pending_global_search_clear = true;
        let app_weak = cx.entity().downgrade();
        let app_on_cancel = app_weak.clone();

        window.open_dialog(cx, move |dialog, _, cx| {
            let st = modal_state.read(cx);
            let meal = st.meal;
            let menu_kind = st.menu_kind;
            let portions = st.portions;
            let third_party = st.third_party;
            let error_msg = st.error.clone();
            let submitting = st.submitting;
            let econ_unit_cents = pricing.economic_for(meal);
            let cart_items = st.cart.clone();
            let total_portions: u32 = cart_items.iter().map(|i| i.portions).sum();
            let total_cents: i64 = cart_items.iter().map(|i| i.subtotal_cents()).sum();

            let meal_select_view = meal_select.clone();
            let modal_for_econ = modal_state.clone();
            let modal_for_spec = modal_state.clone();
            let modal_for_dec = modal_state.clone();
            let modal_for_inc = modal_state.clone();
            let modal_for_add = modal_state.clone();
            let modal_for_switch = modal_state.clone();
            let modal_for_submit = modal_state.clone();

            let sp_price_add = special_price_input.clone();
            let third_input_submit = third_party_input.clone();
            let app_submit = app_weak.clone();
            let app_cancel_btn = app_weak.clone();
            let app_close_cb = app_on_cancel.clone();

            dialog
                .width(px(560.))
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
                                                .child("Estado: Activo · Atención de demostración"),
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
                                .items_center()
                                .flex_wrap()
                                .child(
                                    div()
                                        .w(px(140.))
                                        .child(Select::new(&meal_select_view).xsmall()),
                                )
                                .child(
                                    Button::new("auto-menu-econ")
                                        .xsmall()
                                        .when(menu_kind == MenuKind::Economic, |b| b.primary())
                                        .when(menu_kind != MenuKind::Economic, |b| b.outline())
                                        .label(format!("Económico ({})", amount(econ_unit_cents)))
                                        .on_click(move |_, _, cx| {
                                            modal_for_econ.update(cx, |s, cx| {
                                                s.menu_kind = MenuKind::Economic;
                                                s.error = None;
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
                                                s.error = None;
                                                cx.notify();
                                            });
                                        }),
                                ),
                        )
                        .child(
                            h_flex()
                                .justify_between()
                                .gap_2()
                                .p_2p5()
                                .rounded(cx.theme().radius)
                                .border_1()
                                .border_color(cx.theme().border)
                                .child(
                                    h_flex()
                                        .gap_2()
                                        .when(menu_kind == MenuKind::Special, |this| {
                                            this.child(
                                                div()
                                                    .w(px(110.))
                                                    .child(Input::new(&special_price_input)),
                                            )
                                        })
                                        .when(menu_kind == MenuKind::Economic, |this| {
                                            this.child(
                                                div()
                                                    .text_xs()
                                                    .text_color(cx.theme().muted_foreground)
                                                    .child(format!(
                                                        "Precio: {}",
                                                        amount(econ_unit_cents)
                                                    )),
                                            )
                                        })
                                        .child(
                                            Button::new("auto-dec")
                                                .outline()
                                                .xsmall()
                                                .disabled(portions <= 1)
                                                .label("-")
                                                .on_click(move |_, _, cx| {
                                                    modal_for_dec.update(cx, |s, cx| {
                                                        s.portions =
                                                            s.portions.saturating_sub(1).max(1);
                                                        s.error = None;
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
                                                        s.error = None;
                                                        cx.notify();
                                                    });
                                                }),
                                        ),
                                )
                                .child(
                                    Button::new("auto-add-cart")
                                        .outline()
                                        .xsmall()
                                        .icon(LucideIcon::Plus)
                                        .label(if menu_kind == MenuKind::Economic {
                                            "Agregar económico"
                                        } else {
                                            "Agregar especial"
                                        })
                                        .on_click(move |_, _, cx| {
                                            let raw_price = sp_price_add.read(cx).value().to_string();
                                            modal_for_add.update(cx, |s, cx| {
                                                let unit_cents = match s.menu_kind {
                                                    MenuKind::Economic => {
                                                        pricing.economic_for(s.meal)
                                                    }
                                                    MenuKind::Special => {
                                                        match parse_price_cents(&raw_price) {
                                                            Ok(c) => c,
                                                            Err(err) => {
                                                                s.error = Some(err);
                                                                cx.notify();
                                                                return;
                                                            }
                                                        }
                                                    }
                                                };
                                                let mut next_id = s.next_item_id;
                                                if let Err(err) = add_to_mixed_cart(
                                                    &mut s.cart,
                                                    &mut next_id,
                                                    s.meal,
                                                    s.menu_kind,
                                                    s.portions,
                                                    unit_cents,
                                                ) {
                                                    s.error = Some(err);
                                                    cx.notify();
                                                    return;
                                                }
                                                s.next_item_id = next_id;
                                                s.portions = 1;
                                                s.error = None;
                                                cx.notify();
                                            });
                                        }),
                                ),
                        )
                        .child(if cart_items.is_empty() {
                            div()
                                .p_3()
                                .rounded(cx.theme().radius)
                                .border_1()
                                .border_color(cx.theme().border)
                                .text_xs()
                                .text_color(cx.theme().muted_foreground)
                                .child("Carrito vacío. Agrega consumos económicos o especiales para esta atención.")
                                .into_any_element()
                        } else {
                            v_flex()
                                .gap_1p5()
                                .children(cart_items.into_iter().map(|item| {
                                    let item_id = item.id;
                                    let modal_dec = modal_state.clone();
                                    let modal_inc = modal_state.clone();
                                    let modal_rm = modal_state.clone();
                                    h_flex()
                                        .justify_between()
                                        .px_2p5()
                                        .py_1p5()
                                        .rounded(cx.theme().radius)
                                        .border_1()
                                        .border_color(cx.theme().border)
                                        .child(
                                            h_flex()
                                                .gap_1p5()
                                                .child(
                                                    div().text_xs().font_weight(FontWeight::MEDIUM).child(
                                                        format!(
                                                            "{} · {} ({})",
                                                            item.meal.label(),
                                                            item.menu_kind.label(),
                                                            amount(item.unit_price_cents)
                                                        ),
                                                    ),
                                                ),
                                        )
                                        .child(
                                            h_flex()
                                                .gap_1p5()
                                                .child(
                                                    Button::new(format!("cart-dec-{item_id}"))
                                                        .ghost()
                                                        .xsmall()
                                                        .label("-")
                                                        .on_click(move |_, _, cx| {
                                                            modal_dec.update(cx, |s, cx| {
                                                                if let Some(pos) = s
                                                                    .cart
                                                                    .iter()
                                                                    .position(|i| i.id == item_id)
                                                                {
                                                                    if s.cart[pos].portions <= 1 {
                                                                        s.cart.remove(pos);
                                                                    } else {
                                                                        s.cart[pos].portions -= 1;
                                                                    }
                                                                }
                                                                cx.notify();
                                                            });
                                                        }),
                                                )
                                                .child(
                                                    div()
                                                        .w(px(20.))
                                                        .text_center()
                                                        .text_xs()
                                                        .font_weight(FontWeight::BOLD)
                                                        .child(item.portions.to_string()),
                                                )
                                                .child(
                                                    Button::new(format!("cart-inc-{item_id}"))
                                                        .ghost()
                                                        .xsmall()
                                                        .label("+")
                                                        .on_click(move |_, _, cx| {
                                                            modal_inc.update(cx, |s, cx| {
                                                                if let Some(i) = s
                                                                    .cart
                                                                    .iter_mut()
                                                                    .find(|i| i.id == item_id)
                                                                {
                                                                    i.portions =
                                                                        i.portions.saturating_add(1);
                                                                }
                                                                cx.notify();
                                                            });
                                                        }),
                                                )
                                                .child(
                                                    div()
                                                        .w(px(72.))
                                                        .text_right()
                                                        .text_xs()
                                                        .font_weight(FontWeight::SEMIBOLD)
                                                        .child(amount(item.subtotal_cents())),
                                                )
                                                .child(
                                                    Button::new(format!("cart-rm-{item_id}"))
                                                        .ghost()
                                                        .xsmall()
                                                        .icon(LucideIcon::X)
                                                        .tooltip("Eliminar")
                                                        .on_click(move |_, _, cx| {
                                                            modal_rm.update(cx, |s, cx| {
                                                                s.cart.retain(|i| i.id != item_id);
                                                                cx.notify();
                                                            });
                                                        }),
                                                ),
                                        )
                                }))
                                .into_any_element()
                        })
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
                                                s.error = None;
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
                                .disabled(submitting || total_portions == 0)
                                .label("Confirmar atención")
                                .on_click(move |_, window, cx| {
                                    let pickup_name =
                                        third_input_submit.read(cx).value().trim().to_string();
                                    let (already_submitting, items, is_third, token) = {
                                        let s = modal_for_submit.read(cx);
                                        (s.submitting, s.cart.clone(), s.third_party, s.token)
                                    };
                                    if already_submitting {
                                        return;
                                    }
                                    if items.is_empty() {
                                        modal_for_submit.update(cx, |s, cx| {
                                            s.error =
                                                Some("Agrega al menos un consumo al carrito.".into());
                                            cx.notify();
                                        });
                                        return;
                                    }
                                    if is_third && pickup_name.chars().count() < 2 {
                                        modal_for_submit.update(cx, |s, cx| {
                                            s.error =
                                                Some("Ingresa el nombre de quien recoge.".into());
                                            cx.notify();
                                        });
                                        return;
                                    }
                                    let pickup = is_third.then_some(pickup_name);

                                    modal_for_submit.update(cx, |s, _| s.submitting = true);

                                    let res = app_submit.update(cx, |app, cx| {
                                        if !allowed(&app.state, "consumptions.create") {
                                            return Err("No tienes permiso para registrar consumos.".to_string());
                                        }
                                        let now = chrono::Local::now();
                                        let date = now.date_naive();
                                        let time = now.format("%H:%M").to_string();
                                        let out = match &mut app.state.data {
                                            DataStatus::Ready(data) => data.record_consumptions(
                                                date,
                                                &time,
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
    finger_present: bool,
    modal_open: bool,
    cx: &Context<DesktopApp>,
    app: &App,
) -> impl IntoElement {
    let theme = app.theme();
    let is_connected = reader_state.is_connected();
    let is_identifying = matches!(reader_state, BiometricReaderState::Identifying);
    let can_identify = allowed(state, "biometrics.identify");
    let sim_disabled = !is_connected || is_identifying || modal_open || !can_identify;
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
                        .flex_wrap()
                        .child(state_tag(reader_state))
                        .when(finger_present, |this| {
                            this.child(Tag::info().outline().small().child("Dedo colocado"))
                        })
                        .when(modal_open, |this| {
                            this.child(Tag::warning().small().child("Atención en curso"))
                        })
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
                                .when(modal_open, |this| {
                                    this.child(Alert::warning(
                                        "bio-modal-lock-alert",
                                        "Atención abierta: nuevas identificaciones bloqueadas hasta confirmar o cancelar.",
                                    ))
                                })
                                .child(
                                    h_flex()
                                        .gap_2()
                                        .flex_wrap()
                                        .justify_center()
                                        .child(
                                            Button::new("bio-sim-match")
                                                .primary()
                                                .small()
                                                .disabled(sim_disabled)
                                                .icon(LucideIcon::FingerprintPattern)
                                                .label("Simular huella reconocida")
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
                                                .disabled(sim_disabled)
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
                                                .disabled(sim_disabled)
                                                .label("Simular error")
                                                .on_click(cx.listener(|this, _, window, cx| {
                                                    this.biometric_simulate(
                                                        SimulationMode::DeviceError,
                                                        window,
                                                        cx,
                                                    );
                                                })),
                                        )
                                        .when(finger_present && !modal_open, |this| {
                                            this.child(
                                                Button::new("bio-release-finger")
                                                    .outline()
                                                    .small()
                                                    .label("Retirar dedo")
                                                    .on_click(cx.listener(|this, _, window, cx| {
                                                        this.biometric_release_finger(window, cx);
                                                    })),
                                            )
                                        })
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
                                                            this.open_manual_consumption_modal(pid, window, cx);
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn schedule_selects_breakfast_or_lunch_by_hour() {
        let schedule = MealSchedule::default();
        assert_eq!(schedule.meal_for_hour(8), Meal::Breakfast);
        assert_eq!(schedule.meal_for_hour(10), Meal::Breakfast);
        assert_eq!(schedule.meal_for_hour(11), Meal::Lunch);
        assert_eq!(schedule.meal_for_hour(14), Meal::Lunch);
    }

    #[test]
    fn mixed_cart_groups_same_type_and_price_and_separates_different_specials() {
        let mut cart = Vec::new();
        let mut next_id = 1;

        // 2 almuerzos económicos + 1 almuerzo económico -> agrupa en 3
        add_to_mixed_cart(
            &mut cart,
            &mut next_id,
            Meal::Lunch,
            MenuKind::Economic,
            2,
            700,
        )
        .unwrap();
        add_to_mixed_cart(
            &mut cart,
            &mut next_id,
            Meal::Lunch,
            MenuKind::Economic,
            1,
            700,
        )
        .unwrap();

        // Especial S/ 12 + Especial S/ 15 -> quedan separados; otro Especial S/ 12 -> agrupa
        add_to_mixed_cart(
            &mut cart,
            &mut next_id,
            Meal::Lunch,
            MenuKind::Special,
            1,
            1200,
        )
        .unwrap();
        add_to_mixed_cart(
            &mut cart,
            &mut next_id,
            Meal::Lunch,
            MenuKind::Special,
            1,
            1500,
        )
        .unwrap();
        add_to_mixed_cart(
            &mut cart,
            &mut next_id,
            Meal::Lunch,
            MenuKind::Special,
            2,
            1200,
        )
        .unwrap();

        assert_eq!(cart.len(), 3);
        assert_eq!(cart[0].portions, 3);
        assert_eq!(cart[0].subtotal_cents(), 2100);
        assert_eq!(cart[1].portions, 3);
        assert_eq!(cart[1].unit_price_cents, 1200);
        assert_eq!(cart[2].portions, 1);
        assert_eq!(cart[2].unit_price_cents, 1500);

        let total: i64 = cart.iter().map(|i| i.subtotal_cents()).sum();
        assert_eq!(total, 2100 + 3600 + 1500);

        assert!(add_to_mixed_cart(
            &mut cart,
            &mut next_id,
            Meal::Lunch,
            MenuKind::Special,
            0,
            1200
        )
        .is_err());
        assert!(add_to_mixed_cart(
            &mut cart,
            &mut next_id,
            Meal::Lunch,
            MenuKind::Special,
            1,
            0
        )
        .is_err());
    }

    #[test]
    fn global_combobox_choices_exclude_inactive_and_match_normalized_fields() {
        let mut data = MockData::demo(chrono::Local::now().date_naive());
        let active_id = data
            .save_pensioner(None, "Lucía", "Gómez Ávila", "955123456")
            .unwrap();
        let inactive_id = data
            .save_pensioner(None, "Pedro", "Inactivo", "944000111")
            .unwrap();
        data.pensioners
            .iter_mut()
            .find(|p| p.id == inactive_id)
            .unwrap()
            .active = false;

        let choices = build_global_pensioner_choices(&data);
        assert!(choices.iter().any(|c| c.id == active_id));
        assert!(!choices.iter().any(|c| c.id == inactive_id));

        let item = choices.iter().find(|c| c.id == active_id).unwrap();
        assert!(item.matches("lucia avila"));
        assert!(item.matches(&format!("#{active_id}")));
        assert!(item.matches("955123"));
        assert!(!item.matches("inexistente"));
        assert!(item.matches("luci\u{301}a go\u{301}mez"));
    }
}
