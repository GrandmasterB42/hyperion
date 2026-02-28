use std::path::Path;

use bevy_ecs::resource::Resource;
#[cfg(feature = "reflect")]
use bevy_reflect::Reflect;
use rustls::pki_types::{CertificateDer, PrivateKeyDer, pem::PemObject};

#[derive(Resource)]
#[cfg_attr(feature = "reflect", derive(Reflect), reflect(opaque))]
pub struct Crypto {
    /// The root certificate authority's certificate
    pub root_ca_cert: CertificateDer<'static>,

    /// The game server's certificate
    pub cert: CertificateDer<'static>,

    /// The game server's private key
    pub key: PrivateKeyDer<'static>,
}

impl Crypto {
    pub fn new(
        root_ca_cert_path: &Path,
        cert_path: &Path,
        key_path: &Path,
    ) -> Result<Self, rustls::pki_types::pem::Error> {
        Ok(Self {
            root_ca_cert: CertificateDer::from_pem_file(root_ca_cert_path)?,
            cert: CertificateDer::from_pem_file(cert_path)?,
            key: PrivateKeyDer::from_pem_file(key_path)?,
        })
    }
}

impl Clone for Crypto {
    fn clone(&self) -> Self {
        Self {
            root_ca_cert: self.root_ca_cert.clone(),
            cert: self.cert.clone(),
            key: self.key.clone_key(),
        }
    }
}
