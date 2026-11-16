//! Interfaz de servicio biométrico desacoplada de GPUI.
//!
//! Define el contrato, estados y eventos internos para integrar en etapas
//! posteriores el crate `biometric` (SecuGen FDx SDK Pro y SecuSearch Pro)
//! sin acoplar la lógica del dispositivo a la capa visual ni depender de WebSocket.

use thiserror::Error;

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
                "Simulación desconectada. Inicia la conexión simulada para continuar.".into()
            }
            Self::Connected => {
                "Conexión simulada lista. Inicia el modo de espera.".into()
            }
            Self::WaitingFingerprint => {
                "Modo de espera simulado. Elige una prueba para continuar; no se captura ninguna huella.".into()
            }
            Self::Identifying => {
                "Procesando un resultado de demostración; no se compara ninguna huella real.".into()
            }
            Self::Identified { biometric_id, .. } => {
                format!("Coincidencia simulada (código {biometric_id}).")
            }
            Self::Unrecognized => {
                "La prueba simulada no encontró coincidencias.".into()
            }
            Self::Error(msg) => msg.clone(),
        }
    }

    pub fn is_connected(&self) -> bool {
        !matches!(self, Self::Disconnected)
    }
}

/// Eventos internos emitidos por el servicio biométrico hacia la UI.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum BiometricEvent {
    ReaderStateChanged(BiometricReaderState),
    Enrolled {
        pensioner_id: u32,
        biometric_id: String,
    },
    OperationCancelled,
}

/// Errores del servicio biométrico.
#[derive(Debug, Error, Clone, PartialEq, Eq)]
#[allow(dead_code)]
pub enum BiometricError {
    #[error("El lector biométrico está desconectado.")]
    ReaderDisconnected,
    #[error("La simulación no está en modo de espera.")]
    InvalidState,

    #[error(
        "El identificador biométrico '{biometric_id}' ya pertenece al pensionista #{owner_id}."
    )]
    DuplicateBiometricId { biometric_id: String, owner_id: u32 },

    #[error("Operación biométrica cancelada por el operador.")]
    Cancelled,

    #[error("Fallo del lector simulado: {0}")]
    DeviceError(String),
}

/// Candidato habilitado para comparación 1:N.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BiometricCandidate {
    pub pensioner_id: u32,
    pub biometric_id: String,
    pub active: bool,
}

/// Resultado de una solicitud de identificación simulada.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum IdentificationOutcome {
    Match {
        pensioner_id: u32,
        biometric_id: String,
    },
    NoMatch,
}

/// Contrato desacoplado de GPUI para lectores biométricos (futuro `crates/biometric`).
pub trait BiometricGateway: Send + Sync {
    fn state(&self) -> &BiometricReaderState;

    fn connect(&mut self) -> Result<BiometricEvent, BiometricError>;

    fn disconnect(&mut self) -> BiometricEvent;

    fn start_waiting(&mut self) -> Result<BiometricEvent, BiometricError>;

    fn begin_identifying(&mut self) -> Result<BiometricEvent, BiometricError>;

    fn finish_identification(
        &mut self,
        outcome: Result<IdentificationOutcome, BiometricError>,
    ) -> BiometricEvent;

    #[allow(dead_code)]
    fn validate_enrollment(
        &self,
        pensioner_id: u32,
        biometric_id: &str,
        existing: &[BiometricCandidate],
    ) -> Result<String, BiometricError>;

    fn cancel(&mut self) -> BiometricEvent;
}

/// Implementación simulada para pruebas de flujo sin DLL ni acceso USB.
#[derive(Debug, Default)]
pub struct SimulatedBiometricGateway {
    state: BiometricReaderState,
}

impl SimulatedBiometricGateway {
    pub fn new() -> Self {
        Self {
            state: BiometricReaderState::Disconnected,
        }
    }

    /// Resuelve una identificación simulada contra candidatos activos con huella.
    pub fn simulate_match(
        candidates: &[BiometricCandidate],
        preferred_pensioner_id: Option<u32>,
    ) -> IdentificationOutcome {
        let matched = match preferred_pensioner_id {
            Some(id) => candidates.iter().find(|c| c.active && c.pensioner_id == id),
            None => candidates.iter().find(|c| c.active),
        };

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

    fn connect(&mut self) -> Result<BiometricEvent, BiometricError> {
        self.state = BiometricReaderState::Connected;
        Ok(BiometricEvent::ReaderStateChanged(self.state.clone()))
    }

    fn disconnect(&mut self) -> BiometricEvent {
        self.state = BiometricReaderState::Disconnected;
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
        if !matches!(self.state, BiometricReaderState::WaitingFingerprint) {
            return Err(BiometricError::InvalidState);
        }
        self.state = BiometricReaderState::Identifying;
        Ok(BiometricEvent::ReaderStateChanged(self.state.clone()))
    }

    fn finish_identification(
        &mut self,
        outcome: Result<IdentificationOutcome, BiometricError>,
    ) -> BiometricEvent {
        if !matches!(self.state, BiometricReaderState::Identifying) {
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
            Ok(IdentificationOutcome::NoMatch) => BiometricReaderState::Unrecognized,
            Err(BiometricError::Cancelled) => BiometricReaderState::WaitingFingerprint,
            Err(err) => BiometricReaderState::Error(err.to_string()),
        };

        BiometricEvent::ReaderStateChanged(self.state.clone())
    }

    fn validate_enrollment(
        &self,
        pensioner_id: u32,
        biometric_id: &str,
        existing: &[BiometricCandidate],
    ) -> Result<String, BiometricError> {
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

        Ok(clean.to_string())
    }

    fn cancel(&mut self) -> BiometricEvent {
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
    fn reader_lifecycle_transitions_and_cancel() {
        let mut gw = SimulatedBiometricGateway::new();
        assert_eq!(*gw.state(), BiometricReaderState::Disconnected);
        assert!(gw.start_waiting().is_err());

        gw.connect().unwrap();
        assert_eq!(*gw.state(), BiometricReaderState::Connected);

        gw.start_waiting().unwrap();
        assert_eq!(*gw.state(), BiometricReaderState::WaitingFingerprint);

        gw.begin_identifying().unwrap();
        assert_eq!(*gw.state(), BiometricReaderState::Identifying);

        let evt = gw.cancel();
        assert_eq!(evt, BiometricEvent::OperationCancelled);
        assert_eq!(*gw.state(), BiometricReaderState::WaitingFingerprint);

        gw.disconnect();
        assert_eq!(*gw.state(), BiometricReaderState::Disconnected);
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

        assert!(gw.validate_enrollment(1, "BIO-0001", &existing).is_ok());
        assert!(gw.validate_enrollment(2, "BIO-0002", &existing).is_ok());
    }

    #[test]
    fn late_result_after_cancel_or_disconnect_is_ignored() {
        let mut gw = SimulatedBiometricGateway::new();
        gw.connect().unwrap();
        assert_eq!(gw.begin_identifying(), Err(BiometricError::InvalidState));
        gw.start_waiting().unwrap();
        gw.begin_identifying().unwrap();
        gw.cancel();
        gw.finish_identification(Ok(IdentificationOutcome::Match {
            pensioner_id: 1,
            biometric_id: "BIO-1".into(),
        }));
        assert_eq!(*gw.state(), BiometricReaderState::WaitingFingerprint);
        gw.disconnect();
        gw.finish_identification(Ok(IdentificationOutcome::NoMatch));
        assert_eq!(*gw.state(), BiometricReaderState::Disconnected);
    }

    #[test]
    fn inactive_or_unmatched_candidate_never_identifies_another_person() {
        let candidates = vec![
            BiometricCandidate {
                pensioner_id: 1,
                biometric_id: "BIO-1".into(),
                active: false,
            },
            BiometricCandidate {
                pensioner_id: 2,
                biometric_id: "BIO-2".into(),
                active: true,
            },
        ];
        assert_eq!(
            SimulatedBiometricGateway::simulate_match(&candidates, Some(1)),
            IdentificationOutcome::NoMatch
        );
        assert_eq!(
            SimulatedBiometricGateway::simulate_match(&candidates, Some(99)),
            IdentificationOutcome::NoMatch
        );
    }
}
