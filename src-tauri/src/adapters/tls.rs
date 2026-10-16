//! TLS das conexões com rustls, a partir do [`SslMode`] do servidor.
//!
//! - `prefer`/`require`: cifra sem verificar o certificado (servidor
//!   autoassinado) — a assinatura do handshake continua conferida;
//! - `verify-ca`: a cadeia precisa terminar numa CA confiável, mas o nome no
//!   certificado não é checado;
//! - `verify-full`: cadeia **e** nome do host.
//!
//! As CAs confiáveis são as do sistema (no macOS, o Keychain — inclusive uma
//! CA corporativa instalada lá) ou, se o servidor tiver `ssl_root_cert`, só a
//! do arquivo PEM.

use std::path::Path;
use std::sync::{Arc, OnceLock};

use rustls::client::danger::{HandshakeSignatureValid, ServerCertVerified, ServerCertVerifier};
use rustls::client::WebPkiServerVerifier;
use rustls::crypto::{self, CryptoProvider};
use rustls::pki_types::pem::PemObject;
use rustls::pki_types::{CertificateDer, ServerName, UnixTime};
use rustls::{
    CertificateError, ClientConfig, DigitallySignedStruct, RootCertStore, SignatureScheme,
};

use crate::error::{Error, Result};
use crate::models::SslMode;

fn provider() -> Arc<CryptoProvider> {
    static PROVIDER: OnceLock<Arc<CryptoProvider>> = OnceLock::new();
    Arc::clone(PROVIDER.get_or_init(|| Arc::new(crypto::ring::default_provider())))
}

/// As CAs do sistema, lidas uma vez por execução (no macOS a leitura do
/// Keychain custa dezenas de ms). Certificado ilegível é ignorado.
fn system_roots() -> Arc<RootCertStore> {
    static ROOTS: OnceLock<Arc<RootCertStore>> = OnceLock::new();
    Arc::clone(ROOTS.get_or_init(|| {
        let loaded = rustls_native_certs::load_native_certs();
        for error in &loaded.errors {
            log::warn!("TLS: falha ao ler CAs do sistema: {error}");
        }
        let mut roots = RootCertStore::empty();
        let (added, ignored) = roots.add_parsable_certificates(loaded.certs);
        log::debug!("TLS: {added} CA(s) do sistema ({ignored} ignorada(s))");
        Arc::new(roots)
    }))
}

/// CAs de um arquivo PEM (um ou mais certificados).
fn file_roots(path: &str) -> Result<Arc<RootCertStore>> {
    let invalid = |detail: String| Error::Connection(format!("CA {path}: {detail}"));
    let mut roots = RootCertStore::empty();
    for cert in
        CertificateDer::pem_file_iter(Path::new(path)).map_err(|e| invalid(e.to_string()))?
    {
        roots
            .add(cert.map_err(|e| invalid(e.to_string()))?)
            .map_err(|e| invalid(e.to_string()))?;
    }
    if roots.is_empty() {
        return Err(invalid("nenhum certificado no arquivo".into()));
    }
    Ok(Arc::new(roots))
}

/// Configuração do cliente para o modo; `None` em `disable`.
pub fn client_config(mode: SslMode, root_cert: Option<&str>) -> Result<Option<ClientConfig>> {
    let verifier: Arc<dyn ServerCertVerifier> = match mode {
        SslMode::Disable => return Ok(None),
        SslMode::Prefer | SslMode::Require => Arc::new(NoVerification(provider())),
        SslMode::VerifyCa | SslMode::VerifyFull => {
            let roots = match root_cert {
                Some(path) => file_roots(path)?,
                None => system_roots(),
            };
            let webpki = WebPkiServerVerifier::builder_with_provider(roots, provider())
                .build()
                .map_err(|e| Error::Connection(format!("TLS setup failed: {e}")))?;
            if mode == SslMode::VerifyCa {
                Arc::new(IgnoreHostname(webpki))
            } else {
                webpki
            }
        }
    };

    let config = ClientConfig::builder_with_provider(provider())
        .with_safe_default_protocol_versions()
        .map_err(|e| Error::Connection(format!("TLS setup failed: {e}")))?
        .dangerous()
        .with_custom_certificate_verifier(verifier)
        .with_no_client_auth();
    Ok(Some(config))
}

/// `prefer`/`require`: aceita qualquer certificado, mas confere a assinatura
/// do handshake (sem isso nem a cifra estaria garantida).
#[derive(Debug)]
struct NoVerification(Arc<CryptoProvider>);

impl ServerCertVerifier for NoVerification {
    fn verify_server_cert(
        &self,
        _end_entity: &CertificateDer<'_>,
        _intermediates: &[CertificateDer<'_>],
        _server_name: &ServerName<'_>,
        _ocsp_response: &[u8],
        _now: UnixTime,
    ) -> std::result::Result<ServerCertVerified, rustls::Error> {
        Ok(ServerCertVerified::assertion())
    }

    fn verify_tls12_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> std::result::Result<HandshakeSignatureValid, rustls::Error> {
        crypto::verify_tls12_signature(
            message,
            cert,
            dss,
            &self.0.signature_verification_algorithms,
        )
    }

    fn verify_tls13_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> std::result::Result<HandshakeSignatureValid, rustls::Error> {
        crypto::verify_tls13_signature(
            message,
            cert,
            dss,
            &self.0.signature_verification_algorithms,
        )
    }

    fn supported_verify_schemes(&self) -> Vec<SignatureScheme> {
        self.0.signature_verification_algorithms.supported_schemes()
    }
}

/// `verify-ca`: a verificação completa, aceitando só o erro de nome — ele vem
/// depois da cadeia já validada, então a CA continua obrigatória.
#[derive(Debug)]
struct IgnoreHostname(Arc<WebPkiServerVerifier>);

impl ServerCertVerifier for IgnoreHostname {
    fn verify_server_cert(
        &self,
        end_entity: &CertificateDer<'_>,
        intermediates: &[CertificateDer<'_>],
        server_name: &ServerName<'_>,
        ocsp_response: &[u8],
        now: UnixTime,
    ) -> std::result::Result<ServerCertVerified, rustls::Error> {
        match self
            .0
            .verify_server_cert(end_entity, intermediates, server_name, ocsp_response, now)
        {
            Err(rustls::Error::InvalidCertificate(
                CertificateError::NotValidForName | CertificateError::NotValidForNameContext { .. },
            )) => Ok(ServerCertVerified::assertion()),
            other => other,
        }
    }

    fn verify_tls12_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> std::result::Result<HandshakeSignatureValid, rustls::Error> {
        self.0.verify_tls12_signature(message, cert, dss)
    }

    fn verify_tls13_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> std::result::Result<HandshakeSignatureValid, rustls::Error> {
        self.0.verify_tls13_signature(message, cert, dss)
    }

    fn supported_verify_schemes(&self) -> Vec<SignatureScheme> {
        self.0.supported_verify_schemes()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn disable_has_no_config_and_the_others_do() {
        assert!(client_config(SslMode::Disable, None).unwrap().is_none());
        for mode in [
            SslMode::Prefer,
            SslMode::Require,
            SslMode::VerifyCa,
            SslMode::VerifyFull,
        ] {
            assert!(client_config(mode, None).unwrap().is_some(), "{mode:?}");
        }
    }

    #[test]
    fn root_cert_file_must_hold_a_certificate() {
        let dir = std::env::temp_dir().join(format!("octapus-tls-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let empty = dir.join("empty.pem");
        std::fs::write(&empty, "nada aqui").unwrap();
        let error = client_config(SslMode::VerifyFull, empty.to_str())
            .expect_err("arquivo sem certificado");
        assert!(error.to_string().contains("nenhum certificado"), "{error}");

        let missing = dir.join("missing.pem");
        assert!(client_config(SslMode::VerifyFull, missing.to_str()).is_err());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
