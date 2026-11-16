use std::time::Duration;

use gpui_kit::SharedString;
use thiserror::Error;

/// Entrada única de los datos de ejemplo que compartirán los módulos.
pub fn load_demo_data() -> Result<crate::mock_data::MockData, String> {
    let data = crate::mock_data::MockData::demo(crate::mock_data::today());
    data.validate()?;
    Ok(data)
}

/// Errores tipados de la capa de escritorio en el Módulo 1.
#[derive(Debug, Error)]
pub enum DesktopError {
    #[error("No se pudo inicializar la ventana principal: {0}")]
    WindowCreation(String),

    #[error("La verificación local del entorno falló: {0}")]
    Diagnostics(String),
}

/// Resultado de la comprobación inicial no bloqueante del entorno local.
#[derive(Clone, Debug)]
pub struct WorkspaceDiagnostics {
    pub app_version: SharedString,
    pub mode_label: SharedString,
    pub os_label: SharedString,
    pub pending_modules: usize,
}

/// Ejecuta una verificación ligera en segundo plano sin bloquear el hilo de UI.
///
/// En etapas posteriores este servicio se conectará con el cliente API local y
/// la detección de periféricos.
pub async fn run_startup_diagnostics(simulate_error: bool) -> Result<WorkspaceDiagnostics, DesktopError> {
    // Simula una operación I/O breve en un hilo de background.
    std::thread::sleep(Duration::from_millis(180));

    if simulate_error {
        return Err(DesktopError::Diagnostics(
            "Error simulado de diagnóstico local. La interfaz permanece operativa.".to_string(),
        ));
    }

    Ok(WorkspaceDiagnostics {
        app_version: env!("CARGO_PKG_VERSION").into(),
        mode_label: "Base local (sin conexión backend)".into(),
        os_label: if cfg!(target_os = "windows") {
            "Windows Desktop".into()
        } else {
            std::env::consts::OS.into()
        },
        pending_modules: 5,
    })
}
