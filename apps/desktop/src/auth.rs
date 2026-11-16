//! Contratos de autenticación. La implementación mock no ofrece seguridad real.
use serde::{Deserialize, Serialize};
use crate::state::Page;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Role { SuperAdmin, Admin, Operator, Cashier }

impl Role {
    pub fn label(&self) -> &'static str {
        match self { Self::SuperAdmin => "Superadministrador", Self::Admin => "Administrador", Self::Operator => "Operador", Self::Cashier => "Cajero" }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct User {
    pub id: String,
    pub name: String,
    pub username: String,
    pub roles: Vec<Role>,
    pub permissions: Vec<String>,
}

impl User {
    pub fn can(&self, permission: &str) -> bool {
        self.roles.contains(&Role::SuperAdmin) || self.permissions.iter().any(|p| p == permission)
    }
    pub fn can_view(&self, page: Page) -> bool { self.can(page.permission()) }
    pub fn role_label(&self) -> &'static str { self.roles.first().map(Role::label).unwrap_or("Sin rol") }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Session { pub user: User }

#[derive(Clone, Debug, Default)]
pub enum AuthStatus {
    #[default] Unauthenticated,
    Authenticating,
    Authenticated(Session),
    SessionExpired,
    Error(String),
}

impl AuthStatus {
    pub fn user(&self) -> Option<&User> { if let Self::Authenticated(session) = self { Some(&session.user) } else { None } }
    pub fn can_view(&self, page: Page) -> bool { self.user().is_some_and(|user| user.can_view(page)) }
}

#[derive(Debug, PartialEq, Eq)]
pub enum AuthError { InvalidCredentials, Inactive, Expired, Unavailable }
impl AuthError {
    pub fn message(&self) -> &'static str { match self {
        Self::InvalidCredentials => "Usuario o contraseña incorrectos.",
        Self::Inactive => "Esta cuenta está inactiva.",
        Self::Expired => "La sesión ha expirado. Inicia sesión nuevamente.",
        Self::Unavailable => "El servicio de autenticación no está disponible.",
    } }
}

/// Sustituible por un cliente REST que implemente login y logout.
pub trait AuthService: Send + Sync {
    fn login(&self, username: &str, password: &str) -> Result<Session, AuthError>;
    fn logout(&self, session: &Session) -> Result<(), AuthError>;
}

pub struct MockAuthService;
impl AuthService for MockAuthService {
    fn login(&self, username: &str, password: &str) -> Result<Session, AuthError> {
        if !cfg!(debug_assertions) { return Err(AuthError::Unavailable); }
        if password != "demo" { return Err(AuthError::InvalidCredentials); }
        let (role, name, permissions): (Role, &str, &[&str]) = match username.trim() {
            "super_admin" => (Role::SuperAdmin, "Superadministrador de prueba", &[]),
            "admin" => (Role::Admin, "Administradora de prueba", &["dashboard.view", "pensioners.view", "pensioners.create", "pensioners.update", "pensioners.delete", "biometrics.view", "biometrics.enroll", "biometrics.identify", "consumptions.view", "consumptions.create", "payments.view", "payments.create", "reports.view", "settings.view", "settings.update"]),
            "operator" => (Role::Operator, "Operador de prueba", &["dashboard.view", "pensioners.view", "biometrics.view", "biometrics.identify", "consumptions.view", "consumptions.create"]),
            "cashier" => (Role::Cashier, "Cajero de prueba", &["dashboard.view", "payments.view", "payments.create"]),
            "inactivo" => return Err(AuthError::Inactive),
            "expirado" => return Err(AuthError::Expired),
            "error" => return Err(AuthError::Unavailable),
            _ => return Err(AuthError::InvalidCredentials),
        };
        Ok(Session { user: User { id: format!("demo:{username}"), name: name.into(), username: username.trim().into(), roles: vec![role], permissions: permissions.iter().map(|p| (*p).into()).collect() } })
    }
    fn logout(&self, _session: &Session) -> Result<(), AuthError> { Ok(()) }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test] fn mock_and_permissions() {
        let svc = MockAuthService;
        let cashier = svc.login("cashier", "demo").unwrap();
        assert!(cashier.user.can_view(Page::Caja));
        assert!(!cashier.user.can_view(Page::Reportes));
        assert!(svc.login("super_admin", "demo").unwrap().user.can("settings.update"));
        assert_eq!(svc.login("admin", "incorrecto").unwrap_err(), AuthError::InvalidCredentials);
        assert_eq!(svc.login("inactivo", "demo").unwrap_err(), AuthError::Inactive);
        assert_eq!(svc.login("expirado", "demo").unwrap_err(), AuthError::Expired);
        assert_eq!(svc.login("error", "demo").unwrap_err(), AuthError::Unavailable);
    }
    #[test] fn no_session_has_no_access() { assert!(!AuthStatus::Unauthenticated.can_view(Page::Dashboard)); }
    #[test] fn session_transitions_clear_access() {
        let session = MockAuthService.login("operator", "demo").unwrap();
        let mut state = AuthStatus::Authenticated(session.clone());
        assert!(state.can_view(Page::Dashboard));
        assert!(!state.can_view(Page::Caja));
        MockAuthService.logout(&session).unwrap();
        state = AuthStatus::Unauthenticated;
        assert!(state.user().is_none());
        assert!(!state.can_view(Page::Dashboard));
        state = AuthStatus::SessionExpired;
        assert!(!state.can_view(Page::Dashboard));
    }
}
