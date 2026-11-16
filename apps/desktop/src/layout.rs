use gpui_kit::{
    AppContext as _, Context, Entity, FocusHandle, Focusable as _, InteractiveElement as _,
    IntoElement, ParentElement, Render, SharedString, Styled, Subscription, Task, Window, actions,
    component::{
        ActiveTheme as _, Disableable as _, IconName, Side, Sizable as _, ThemeMode, TitleBar,
        button::{Button, ButtonVariants as _},
        combobox::{Combobox, ComboboxEvent, ComboboxState},
        searchable_list::SearchableVec,
        h_flex,
        menu::{AppMenuBar, DropdownMenu as _, PopupMenuItem},
        input::{InputEvent, InputState},
        scroll::ScrollableElement as _,
        sidebar::SidebarToggleButton,
        table::TableState,
        tag::Tag,
        v_flex,
        WindowExt as _,
        notification::Notification,
    },
    px,
    prelude::FluentBuilder as _,
};
use gpui_kit::assets::IconName as LucideIcon;

use crate::{
    auth::{AuthError, AuthService, AuthStatus, MockAuthService},
    biometric_service::{BiometricGateway, SimulatedBiometricGateway},
    components::{common, sidebar},
    login, mock_data, pages::{self, identification::{GlobalPensionerChoice, build_global_pensioner_choices}}, services,
    settings_store::{
        AppPreferences, BiometricBackupStatus, OsCredentialVaultPlaceholder,
        ServerConnectionStatus, ThemePreference, default_preferences_path, format_time_minutes,
        load_preferences_from,
    },
    state::{AppState, DataStatus, Page, ServiceStatus},
    theme,
};

actions!(
    desktop,
    [
        GoDashboard,
        GoSettings,
        ToggleSidebar,
        ToggleTheme,
        RunDiagnostics,
        ShowAbout,
        QuitApp,
        FocusGlobalSearch,
    ]
);

/// Vista raíz de la aplicación de escritorio.
pub struct DesktopApp {
    pub(crate) state: AppState,
    menu_bar: Entity<AppMenuBar>,
    focus_handle: FocusHandle,
    compact: bool,
    sidebar_before_compact: bool,
    pub(crate) follow_system_theme: bool,
    _appearance_subscription: Subscription,
    pub username: Entity<InputState>,
    pub password: Entity<InputState>,
    denied: bool,
    pub(crate) pensioner_search: Entity<InputState>,
    pub(crate) pensioner_filter: crate::pages::pensioners::Filter,
    pub(crate) selected_pensioner: Option<u32>,
    pub(crate) revealed_pensioner_balances: std::collections::HashSet<(u32, bool)>,
    pub(crate) pensioners_table: Entity<TableState<crate::pages::pensioners::PensionersTableDelegate>>,
    _search_subscription: Subscription,
    pub(crate) biometric_gateway: Box<dyn BiometricGateway>,
    pub(crate) biometric_event_log: Vec<SharedString>,
    pub(crate) biometric_task: Option<Task<()>>,
    pub(crate) biometric_op_id: u64,
    pub(crate) biometric_signal_tx: Option<std::sync::mpsc::Sender<crate::biometric_service::ContinuousBiometricSignal>>,
    pub(crate) biometric_listener_task: Option<Task<()>>,
    pub(crate) biometric_modal_open: bool,
    pub(crate) pending_auto_modal_pensioner: Option<u32>,
    pub(crate) pending_biometric_notice: Option<(bool, SharedString)>,
    pub(crate) global_pensioner_combobox: Entity<ComboboxState<SearchableVec<GlobalPensionerChoice>>>,
    pub(crate) global_search_last_items: Vec<GlobalPensionerChoice>,
    pub(crate) pending_global_search_clear: bool,
    pub(crate) pending_global_search_focus: bool,
    pub(crate) global_search_opened_modal: bool,
    _global_search_subscription: Subscription,
    pub(crate) consumption_pricing: mock_data::ConsumptionPricing,
    pub(crate) consumption_confirm_token: u64,
    pub(crate) consumption_processed_tokens: std::collections::HashSet<u64>,
    pub(crate) cash_search: Entity<InputState>,
    pub(crate) cash_amount_input: Entity<InputState>,
    pub(crate) cash_filter: crate::pages::cash::CashFilter,
    pub(crate) cash_op_kind: crate::pages::cash::CashOpKind,
    pub(crate) cash_selected_pensioner: Option<u32>,
    pub(crate) cash_ticket_count: u32,
    pub(crate) cash_confirm_token: u64,
    pub(crate) cash_processed_tokens: std::collections::HashSet<u64>,
    pub(crate) cash_submitting: bool,
    _cash_search_subscription: Subscription,
    pub(crate) report_tab: crate::pages::reports::ReportTab,
    pub(crate) report_search: Entity<InputState>,
    pub(crate) report_date_preset: crate::reports_service::DateRangePreset,
    pub(crate) report_type_filter: crate::reports_service::ConsumptionTypeFilter,
    pub(crate) report_status_filter: crate::reports_service::PensionerStatusFilter,
    pub(crate) report_only_third_party: bool,
    pub(crate) report_pensioner_id: Option<u32>,
    pub(crate) report_sort_dir: crate::reports_service::SortDirection,
    pub(crate) report_consumption_sort: crate::reports_service::ConsumptionSortColumn,
    pub(crate) report_payment_sort: crate::reports_service::PaymentSortColumn,
    pub(crate) report_debt_sort: crate::reports_service::DebtSortColumn,
    pub(crate) report_page: usize,
    _report_search_subscription: Subscription,
    pub(crate) preferences: AppPreferences,
    pub(crate) server_status: ServerConnectionStatus,
    pub(crate) server_test_task: Option<Task<()>>,
    pub(crate) biometric_backup_status: BiometricBackupStatus,
    pub(crate) credential_vault: OsCredentialVaultPlaceholder,
    pub(crate) cfg_dining_hall_input: Entity<InputState>,
    pub(crate) cfg_breakfast_input: Entity<InputState>,
    pub(crate) cfg_lunch_input: Entity<InputState>,
    pub(crate) cfg_special_input: Entity<InputState>,
    pub(crate) cfg_ticket_input: Entity<InputState>,
    pub(crate) cfg_breakfast_start_input: Entity<InputState>,
    pub(crate) cfg_breakfast_end_input: Entity<InputState>,
    pub(crate) cfg_lunch_start_input: Entity<InputState>,
    pub(crate) cfg_lunch_end_input: Entity<InputState>,
    pub(crate) cfg_server_url_input: Entity<InputState>,
    pub(crate) cfg_station_id_input: Entity<InputState>,
}

impl DesktopApp {
    pub fn focus_global_search(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !self.state.auth.user().is_some_and(|u| u.can("consumptions.create"))
            || self.biometric_modal_open
            || window.has_active_dialog(cx)
        {
            return;
        }
        self.global_pensioner_combobox.update(cx, |cb, cx| cb.focus(window, cx));
        let focus = self.global_pensioner_combobox.read(cx).focus_handle(cx);
        if !focus.is_focused(window) {
            focus.focus(window, cx);
            return;
        }
        window.defer(cx, move |window, cx| {
            if focus.is_focused(window) && !window.has_active_dialog(cx) {
                window.dispatch_action(Box::new(gpui_kit::base::actions::SelectDown), cx);
            }
        });
    }
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let menu_bar = AppMenuBar::new(cx);
        let focus_handle = cx.focus_handle();
        let username = cx.new(|cx| InputState::new(window, cx).placeholder("Usuario"));
        let password = cx.new(|cx| {
            let mut input = InputState::new(window, cx).placeholder("Contraseña");
            input.set_masked(true, window, cx);
            input
        });
        let pensioner_search = cx.new(|cx| InputState::new(window, cx).placeholder("Buscar por nombre o celular"));
        let search_subscription = cx.subscribe_in(&pensioner_search, window, |_, _, event, _, cx| {
            if matches!(event, InputEvent::Change) { cx.notify(); }
        });
        let global_pensioner_combobox = cx.new(|cx| {
            ComboboxState::new(SearchableVec::<GlobalPensionerChoice>::new(vec![]), vec![], window, cx)
                .multiple(false)
                .searchable(true)
        });
        let global_search_sub = cx.subscribe_in(
            &global_pensioner_combobox,
            window,
            |this, cb, event: &ComboboxEvent<SearchableVec<GlobalPensionerChoice>>, window, cx| {
                if let ComboboxEvent::Confirm(values) = event {
                    this.pending_global_search_clear = true;
                    if let Some(&pensioner_id) = values.first() {
                        if this.open_manual_consumption_modal(pensioner_id, window, cx) {
                            this.global_search_opened_modal = true;
                        }
                    } else if !cb.read(cx).query(cx).is_empty() {
                        cx.notify();
                    }
                }
            },
        );
        let preferences = load_preferences_from(&default_preferences_path()).unwrap_or_default();
        let consumption_pricing = mock_data::ConsumptionPricing {
            economic_breakfast_cents: preferences.economic_breakfast_cents,
            economic_lunch_cents: preferences.economic_lunch_cents,
            special_default_cents: preferences.special_default_cents,
        };
        let cash_search = cx.new(|cx| InputState::new(window, cx).placeholder("Buscar pensionista por nombre o celular"));
        let cash_search_sub = cx.subscribe_in(&cash_search, window, |_, _, event, _, cx| {
            if matches!(event, InputEvent::Change) { cx.notify(); }
        });
        let cash_amount_input = cx.new(|cx| InputState::new(window, cx).placeholder("0.00"));
        let report_search = cx.new(|cx| InputState::new(window, cx).placeholder("Buscar por pensionista, plato o MOV"));
        let report_search_sub = cx.subscribe_in(&report_search, window, |this, _, event, _, cx| {
            if matches!(event, InputEvent::Change) {
                this.report_page = 0;
                cx.notify();
            }
        });
        let cfg_dining_hall_input = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder("Nombre del comedor")
                .default_value(preferences.dining_hall_name.clone())
        });
        let cfg_breakfast_input = cx.new(|cx| {
            InputState::new(window, cx).default_value(format!(
                "{}.{:02}",
                preferences.economic_breakfast_cents / 100,
                preferences.economic_breakfast_cents.abs() % 100
            ))
        });
        let cfg_lunch_input = cx.new(|cx| {
            InputState::new(window, cx).default_value(format!(
                "{}.{:02}",
                preferences.economic_lunch_cents / 100,
                preferences.economic_lunch_cents.abs() % 100
            ))
        });
        let cfg_special_input = cx.new(|cx| {
            InputState::new(window, cx).default_value(format!(
                "{}.{:02}",
                preferences.special_default_cents / 100,
                preferences.special_default_cents.abs() % 100
            ))
        });
        let cfg_ticket_input = cx.new(|cx| {
            InputState::new(window, cx).default_value(format!(
                "{}.{:02}",
                preferences.ticket_value_cents / 100,
                preferences.ticket_value_cents.abs() % 100
            ))
        });
        let cfg_breakfast_start_input = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder("06:00")
                .default_value(format_time_minutes(preferences.breakfast_start_minutes))
        });
        let cfg_breakfast_end_input = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder("11:00")
                .default_value(format_time_minutes(preferences.breakfast_end_minutes))
        });
        let cfg_lunch_start_input = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder("11:00")
                .default_value(format_time_minutes(preferences.lunch_start_minutes))
        });
        let cfg_lunch_end_input = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder("16:00")
                .default_value(format_time_minutes(preferences.lunch_end_minutes))
        });
        let cfg_server_url_input = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder("http://127.0.0.1:8080")
                .default_value(preferences.server_url.clone())
        });
        let cfg_station_id_input = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder("CAJA-01")
                .default_value(preferences.station_id.clone())
        });
        let appearance_subscription = cx.observe_window_appearance(window, |this: &mut Self, window, cx| {
            if this.follow_system_theme {
                theme::apply_theme(window.appearance().into(), Some(window), cx);
                cx.notify();
            }
        });
        let mut initial_state = AppState::default();
        initial_state.sidebar_collapsed = preferences.sidebar_collapsed;
        let follow_system_theme = preferences.theme_preference == ThemePreference::System;
        let initial_theme_mode = match preferences.theme_preference {
            ThemePreference::System => window.appearance().into(),
            ThemePreference::Light => ThemeMode::Light,
            ThemePreference::Dark => ThemeMode::Dark,
        };
        let app_weak_for_table = cx.entity().downgrade();
        let pensioners_table = cx.new(|cx| {
            TableState::new(
                crate::pages::pensioners::PensionersTableDelegate::new(app_weak_for_table),
                window,
                cx,
            )
            .col_resizable(true)
            .col_movable(false)
            .row_selectable(true)
            .sortable(true)
        });
        let mut app = Self {
            state: initial_state,
            menu_bar,
            focus_handle,
            compact: false,
            sidebar_before_compact: false,
            follow_system_theme,
            _appearance_subscription: appearance_subscription,
            username,
            password,
            denied: false,
            pensioner_search,
            pensioner_filter: crate::pages::pensioners::Filter::All,
            selected_pensioner: None,
            revealed_pensioner_balances: std::collections::HashSet::new(),
            pensioners_table,
            _search_subscription: search_subscription,
            biometric_gateway: Box::new(SimulatedBiometricGateway::new()),
            biometric_event_log: Vec::new(),
            biometric_task: None,
            biometric_op_id: 0,
            biometric_signal_tx: None,
            biometric_listener_task: None,
            biometric_modal_open: false,
            pending_auto_modal_pensioner: None,
            pending_biometric_notice: None,
            global_pensioner_combobox,
            global_search_last_items: Vec::new(),
            pending_global_search_clear: false,
            pending_global_search_focus: false,
            global_search_opened_modal: false,
            _global_search_subscription: global_search_sub,
            consumption_pricing,
            consumption_confirm_token: 1,
            consumption_processed_tokens: std::collections::HashSet::new(),
            cash_search,
            cash_amount_input,
            cash_filter: crate::pages::cash::CashFilter::WithDebt,
            cash_op_kind: crate::pages::cash::CashOpKind::Payment,
            cash_selected_pensioner: None,
            cash_ticket_count: 1,
            cash_confirm_token: 1,
            cash_processed_tokens: std::collections::HashSet::new(),
            cash_submitting: false,
            _cash_search_subscription: cash_search_sub,
            report_tab: crate::pages::reports::ReportTab::Consumptions,
            report_search,
            report_date_preset: crate::reports_service::DateRangePreset::All,
            report_type_filter: crate::reports_service::ConsumptionTypeFilter::All,
            report_status_filter: crate::reports_service::PensionerStatusFilter::All,
            report_only_third_party: false,
            report_pensioner_id: None,
            report_sort_dir: crate::reports_service::SortDirection::Desc,
            report_consumption_sort: crate::reports_service::ConsumptionSortColumn::Date,
            report_payment_sort: crate::reports_service::PaymentSortColumn::Date,
            report_debt_sort: crate::reports_service::DebtSortColumn::Balance,
            report_page: 0,
            _report_search_subscription: report_search_sub,
            preferences,
            server_status: ServerConnectionStatus::Untested,
            server_test_task: None,
            biometric_backup_status: BiometricBackupStatus::Pending,
            credential_vault: OsCredentialVaultPlaceholder,
            cfg_dining_hall_input,
            cfg_breakfast_input,
            cfg_lunch_input,
            cfg_special_input,
            cfg_ticket_input,
            cfg_breakfast_start_input,
            cfg_breakfast_end_input,
            cfg_lunch_start_input,
            cfg_lunch_end_input,
            cfg_server_url_input,
            cfg_station_id_input,
        };
        theme::apply_theme(initial_theme_mode, Some(window), cx);
        app.start_diagnostics(false, cx);
        app.load_demo_data(cx);
        app
    }

    pub fn navigate_to(&mut self, page: Page, cx: &mut Context<Self>) {
        if self.state.auth.user().is_none() { return; }
        if !self.state.auth.can_view(page) { self.denied = true; cx.notify(); return; }
        if page != Page::Pensionistas { self.revealed_pensioner_balances.clear(); }
        self.denied = false;
        let was_selected = self.selected_pensioner.take().is_some();
        if self.state.push_page(page) || was_selected {
            cx.notify();
        }
    }

    pub fn navigate_back(&mut self, cx: &mut Context<Self>) {
        if self.state.auth.user().is_none() { return; }
        self.denied = false;
        if self.state.go_back() {
            if self.state.page != Page::Pensionistas { self.revealed_pensioner_balances.clear(); }
            cx.notify();
        }
    }

    pub fn navigate_forward(&mut self, cx: &mut Context<Self>) {
        if self.state.auth.user().is_none() { return; }
        self.denied = false;
        if self.state.go_forward() {
            if self.state.page != Page::Pensionistas { self.revealed_pensioner_balances.clear(); }
            cx.notify();
        }
    }

    pub fn toggle_sidebar(&mut self, cx: &mut Context<Self>) {
        if self.state.auth.user().is_none() { return; }
        self.state.sidebar_collapsed = !self.state.sidebar_collapsed;
        cx.notify();
    }

    pub fn sign_in(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if matches!(self.state.auth, AuthStatus::Authenticating) { return; }
        let username = self.username.read(cx).value().to_string();
        let password = self.password.read(cx).value().to_string();
        self.password.update(cx, |input, cx| input.set_value("", window, cx));
        self.state.auth = AuthStatus::Authenticating;
        cx.notify();
        let task = cx.background_spawn(async move { MockAuthService.login(&username, &password) });
        cx.spawn(async move |this, cx| {
            let result = task.await;
            let _ = this.update(cx, |this, cx| {
                this.state.auth = match result {
                    Ok(session) => {
                        this.state.user.name = session.user.name.clone().into();
                        this.state.user.role = session.user.role_label().into();
                        this.state.page = Page::Dashboard;
                        this.state.history = vec![Page::Dashboard];
                        this.state.history_index = 0;
                        let status = AuthStatus::Authenticated(session);
                        this.state.auth = status.clone();
                        this.activate_continuous_biometrics(cx);
                        status
                    },
                    Err(AuthError::Expired) => AuthStatus::SessionExpired,
                    Err(err) => AuthStatus::Error(err.message().into()),
                };
                cx.notify();
            });
        }).detach();
    }

    fn sign_out(&mut self, cx: &mut Context<Self>) {
        if let AuthStatus::Authenticated(session) = &self.state.auth { let _ = MockAuthService.logout(session); }
        self.biometric_task = None;
        self.biometric_op_id = self.biometric_op_id.wrapping_add(1);
        self.biometric_signal_tx = None;
        self.biometric_listener_task = None;
        self.biometric_modal_open = false;
        self.pending_auto_modal_pensioner = None;
        self.pending_biometric_notice = None;
        self.pending_global_search_clear = true;
        self.pending_global_search_focus = false;
        self.global_search_opened_modal = false;
        self.biometric_gateway.disconnect();
        self.biometric_event_log.clear();
        self.state.auth = AuthStatus::Unauthenticated;
        self.selected_pensioner = None;
        self.revealed_pensioner_balances.clear();
        self.state.user = Default::default();
        self.denied = false;
        self.state.page = Page::Dashboard;
        self.state.history = vec![Page::Dashboard];
        self.state.history_index = 0;
        cx.notify();
    }

    fn expire_session(&mut self, cx: &mut Context<Self>) {
        self.sign_out(cx);
        self.state.auth = AuthStatus::SessionExpired;
        cx.notify();
    }

    pub fn set_sidebar_collapsed(&mut self, collapsed: bool, cx: &mut Context<Self>) {
        if !self.state.auth.user().is_some_and(|u| u.can("settings.update")) { return; }
        if self.state.sidebar_collapsed != collapsed {
            self.state.sidebar_collapsed = collapsed;
            cx.notify();
        }
    }

    pub fn toggle_theme(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let next_mode = if cx.theme().is_dark() {
            ThemeMode::Light
        } else {
            ThemeMode::Dark
        };
        self.apply_theme_mode(next_mode, window, cx);
    }

    pub fn set_dark_mode(&mut self, dark: bool, window: &mut Window, cx: &mut Context<Self>) {
        let next_mode = if dark {
            ThemeMode::Dark
        } else {
            ThemeMode::Light
        };
        self.apply_theme_mode(next_mode, window, cx);
    }

    pub(crate) fn apply_theme_mode(&mut self, mode: ThemeMode, window: &mut Window, cx: &mut Context<Self>) {
        self.follow_system_theme = false;
        self.state.theme_mode = mode;
        theme::apply_theme(mode, Some(window), cx);
        cx.notify();
    }

    fn use_system_theme(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.follow_system_theme = true;
        theme::apply_theme(window.appearance().into(), Some(window), cx);
        cx.notify();
    }

    pub fn dismiss_error(&mut self, cx: &mut Context<Self>) {
        self.state.error_banner = None;
        cx.notify();
    }

    pub fn toggle_consumption_sort(
        &mut self,
        col: crate::reports_service::ConsumptionSortColumn,
        cx: &mut Context<Self>,
    ) {
        if self.report_consumption_sort == col {
            self.report_sort_dir = self.report_sort_dir.toggle();
        } else {
            self.report_consumption_sort = col;
            self.report_sort_dir = crate::reports_service::SortDirection::Desc;
        }
        self.report_page = 0;
        cx.notify();
    }

    pub fn toggle_payment_sort(
        &mut self,
        col: crate::reports_service::PaymentSortColumn,
        cx: &mut Context<Self>,
    ) {
        if self.report_payment_sort == col {
            self.report_sort_dir = self.report_sort_dir.toggle();
        } else {
            self.report_payment_sort = col;
            self.report_sort_dir = crate::reports_service::SortDirection::Desc;
        }
        self.report_page = 0;
        cx.notify();
    }

    pub fn toggle_debt_sort(
        &mut self,
        col: crate::reports_service::DebtSortColumn,
        cx: &mut Context<Self>,
    ) {
        if self.report_debt_sort == col {
            self.report_sort_dir = self.report_sort_dir.toggle();
        } else {
            self.report_debt_sort = col;
            self.report_sort_dir = crate::reports_service::SortDirection::Desc;
        }
        self.report_page = 0;
        cx.notify();
    }

    pub fn load_demo_data(&mut self, cx: &mut Context<Self>) {
        self.state.data = DataStatus::Loading;
        cx.notify();
        let task = cx.background_spawn(async move {
            // Punto de sustitución por un repositorio real; no bloquea la interfaz.
            services::load_demo_data()
        });
        cx.spawn(async move |this, cx| {
            let result = task.await;
            let _ = this.update(cx, |this, cx| {
                this.state.data = match result { Ok(data) => DataStatus::Ready(data), Err(error) => DataStatus::Error(error) };
                cx.notify();
            });
        }).detach();
    }

    pub fn change_dashboard_date(&mut self, offset: i64, cx: &mut Context<Self>) {
        if !self.state.auth.can_view(Page::Dashboard) { return; }
        if let Some(date) = self.state.dashboard_date.checked_add_signed(chrono::Duration::days(offset)) {
            self.state.dashboard_date = date;
            cx.notify();
        }
    }

    pub fn dashboard_today(&mut self, cx: &mut Context<Self>) {
        if self.state.auth.can_view(Page::Dashboard) { self.state.dashboard_date = mock_data::today(); cx.notify(); }
    }

    /// Lanza una verificación en segundo plano sin bloquear el hilo de interfaz.
    pub fn start_diagnostics(&mut self, simulate_error: bool, cx: &mut Context<Self>) {
        if self.state.auth.user().is_some() && !self.state.auth.user().is_some_and(|u| u.can("settings.update")) { return; }
        self.state.service_status = ServiceStatus::Loading;
        self.state.error_banner = None;
        cx.notify();

        let bg_task = cx.background_spawn(services::run_startup_diagnostics(simulate_error));
        self.state.diagnostics_task = Some(cx.spawn(async move |this, cx| {
            let result = bg_task.await;
            let _ = this.update(cx, |app, cx| {
                match result {
                    Ok(diag) => {
                        app.state.service_status = ServiceStatus::Ready(diag);
                        app.state.error_banner = None;
                    }
                    Err(err) => {
                        let msg = err.to_string();
                        app.state.service_status = ServiceStatus::Failed(msg.clone().into());
                        app.state.error_banner = Some(msg.into());
                    }
                }
                app.state.diagnostics_task = None;
                cx.notify();
            });
        }));
    }
}

impl Render for DesktopApp {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if self.state.page == Page::Pensionistas && self.selected_pensioner.is_none() {
            let query = self.pensioner_search.read(cx).value().to_string();
            self.pensioners_table.update(cx, |table, cx| {
                if table.delegate_mut().sync(
                    &self.state,
                    &query,
                    self.pensioner_filter,
                    self.biometric_modal_open,
                    &self.revealed_pensioner_balances,
                ) {
                    table.refresh(cx);
                }
            });
        }
        if let Some(pid) = self.pending_auto_modal_pensioner.take() {
            let entity = cx.entity().downgrade();
            window.defer(cx, move |window, cx| {
                let _ = entity.update(cx, |app, cx| {
                    app.open_auto_consumption_modal(pid, window, cx);
                });
            });
        }
        if let Some((is_warn, msg)) = self.pending_biometric_notice.take() {
            window.defer(cx, move |window, cx| {
                window.push_notification(
                    if is_warn {
                        Notification::warning(msg)
                    } else {
                        Notification::info(msg)
                    },
                    cx,
                );
            });
        }
        if self.pending_global_search_clear || self.pending_global_search_focus {
            let do_clear = std::mem::take(&mut self.pending_global_search_clear);
            let do_focus = std::mem::take(&mut self.pending_global_search_focus);
            let entity = cx.entity().downgrade();
            window.defer(cx, move |window, cx| {
                let _ = entity.update(cx, |app, cx| {
                    app.global_pensioner_combobox.update(cx, |cb, cx| {
                        if do_clear {
                            cb.clear_selection(cx);
                            cb.set_query("", window, cx);
                        }
                        if do_focus
                            && !window.has_active_dialog(cx)
                            && app.state.auth.user().is_some_and(|u| u.can("consumptions.create"))
                        {
                            cb.focus(window, cx);
                        }
                    });
                });
            });
        }
        if let DataStatus::Ready(data) = &self.state.data {
            let items = build_global_pensioner_choices(data);
            if items != self.global_search_last_items {
                self.global_search_last_items = items.clone();
                let entity = cx.entity().downgrade();
                window.defer(cx, move |window, cx| {
                    let _ = entity.update(cx, |app, cx| {
                        app.global_pensioner_combobox.update(cx, |cb, cx| {
                            let query = cb.query(cx);
                            cb.set_items(SearchableVec::new(items), window, cx);
                            cb.set_query(query, window, cx);
                        });
                    });
                });
            }
        }
        let compact = window.viewport_size().width < px(760.);
        if compact != self.compact {
            if compact {
                self.sidebar_before_compact = self.state.sidebar_collapsed;
                self.state.sidebar_collapsed = true;
            } else {
                self.state.sidebar_collapsed = self.sidebar_before_compact;
            }
            self.compact = compact;
        }
        let bg = cx.theme().background;
        let fg = cx.theme().foreground;
        let can_back = self.state.can_go_back();
        let can_forward = self.state.can_go_forward();
        let is_dark = cx.theme().is_dark();
        let follow_system = self.follow_system_theme;
        let app = cx.entity().downgrade();
        let account_app = cx.entity().downgrade();
        let signed_in = self.state.auth.user().is_some();
        let account_name = self.state.user.name.clone();
        let account_role = self.state.user.role.clone();
        let bio_connected = self.biometric_gateway.state().is_connected();
        let bio_label = self.biometric_gateway.state().label();
        let can_create_consumption = self
            .state
            .auth
            .user()
            .is_some_and(|u| u.can("consumptions.create"));
        let title = if !signed_in {
            "Comedor".to_string()
        } else if self.denied || !self.state.auth.can_view(self.state.page) {
            "Comedor · Acceso denegado".to_string()
        } else {
            format!("Comedor · {}", self.state.page.title())
        };

        v_flex()
            .size_full()
            .track_focus(&self.focus_handle)
            .on_action(cx.listener(|this, _: &GoDashboard, _, cx| {
                this.navigate_to(Page::Dashboard, cx);
            }))
            .on_action(cx.listener(|this, _: &GoSettings, _, cx| {
                this.navigate_to(Page::Configuracion, cx);
            }))
            .on_action(cx.listener(|this, _: &ToggleSidebar, _, cx| {
                this.toggle_sidebar(cx);
            }))
            .on_action(cx.listener(|this, _: &ToggleTheme, window, cx| {
                this.toggle_theme(window, cx);
            }))
            .on_action(cx.listener(|this, _: &RunDiagnostics, _, cx| {
                if this.state.auth.user().is_some_and(|u| u.can("settings.update")) {
                    this.start_diagnostics(false, cx);
                }
            }))
            .on_action(cx.listener(|this, _: &ShowAbout, _, cx| {
                this.state.error_banner = Some(
                    "Comedor · Gestión de Pensionistas v0.1.0 (Módulo 1 Base · Windows Desktop)."
                        .into(),
                );
                cx.notify();
            }))
            .on_action(|_: &QuitApp, _, cx| {
                cx.quit();
            })
            .on_action(cx.listener(|this, _: &FocusGlobalSearch, window, cx| {
                this.focus_global_search(window, cx);
            }))
            .bg(bg)
            .text_color(fg)
            .child(
                TitleBar::new().child(
                    h_flex()
                        .id("titlebar-interactive-controls")
                        .occlude()
                        .gap_1()
                        .items_center()
                        .when(signed_in, |this| this.child(
                            Button::new("titlebar-nav-back")
                                .ghost()
                                .xsmall()
                                .icon(IconName::ArrowLeft)
                                .disabled(!can_back)
                                .tooltip("Atrás")
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.navigate_back(cx);
                                })),
                        ))
                        .when(signed_in, |this| this.child(
                            Button::new("titlebar-nav-forward")
                                .ghost()
                                .xsmall()
                                .icon(IconName::ArrowRight)
                                .disabled(!can_forward)
                                .tooltip("Adelante")
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.navigate_forward(cx);
                                })),
                        ))
                        .when(signed_in, |this| this.child(
                            SidebarToggleButton::new()
                                .side(Side::Left)
                                .collapsed(self.state.sidebar_collapsed)
                                .accessibility_label(if self.state.sidebar_collapsed {
                                    "Expandir menú lateral"
                                } else {
                                    "Contraer menú lateral"
                                })
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.toggle_sidebar(cx);
                                })),
                        ))
                        .child(
                            Button::new("titlebar-theme-toggle")
                                .ghost()
                                .xsmall()
                                .icon(if is_dark { LucideIcon::Moon } else { LucideIcon::Sun })
                                .label("Tema")
                                .dropdown_caret(true)
                                .tooltip("Elegir tema")
                                .dropdown_menu(move |menu, _, _| {
                                    let light_app = app.clone();
                                    let dark_app = app.clone();
                                    let system_app = app.clone();
                                    menu.item(PopupMenuItem::new("Sistema").checked(follow_system).on_click(move |_, window, cx| {
                                        let _ = system_app.update(cx, |this, cx| this.use_system_theme(window, cx));
                                    }))
                                    .item(PopupMenuItem::new("Claro").checked(!follow_system && !is_dark).on_click(move |_, window, cx| {
                                        let _ = light_app.update(cx, |this, cx| this.set_dark_mode(false, window, cx));
                                    }))
                                    .item(PopupMenuItem::new("Oscuro").checked(!follow_system && is_dark).on_click(move |_, window, cx| {
                                        let _ = dark_app.update(cx, |this, cx| this.set_dark_mode(true, window, cx));
                                    }))
                                }),
                        )
                        .when(signed_in, |this| this.child(self.menu_bar.clone())),
                ).child(
                    gpui_kit::div()
                        .flex_1()
                        .min_w_0()
                        .px_2()
                        .text_sm()
                        .text_color(cx.theme().muted_foreground)
                        .text_center()
                        .truncate()
                        .child(title),
                ).when(signed_in, |this| this.child(
                    h_flex()
                        .id("titlebar-account-controls")
                        .occlude()
                        .gap_2()
                        .items_center()
                        .when(can_create_consumption, |this| {
                            this.child(
                                gpui_kit::div()
                                    .w(px(240.))
                                    .child(
                                        Combobox::new(&self.global_pensioner_combobox)
                                            .xsmall()
                                            .icon(LucideIcon::Search)
                                            .placeholder("Buscar pensionista (Ctrl+K)")
                                            .search_placeholder("Nombre, apellido o celular…")
                                            .menu_width(px(360.))
                                            .cleanable(true)
                                            .disabled(self.biometric_modal_open)
                                            .empty(|_, cx| {
                                                gpui_kit::div()
                                                    .py_4()
                                                    .px_3()
                                                    .text_xs()
                                                    .text_center()
                                                    .text_color(cx.theme().muted_foreground)
                                                    .child("Sin pensionistas activos que coincidan.")
                                            }),
                                    ),
                            )
                        })
                        .when(bio_connected, |this| {
                            this.child(
                                Tag::secondary()
                                    .small()
                                    .child(format!("Huella: {bio_label}")),
                            )
                            .child(
                                Button::new("titlebar-sim-finger")
                                    .ghost()
                                    .xsmall()
                                    .icon(LucideIcon::FingerprintPattern)
                                    .tooltip("Simular lectura biométrica automática")
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        this.biometric_simulate(
                                            crate::pages::identification::SimulationMode::MatchActive,
                                            window,
                                            cx,
                                        );
                                    })),
                            )
                        })
                        .child(
                            Button::new("titlebar-account")
                                .ghost()
                                .xsmall()
                                .label(account_name.clone())
                                .dropdown_caret(true)
                                .tooltip("Opciones de cuenta")
                                .dropdown_menu(move |menu, _, _| {
                                    let app = account_app.clone();
                                    let expiry_app = account_app.clone();
                                    menu.item(PopupMenuItem::new(account_name.clone()).disabled(true))
                                        .item(PopupMenuItem::new(account_role.clone()).disabled(true))
                                        .item(PopupMenuItem::new("Mi perfil · próximamente").disabled(true))
                                        .when(cfg!(debug_assertions), |menu| menu.item(PopupMenuItem::new("Simular sesión expirada").on_click(move |_, _, cx| {
                                            let _ = expiry_app.update(cx, |this, cx| this.expire_session(cx));
                                        })))
                                        .item(PopupMenuItem::new("Cerrar sesión").on_click(move |_, _, cx| {
                                            let _ = app.update(cx, |this, cx| this.sign_out(cx));
                                        }))
                                }),
                        ),
                )),
            )
            .when(!signed_in, |this| this.child(login::render(&self.state.auth, &self.username, &self.password, cx)))
            .when(signed_in, |this| this.child(
                h_flex()
                    .flex_1()
                    .min_h_0()
                    .child(sidebar::render_sidebar(&self.state, &self.preferences.dining_hall_name, cx, cx))
                    .child(
                        v_flex()
                            .flex_1()
                            .h_full()
                            .min_w_0()
                            .child(
                                gpui_kit::div()
                                    .flex_1()
                                    .min_h_0()
                                    .overflow_y_scrollbar()
                                    .p_6()
                                    .child(
                                        v_flex()
                                            .gap_4()
                                            .children(
                                                self.state
                                                    .error_banner
                                                    .clone()
                                                    .map(|msg| common::render_error_banner(msg, cx)),
                                            )
                                            .when(self.denied || !self.state.auth.can_view(self.state.page), |this| this.child(gpui_kit::div().text_lg().child("Acceso denegado: no tienes permiso para abrir esta sección.")))
                                            .when(!self.denied && self.state.auth.can_view(self.state.page), |this| this.child(pages::render_page(&self.state, self, cx, cx, &self.pensioner_search, self.pensioner_filter, self.selected_pensioner, self.biometric_gateway.state(), &self.biometric_event_log))),
                                    ),
                            ),
                    ),
            ))
    }
}
