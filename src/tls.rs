//! Explicit per-client TLS configuration shared by the two network owners.
use std::sync::Arc;

pub(crate) fn client_config() -> Result<rustls::ClientConfig, String> {
    let mut roots = rustls::RootCertStore::empty();
    roots.add_parsable_certificates(rustls_native_certs::load_native_certs().certs);
    if roots.is_empty() {
        return Err(crate::msg!("tls-no-roots"));
    }
    rustls::ClientConfig::builder_with_provider(Arc::new(rustls::crypto::ring::default_provider()))
        .with_safe_default_protocol_versions()
        .map_err(|e| e.to_string())
        .map(|builder| builder.with_root_certificates(roots).with_no_client_auth())
}
