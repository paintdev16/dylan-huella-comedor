//! Persistencia local de preferencias no sensibles y abstracción de credenciales.
//!
//! No almacena contraseñas de PostgreSQL ni claves biométricas en texto plano.

use std::{
    fs,
    path::{Path, PathBuf},
};

use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default, Serialize, Deserialize)]
pub enum ThemePreference {
    #[default]
    System,
    Light,
    Dark,
}

impl ThemePreference {
    pub fn label(self) -> &'static str {
        match self {
            Self::System => "Sistema",
            Self::Light => "Claro",
            Self::Dark => "Oscuro",
        }
    }
}

/// Preferencias no sensibles persistidas localmente en disco.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct AppPreferences {
    pub dining_hall_name: String,
    pub currency_code: String,
    pub theme_preference: ThemePreference,
    pub sidebar_collapsed: bool,
    pub economic_breakfast_cents: i64,
    pub economic_lunch_cents: i64,
    pub special_default_cents: i64,
    pub ticket_value_cents: i64,
    pub breakfast_start_minutes: u32,
    pub breakfast_end_minutes: u32,
    pub lunch_start_minutes: u32,
    pub lunch_end_minutes: u32,
    pub biometric_identification_enabled: bool,
    pub server_url: String,
    pub station_id: String,
}

impl Default for AppPreferences {
    fn default() -> Self {
        Self {
            dining_hall_name: "Comedor".to_string(),
            currency_code: "PEN".to_string(),
            theme_preference: ThemePreference::System,
            sidebar_collapsed: false,
            economic_breakfast_cents: 700,
            economic_lunch_cents: 700,
            special_default_cents: 1200,
            ticket_value_cents: 700,
            breakfast_start_minutes: 6 * 60,
            breakfast_end_minutes: 11 * 60,
            lunch_start_minutes: 11 * 60,
            lunch_end_minutes: 16 * 60,
            biometric_identification_enabled: true,
            server_url: "http://127.0.0.1:8080".to_string(),
            station_id: "CAJA-01".to_string(),
        }
    }
}

/// Formatea minutos desde medianoche como `HH:MM`.
pub fn format_time_minutes(minutes: u32) -> String {
    let clamped = minutes.min(23 * 60 + 59);
    format!("{:02}:{:02}", clamped / 60, clamped % 60)
}

/// Parsea una hora en formato `HH:MM` o `H` (0..=23) a minutos desde medianoche.
pub fn parse_time_minutes(raw: &str) -> Result<u32, String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Err("La hora no puede estar vacía (usa formato HH:MM).".into());
    }
    let (hour_str, min_str) = match trimmed.split_once(':') {
        Some((h, m)) => (h.trim(), m.trim()),
        None => (trimmed, "00"),
    };
    if hour_str.is_empty() || min_str.is_empty() || min_str.len() > 2 {
        return Err(format!("Formato de hora inválido '{trimmed}'. Usa HH:MM (ej. 07:30)."));
    }
    let hour: u32 = hour_str
        .parse()
        .map_err(|_| format!("Hora inválida '{trimmed}'. Usa formato HH:MM."))?;
    let minute: u32 = min_str
        .parse()
        .map_err(|_| format!("Minutos inválidos '{trimmed}'. Usa formato HH:MM."))?;
    if hour > 23 || minute > 59 {
        return Err(format!(
            "La hora '{trimmed}' está fuera de rango (00:00 a 23:59)."
        ));
    }
    Ok(hour * 60 + minute)
}

/// Valida que los horarios de desayuno y almuerzo sean coherentes y no se superpongan.
pub fn validate_meal_schedule(
    breakfast_start: u32,
    breakfast_end: u32,
    lunch_start: u32,
    lunch_end: u32,
) -> Result<(), String> {
    if breakfast_start >= breakfast_end {
        return Err("La hora de fin del desayuno debe ser posterior a la hora de inicio.".into());
    }
    if lunch_start >= lunch_end {
        return Err("La hora de fin del almuerzo debe ser posterior a la hora de inicio.".into());
    }
    if breakfast_end > lunch_start {
        return Err(
            "El horario de desayuno no puede superponerse con el inicio del almuerzo.".into(),
        );
    }
    Ok(())
}

/// Valida el formato de la URL del backend Axum.
pub fn validate_server_url(raw: &str) -> Result<String, String> {
    let trimmed = raw.trim().trim_end_matches('/');
    if trimmed.is_empty() {
        return Err("La URL del backend Axum es obligatoria.".into());
    }
    if trimmed.contains(char::is_whitespace) {
        return Err("La URL no puede contener espacios.".into());
    }
    let rest = trimmed
        .strip_prefix("http://")
        .or_else(|| trimmed.strip_prefix("https://"))
        .ok_or_else(|| "La URL debe iniciar con http:// o https://.".to_string())?;

    let authority = rest.split('/').next().unwrap_or("");
    if authority.is_empty() {
        return Err("La URL debe incluir un host válido (ej. http://127.0.0.1:8080).".into());
    }
    if let Some((host, port_str)) = authority.rsplit_once(':') {
        if host.is_empty() {
            return Err("El host de la URL no puede estar vacío.".into());
        }
        if !port_str.is_empty() {
            let port: u16 = port_str
                .parse()
                .map_err(|_| "El puerto de la URL debe ser un número entre 1 y 65535.".to_string())?;
            if port == 0 {
                return Err("El puerto de la URL debe ser mayor a 0.".into());
            }
        }
    }
    Ok(trimmed.to_string())
}

pub fn default_preferences_path() -> PathBuf {
    let base = std::env::var_os("LOCALAPPDATA")
        .or_else(|| std::env::var_os("XDG_CONFIG_HOME"))
        .map(PathBuf::from)
        .unwrap_or_else(std::env::temp_dir);
    base.join("ComedorPensionistas").join("preferences.json")
}

pub fn load_preferences_from(path: &Path) -> Result<AppPreferences, String> {
    if !path.exists() {
        return Ok(AppPreferences::default());
    }
    let raw = fs::read_to_string(path)
        .map_err(|e| format!("No se pudieron leer las preferencias: {e}"))?;
    let mut prefs: AppPreferences = serde_json::from_str(&raw)
        .map_err(|e| format!("El archivo de preferencias es inválido: {e}"))?;
    prefs.currency_code = "PEN".to_string();
    Ok(prefs)
}

pub fn save_preferences_to(path: &Path, prefs: &AppPreferences) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|e| format!("No se pudo crear el directorio de configuración: {e}"))?;
    }
    let json = serde_json::to_string_pretty(prefs)
        .map_err(|e| format!("No se pudieron serializar las preferencias: {e}"))?;
    fs::write(path, json)
        .map_err(|e| format!("No se pudieron guardar las preferencias: {e}"))?;
    Ok(())
}

#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub enum ServerConnectionStatus {
    #[default]
    Untested,
    Testing,
    Connected(String),
    Failed(String),
}

#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub enum BiometricBackupStatus {
    #[default]
    Pending,
    BackedUp(String),
}

/// Abstracción para almacenar credenciales en el almacén seguro del sistema operativo
/// (por ejemplo Windows Credential Manager / DPAPI) en etapas futuras.
#[allow(dead_code)]
pub trait CredentialVault: Send + Sync {
    fn backend_name(&self) -> &'static str;
    fn store_secret(&self, key: &str, secret: &str) -> Result<(), String>;
    fn has_secret(&self, key: &str) -> bool;
    fn delete_secret(&self, key: &str) -> Result<(), String>;
}

/// Implementación placeholder que evita persistir secretos en texto plano.
#[derive(Debug, Default)]
pub struct OsCredentialVaultPlaceholder;

impl CredentialVault for OsCredentialVaultPlaceholder {
    fn backend_name(&self) -> &'static str {
        if cfg!(target_os = "windows") {
            "Windows Credential Manager / DPAPI (pendiente de integración)"
        } else {
            "Keyring del sistema operativo (pendiente de integración)"
        }
    }

    fn store_secret(&self, key: &str, _secret: &str) -> Result<(), String> {
        Err(format!(
            "Almacenamiento seguro para '{key}' diferido al módulo de credenciales del SO; no se guardan secretos en texto plano."
        ))
    }

    fn has_secret(&self, _key: &str) -> bool {
        false
    }

    fn delete_secret(&self, _key: &str) -> Result<(), String> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn url_validation_accepts_http_https_and_rejects_invalid() {
        assert_eq!(
            validate_server_url("http://127.0.0.1:8080/").unwrap(),
            "http://127.0.0.1:8080"
        );
        assert_eq!(
            validate_server_url("https://api.comedor.local").unwrap(),
            "https://api.comedor.local"
        );
        assert!(validate_server_url("").is_err());
        assert!(validate_server_url("ftp://127.0.0.1").is_err());
        assert!(validate_server_url("http://").is_err());
        assert!(validate_server_url("http://localhost:99999").is_err());
    }

    #[test]
    fn preferences_persist_only_non_sensitive_fields() {
        let dir = std::env::temp_dir().join("comedor_prefs_test");
        let file = dir.join("preferences.json");
        let _ = fs::remove_file(&file);

        let prefs = AppPreferences {
            dining_hall_name: "Comedor Central".into(),
            server_url: "https://comedor.local:8443".into(),
            station_id: "CAJA-02".into(),
            ..Default::default()
        };
        save_preferences_to(&file, &prefs).unwrap();

        let raw = fs::read_to_string(&file).unwrap();
        assert!(!raw.contains("password"));
        assert!(!raw.contains("postgres"));
        assert!(!raw.contains("biometric_key"));

        let loaded = load_preferences_from(&file).unwrap();
        assert_eq!(loaded, prefs);

        let vault = OsCredentialVaultPlaceholder;
        assert!(vault.store_secret("pg_password", "secret").is_err());
        assert!(!vault.has_secret("pg_password"));

        let _ = fs::remove_file(&file);
    }

    #[test]
    fn meal_schedule_parsing_and_validation() {
        assert_eq!(parse_time_minutes("06:00").unwrap(), 360);
        assert_eq!(parse_time_minutes("11:30").unwrap(), 690);
        assert_eq!(parse_time_minutes("16").unwrap(), 960);
        assert_eq!(format_time_minutes(360), "06:00");
        assert_eq!(format_time_minutes(690), "11:30");

        assert!(parse_time_minutes("").is_err());
        assert!(parse_time_minutes("24:00").is_err());
        assert!(parse_time_minutes("10:60").is_err());
        assert!(parse_time_minutes("abc").is_err());

        assert!(validate_meal_schedule(360, 660, 660, 960).is_ok());
        assert!(validate_meal_schedule(660, 360, 660, 960).is_err());
        assert!(validate_meal_schedule(360, 720, 660, 960).is_err());
        assert!(validate_meal_schedule(360, 660, 960, 900).is_err());
    }
}
