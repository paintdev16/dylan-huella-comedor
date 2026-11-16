//! Adaptador del servicio biométrico para `apps/desktop`.
//!
//! Reexporta los contratos desacoplados de `crates/biometric` y define las
//! señales internas para el bucle continuo sin bloquear el hilo de GPUI.

pub use biometric::{
    BiometricCandidate, BiometricError, BiometricEvent, BiometricGateway, BiometricReaderState,
    EnrollmentStep, IdentificationOutcome, SimulatedBiometricGateway, generate_unique_biometric_id,
};

/// Señales de eventos simulados o de dispositivo hacia el monitor continuo.
#[derive(Clone, Debug)]
pub enum ContinuousBiometricSignal {
    FingerMatch { pensioner_id: Option<u32> },
    FingerUnrecognized,
    DeviceError(String),
}
