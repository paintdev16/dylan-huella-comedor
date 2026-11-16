//! Adaptador del servicio biométrico para `apps/desktop`.
//!
//! Reexporta los contratos desacoplados de `crates/biometric` y define las
//! señales internas para el bucle continuo sin bloquear el hilo de GPUI.

pub use biometric::{
    BiometricCandidate, BiometricError, BiometricEvent, BiometricGateway, BiometricReaderState,
    ContinuousBiometricSignal, EnrollmentStep, IdentificationOutcome, SimulatedBiometricGateway,
    generate_unique_biometric_id,
};
