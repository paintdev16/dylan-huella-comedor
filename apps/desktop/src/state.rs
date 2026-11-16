use gpui_kit::{
    SharedString, Task,
    assets::IconName as LucideIcon,
    component::{Icon, ThemeMode},
};

use crate::{auth::AuthStatus, services::WorkspaceDiagnostics};
use crate::mock_data::{self, MockData};
use chrono::NaiveDate;

#[derive(Default)]
pub enum DataStatus { #[default] Loading, Ready(MockData), Error(String) }

/// Secciones principales de navegación de la aplicación.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum Page {
    #[default]
    Dashboard,
    Pensionistas,
    Identificacion,
    Consumos,
    Caja,
    Reportes,
    Configuracion,
}

impl Page {
    pub fn permission(self) -> &'static str { match self {
        Self::Dashboard => "dashboard.view", Self::Pensionistas => "pensioners.view", Self::Identificacion => "biometrics.view", Self::Consumos => "consumptions.view", Self::Caja => "payments.view", Self::Reportes => "reports.view", Self::Configuracion => "settings.view",
    } }
    pub const PRINCIPAL: [Self; 1] = [Self::Dashboard];
    pub const OPERACION: [Self; 3] = [Self::Pensionistas, Self::Identificacion, Self::Consumos];
    pub const ADMINISTRACION: [Self; 2] = [Self::Caja, Self::Reportes];
    pub const SISTEMA: [Self; 1] = [Self::Configuracion];

    pub fn title(self) -> &'static str {
        match self {
            Self::Dashboard => "Dashboard",
            Self::Pensionistas => "Pensionistas",
            Self::Identificacion => "Identificación",
            Self::Consumos => "Consumos",
            Self::Caja => "Caja",
            Self::Reportes => "Reportes",
            Self::Configuracion => "Configuración",
        }
    }

    pub fn icon(self) -> Icon {
        let name = match self {
            Self::Dashboard => LucideIcon::LayoutDashboard,
            Self::Pensionistas => LucideIcon::Users,
            Self::Identificacion => LucideIcon::FingerprintPattern,
            Self::Consumos => LucideIcon::Utensils,
            Self::Caja => LucideIcon::Wallet,
            Self::Reportes => LucideIcon::FileChartColumn,
            Self::Configuracion => LucideIcon::Settings,
        };
        Icon::new(name)
    }
}

/// Información del operador local mostrado en la barra superior.
#[derive(Clone, Debug)]
pub struct UserProfile {
    pub name: SharedString,
    pub role: SharedString,
}

impl Default for UserProfile {
    fn default() -> Self {
        Self {
            name: "Operador Local".into(),
            role: "Administración · Comedor".into(),
        }
    }
}

/// Estado de la verificación asíncrona del entorno local.
#[derive(Clone, Debug, Default)]
pub enum ServiceStatus {
    #[default]
    Idle,
    Loading,
    Ready(WorkspaceDiagnostics),
    Failed(SharedString),
}

/// Estado global de la ventana principal de escritorio.
pub struct AppState {
    /// Fuente común en memoria; las futuras vistas deben actualizar esta misma colección.
    pub data: DataStatus,
    pub dashboard_date: NaiveDate,
    pub auth: AuthStatus,
    pub page: Page,
    pub history: Vec<Page>,
    pub history_index: usize,
    pub sidebar_collapsed: bool,
    pub theme_mode: ThemeMode,
    pub user: UserProfile,
    pub service_status: ServiceStatus,
    pub error_banner: Option<SharedString>,
    pub diagnostics_task: Option<Task<()>>,
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            data: DataStatus::Loading,
            dashboard_date: mock_data::today(),
            auth: AuthStatus::default(),
            page: Page::Dashboard,
            history: vec![Page::Dashboard],
            history_index: 0,
            sidebar_collapsed: false,
            theme_mode: ThemeMode::Light,
            user: UserProfile::default(),
            service_status: ServiceStatus::Idle,
            error_banner: None,
            diagnostics_task: None,
        }
    }
}

impl AppState {
    pub fn can_go_back(&self) -> bool {
        self.history_index > 0
    }

    pub fn can_go_forward(&self) -> bool {
        self.history_index + 1 < self.history.len()
    }

    pub fn push_page(&mut self, page: Page) -> bool {
        if self.page == page {
            return false;
        }
        self.history.truncate(self.history_index + 1);
        self.history.push(page);
        self.history_index = self.history.len() - 1;
        self.page = page;
        true
    }

    pub fn go_back(&mut self) -> bool {
        if !self.can_go_back() {
            return false;
        }
        self.history_index -= 1;
        self.page = self.history[self.history_index];
        true
    }

    pub fn go_forward(&mut self) -> bool {
        if !self.can_go_forward() {
            return false;
        }
        self.history_index += 1;
        self.page = self.history[self.history_index];
        true
    }
}
