//! Integración y contratos biométricos independientes de GPUI y Axum.
//!
//! Prepara la abstracción para conectar posteriormente **SecuGen FDx SDK Pro**
//! (captura de huellas y extracción de plantillas) y **SecuSearch Pro**
//! (identificación 1:N) sin acoplar hardware ni DLLs en esta etapa.

use std::fmt;

/// Plantilla biométrica protegida, separada de los datos de negocio.
#[derive(Clone, PartialEq, Eq)]
pub struct BiometricTemplate {
    pub biometric_id: String,
    /// Representación opaca de plantilla; nunca almacena secretos en texto plano.
    opaque_digest: String,
}

impl BiometricTemplate {
    pub fn new(biometric_id: impl Into<String>, opaque_digest: impl Into<String>) -> Self {
        Self {
            biometric_id: biometric_id.into(),
            opaque_digest: opaque_digest.into(),
        }
    }

    pub fn digest_preview(&self) -> &str {
        &self.opaque_digest
    }
}

impl fmt::Debug for BiometricTemplate {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("BiometricTemplate")
            .field("biometric_id", &self.biometric_id)
            .field("opaque_digest", &"<protegido>")
            .finish()
    }
}

/// Estados operativos del lector biométrico.
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub enum BiometricReaderState {
    #[default]
    Disconnected,
    Connected,
    WaitingFingerprint,
    Identifying,
    Identified {
        pensioner_id: u32,
        biometric_id: String,
    },
    Unrecognized,
    Error(String),
}

impl BiometricReaderState {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Disconnected => "Desconectado",
            Self::Connected => "Conectado",
            Self::WaitingFingerprint => "Esperando huella",
            Self::Identifying => "Identificando",
            Self::Identified { .. } => "Identificado",
            Self::Unrecognized => "No reconocido",
            Self::Error(_) => "Error",
        }
    }

    pub fn description(&self) -> String {
        match self {
            Self::Disconnected => {
                "Lector desconectado. Conecta el dispositivo para iniciar la escucha continua.".into()
            }
            Self::Connected => {
                "Lector conectado. Listo para activar la espera continua de huellas.".into()
            }
            Self::WaitingFingerprint => {
                "Escucha continua activa: esperando que el pensionista coloque su huella.".into()
            }
            Self::Identifying => {
                "Buscando coincidencia 1:N en las plantillas registradas...".into()
            }
            Self::Identified { biometric_id, .. } => {
                format!("Pensionista identificado automáticamente (código {biometric_id}).")
            }
            Self::Unrecognized => {
                "Huella no reconocida. El lector continúa esperando la siguiente lectura.".into()
            }
            Self::Error(msg) => msg.clone(),
        }
    }

    pub fn is_connected(&self) -> bool {
        !matches!(self, Self::Disconnected)
    }
}

/// Eventos internos emitidos por el servicio biométrico sin WebSocket.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum BiometricEvent {
    ReaderStateChanged(BiometricReaderState),
    FingerPlaced,
    FingerRemoved,
    Enrolled {
        pensioner_id: u32,
        biometric_id: String,
    },
    OperationCancelled,
}

/// Errores del subsistema biométrico.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BiometricError {
    ReaderDisconnected,
    DuplicateBiometricId {
        biometric_id: String,
        owner_id: u32,
    },
    Cancelled,
    ModalLocked,
    FingerStillPresent,
    DeviceError(String),
}

impl fmt::Display for BiometricError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ReaderDisconnected => write!(f, "El lector biométrico está desconectado."),
            Self::DuplicateBiometricId {
                biometric_id,
                owner_id,
            } => write!(
                f,
                "El identificador biométrico '{biometric_id}' ya pertenece al pensionista #{owner_id}."
            ),
            Self::Cancelled => write!(f, "Operación biométrica cancelada por el operador."),
            Self::ModalLocked => {
                write!(f, "Lectura ignorada mientras el modal de atención está abierto.")
            }
            Self::FingerStillPresent => {
                write!(f, "El mismo dedo permanece en el lector; retíralo para una nueva lectura.")
            }
            Self::DeviceError(msg) => write!(f, "Fallo del lector biométrico: {msg}"),
        }
    }
}

impl std::error::Error for BiometricError {}

/// Candidato habilitado para búsqueda 1:N (SecuSearch Pro).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BiometricCandidate {
    pub pensioner_id: u32,
    pub biometric_id: String,
    pub active: bool,
}

/// Resultado de una búsqueda 1:N.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum IdentificationOutcome {
    Match {
        pensioner_id: u32,
        biometric_id: String,
    },
    NoMatch,
}

/// Contrato desacoplado de GPUI y Axum para integrar SecuGen FDx SDK Pro y SecuSearch Pro.
pub trait BiometricGateway: Send + Sync {
    fn state(&self) -> &BiometricReaderState;
    fn is_finger_present(&self) -> bool;
    fn is_modal_locked(&self) -> bool;
    fn set_modal_locked(&mut self, locked: bool);

    fn connect(&mut self) -> Result<BiometricEvent, BiometricError>;
    fn disconnect(&mut self) -> BiometricEvent;
    fn start_waiting(&mut self) -> Result<BiometricEvent, BiometricError>;
    fn begin_identifying(&mut self) -> Result<BiometricEvent, BiometricError>;
    fn finish_identification(
        &mut self,
        outcome: Result<IdentificationOutcome, BiometricError>,
    ) -> BiometricEvent;
    fn release_finger(&mut self) -> BiometricEvent;
    fn validate_enrollment(
        &self,
        pensioner_id: u32,
        biometric_id: &str,
        existing: &[BiometricCandidate],
    ) -> Result<BiometricTemplate, BiometricError>;
    fn cancel(&mut self) -> BiometricEvent;
}

/// Implementación basada en eventos para simular el flujo continuo sin hardware ni DLL.
#[derive(Debug, Default)]
pub struct SimulatedBiometricGateway {
    state: BiometricReaderState,
    finger_present: bool,
    modal_locked: bool,
}

impl SimulatedBiometricGateway {
    pub fn new() -> Self {
        Self {
            state: BiometricReaderState::Disconnected,
            finger_present: false,
            modal_locked: false,
        }
    }

    /// Simula búsqueda 1:N sobre candidatos activos con plantilla registrada.
    pub fn simulate_match(
        candidates: &[BiometricCandidate],
        preferred_pensioner_id: Option<u32>,
    ) -> IdentificationOutcome {
        let matched = preferred_pensioner_id
            .and_then(|id| {
                candidates
                    .iter()
                    .find(|c| c.active && c.pensioner_id == id)
            })
            .or_else(|| candidates.iter().find(|c| c.active));

        match matched {
            Some(c) => IdentificationOutcome::Match {
                pensioner_id: c.pensioner_id,
                biometric_id: c.biometric_id.clone(),
            },
            None => IdentificationOutcome::NoMatch,
        }
    }
}

impl BiometricGateway for SimulatedBiometricGateway {
    fn state(&self) -> &BiometricReaderState {
        &self.state
    }

    fn is_finger_present(&self) -> bool {
        self.finger_present
    }

    fn is_modal_locked(&self) -> bool {
        self.modal_locked
    }

    fn set_modal_locked(&mut self, locked: bool) {
        self.modal_locked = locked;
    }

    fn connect(&mut self) -> Result<BiometricEvent, BiometricError> {
        self.state = BiometricReaderState::Connected;
        self.finger_present = false;
        self.modal_locked = false;
        Ok(BiometricEvent::ReaderStateChanged(self.state.clone()))
    }

    fn disconnect(&mut self) -> BiometricEvent {
        self.state = BiometricReaderState::Disconnected;
        self.finger_present = false;
        self.modal_locked = false;
        BiometricEvent::ReaderStateChanged(self.state.clone())
    }

    fn start_waiting(&mut self) -> Result<BiometricEvent, BiometricError> {
        if !self.state.is_connected() {
            return Err(BiometricError::ReaderDisconnected);
        }
        self.state = BiometricReaderState::WaitingFingerprint;
        Ok(BiometricEvent::ReaderStateChanged(self.state.clone()))
    }

    fn begin_identifying(&mut self) -> Result<BiometricEvent, BiometricError> {
        if !self.state.is_connected() {
            return Err(BiometricError::ReaderDisconnected);
        }
        if self.modal_locked {
            return Err(BiometricError::ModalLocked);
        }
        if self.finger_present {
            return Err(BiometricError::FingerStillPresent);
        }
        self.finger_present = true;
        self.state = BiometricReaderState::Identifying;
        Ok(BiometricEvent::ReaderStateChanged(self.state.clone()))
    }

    fn finish_identification(
        &mut self,
        outcome: Result<IdentificationOutcome, BiometricError>,
    ) -> BiometricEvent {
        if !self.state.is_connected() {
            self.state = BiometricReaderState::Disconnected;
            self.finger_present = false;
            return BiometricEvent::ReaderStateChanged(self.state.clone());
        }

        self.state = match outcome {
            Ok(IdentificationOutcome::Match {
                pensioner_id,
                biometric_id,
            }) => BiometricReaderState::Identified {
                pensioner_id,
                biometric_id,
            },
            Ok(IdentificationOutcome::NoMatch) => {
                // En no reconocido, libera para permitir reintento continuo.
                self.finger_present = false;
                BiometricReaderState::Unrecognized
            }
            Err(BiometricError::Cancelled) => {
                self.finger_present = false;
                BiometricReaderState::WaitingFingerprint
            }
            Err(err) => {
                self.finger_present = false;
                BiometricReaderState::Error(err.to_string())
            }
        };

        BiometricEvent::ReaderStateChanged(self.state.clone())
    }

    fn release_finger(&mut self) -> BiometricEvent {
        self.finger_present = false;
        BiometricEvent::FingerRemoved
    }

    fn validate_enrollment(
        &self,
        pensioner_id: u32,
        biometric_id: &str,
        existing: &[BiometricCandidate],
    ) -> Result<BiometricTemplate, BiometricError> {
        let clean = biometric_id.trim();
        if clean.is_empty() {
            return Err(BiometricError::DeviceError(
                "El identificador biométrico no puede estar vacío.".into(),
            ));
        }

        if let Some(conflict) = existing
            .iter()
            .find(|c| c.pensioner_id != pensioner_id && c.biometric_id.eq_ignore_ascii_case(clean))
        {
            return Err(BiometricError::DuplicateBiometricId {
                biometric_id: clean.to_string(),
                owner_id: conflict.pensioner_id,
            });
        }

        Ok(BiometricTemplate::new(
            clean,
            format!("secugen-sim-tpl:{clean}"),
        ))
    }

    fn cancel(&mut self) -> BiometricEvent {
        self.finger_present = false;
        self.modal_locked = false;
        if self.state.is_connected() {
            self.state = BiometricReaderState::WaitingFingerprint;
        } else {
            self.state = BiometricReaderState::Disconnected;
        }
        BiometricEvent::OperationCancelled
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn continuous_guard_blocks_repeated_finger_and_modal_lock() {
        let mut gw = SimulatedBiometricGateway::new();
        gw.connect().unwrap();
        gw.start_waiting().unwrap();

        gw.begin_identifying().unwrap();
        // Mientras el mismo dedo está presente, rechaza nueva lectura
        assert_eq!(gw.begin_identifying().unwrap_err(), BiometricError::FingerStillPresent);

        gw.finish_identification(Ok(IdentificationOutcome::Match {
            pensioner_id: 1,
            biometric_id: "BIO-0001".into(),
        }));
        gw.set_modal_locked(true);
        gw.release_finger();
        // Aunque retire el dedo, si el modal sigue abierto, bloquea nuevas lecturas
        assert_eq!(gw.begin_identifying().unwrap_err(), BiometricError::ModalLocked);

        gw.set_modal_locked(false);
        gw.start_waiting().unwrap();
        assert!(gw.begin_identifying().is_ok());
    }

    #[test]
    fn duplicate_biometric_id_is_rejected() {
        let gw = SimulatedBiometricGateway::new();
        let existing = vec![BiometricCandidate {
            pensioner_id: 1,
            biometric_id: "BIO-0001".into(),
            active: true,
        }];

        let err = gw
            .validate_enrollment(2, "bio-0001", &existing)
            .unwrap_err();
        assert!(matches!(
            err,
            BiometricError::DuplicateBiometricId { owner_id: 1, .. }
        ));

        let tpl = gw.validate_enrollment(2, "BIO-0002", &existing).unwrap();
        assert_eq!(tpl.biometric_id, "BIO-0002");
    }
}
