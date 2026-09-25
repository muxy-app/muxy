use muxy_protocol::ErrorCode;
use muxy_protocol::transport::tls::TlsIdentity;

use crate::ServerError;

/// A self-signed certificate; phones trust it by pinning its fingerprint.
pub(super) fn generate() -> Result<TlsIdentity, ServerError> {
    let failed = |error: rcgen::Error| {
        ServerError::new(
            ErrorCode::PersistenceFailed,
            format!("could not create the server identity: {error}"),
        )
    };
    let key = rcgen::KeyPair::generate_for(&rcgen::PKCS_ECDSA_P256_SHA256).map_err(failed)?;
    let mut params = rcgen::CertificateParams::default();
    params
        .distinguished_name
        .push(rcgen::DnType::CommonName, "Muxy");
    let certificate = params.self_signed(&key).map_err(failed)?;
    Ok(TlsIdentity {
        certificate: certificate.der().to_vec(),
        private_key: key.serialize_der(),
    })
}
