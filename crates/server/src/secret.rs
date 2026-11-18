//! Encryption of remote-source credentials at rest.
//!
//! The key is a 32-byte base64 file next to the database (`sources.key`), created
//! with `0600` permission on the first write of a password. The secret itself is stored as
//! `v1:<base64(nonce || ChaCha20-Poly1305)>`; neither the password nor the key appears in logs,
//! error messages or HTTP responses.

use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
};

use base64::{Engine as _, engine::general_purpose::STANDARD};
use ring::{
    aead::{Aad, CHACHA20_POLY1305, LessSafeKey, NONCE_LEN, Nonce, UnboundKey},
    rand::{SecureRandom, SystemRandom},
};

/// Size of the symmetric key.
const KEY_LEN: usize = 32;
/// Format prefix of the encrypted secret.
const SECRET_PREFIX: &str = "v1:";

/// Sanitized failure of the secret store.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum SecretError {
    /// The key does not exist or could not be read.
    #[error("a chave de segredos não está disponível")]
    KeyUnavailable,
    /// The stored value could not be decrypted.
    #[error("o segredo gravado não pôde ser decifrado")]
    Undecryptable,
}

/// Symmetric key used to encrypt and decrypt source secrets.
pub(crate) struct SecretKey {
    key: LessSafeKey,
}

impl std::fmt::Debug for SecretKey {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("SecretKey(<redacted>)")
    }
}

impl SecretKey {
    /// Default path of the key for a database.
    pub(crate) fn path_for(database_path: &Path) -> PathBuf {
        let parent = database_path
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
            .unwrap_or_else(|| Path::new("."));
        parent.join("sources.key")
    }

    /// Loads an existing key.
    pub(crate) fn load(path: &Path) -> Result<Self, SecretError> {
        let raw = fs::read_to_string(path).map_err(|_| SecretError::KeyUnavailable)?;
        let bytes = STANDARD
            .decode(raw.trim())
            .map_err(|_| SecretError::KeyUnavailable)?;
        Self::from_bytes(&bytes)
    }

    /// Loads the existing key or creates a new one with restricted permission.
    pub(crate) fn load_or_create(path: &Path) -> Result<Self, SecretError> {
        if path.exists() {
            return Self::load(path);
        }
        let mut bytes = [0_u8; KEY_LEN];
        SystemRandom::new()
            .fill(&mut bytes)
            .map_err(|_| SecretError::KeyUnavailable)?;
        let encoded = STANDARD.encode(bytes);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|_| SecretError::KeyUnavailable)?;
        }
        // `create_new` guarantees that two concurrent writes do not choose different
        // keys; the loser reads the winner's key.
        match restricted_file(path) {
            Ok(mut file) => {
                file.write_all(encoded.as_bytes())
                    .map_err(|_| SecretError::KeyUnavailable)?;
                Self::from_bytes(&bytes)
            }
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => Self::load(path),
            Err(_) => Err(SecretError::KeyUnavailable),
        }
    }

    fn from_bytes(bytes: &[u8]) -> Result<Self, SecretError> {
        let key =
            UnboundKey::new(&CHACHA20_POLY1305, bytes).map_err(|_| SecretError::KeyUnavailable)?;
        Ok(Self {
            key: LessSafeKey::new(key),
        })
    }

    /// Encrypts a secret for storage.
    pub(crate) fn encrypt(&self, plaintext: &str) -> Result<String, SecretError> {
        let mut nonce_bytes = [0_u8; NONCE_LEN];
        SystemRandom::new()
            .fill(&mut nonce_bytes)
            .map_err(|_| SecretError::KeyUnavailable)?;
        let nonce = Nonce::assume_unique_for_key(nonce_bytes);
        let mut sealed = plaintext.as_bytes().to_vec();
        self.key
            .seal_in_place_append_tag(nonce, Aad::empty(), &mut sealed)
            .map_err(|_| SecretError::KeyUnavailable)?;
        let mut payload = nonce_bytes.to_vec();
        payload.extend_from_slice(&sealed);
        Ok(format!("{SECRET_PREFIX}{}", STANDARD.encode(payload)))
    }

    /// Decrypts a stored secret.
    pub(crate) fn decrypt(&self, stored: &str) -> Result<String, SecretError> {
        let payload = stored
            .strip_prefix(SECRET_PREFIX)
            .ok_or(SecretError::Undecryptable)?;
        let payload = STANDARD
            .decode(payload)
            .map_err(|_| SecretError::Undecryptable)?;
        if payload.len() < NONCE_LEN {
            return Err(SecretError::Undecryptable);
        }
        let (nonce_bytes, sealed) = payload.split_at(NONCE_LEN);
        let nonce = Nonce::try_assume_unique_for_key(nonce_bytes)
            .map_err(|_| SecretError::Undecryptable)?;
        let mut sealed = sealed.to_vec();
        let plaintext = self
            .key
            .open_in_place(nonce, Aad::empty(), &mut sealed)
            .map_err(|_| SecretError::Undecryptable)?;
        String::from_utf8(plaintext.to_vec()).map_err(|_| SecretError::Undecryptable)
    }
}

fn restricted_file(path: &Path) -> std::io::Result<fs::File> {
    let mut options = fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    options.open(path)
}

#[cfg(test)]
mod tests {
    use super::{SecretError, SecretKey};

    #[test]
    fn secrets_round_trip_and_do_not_expose_the_plaintext() {
        let temp = tempfile::tempdir().unwrap();
        let key = SecretKey::load_or_create(&temp.path().join("sources.key")).unwrap();
        let sealed = key.encrypt("synthetic-secret").unwrap();

        assert!(!sealed.contains("synthetic-secret"));
        assert_eq!(key.decrypt(&sealed).unwrap(), "synthetic-secret");
    }

    #[test]
    fn a_different_key_cannot_decrypt_the_secret() {
        let first = tempfile::tempdir().unwrap();
        let second = tempfile::tempdir().unwrap();
        let first_key = SecretKey::load_or_create(&first.path().join("sources.key")).unwrap();
        let second_key = SecretKey::load_or_create(&second.path().join("sources.key")).unwrap();
        let sealed = first_key.encrypt("synthetic-secret").unwrap();

        assert_eq!(second_key.decrypt(&sealed), Err(SecretError::Undecryptable));
        assert_eq!(
            first_key.decrypt("v1:not-base64"),
            Err(SecretError::Undecryptable)
        );
        assert_eq!(
            first_key.decrypt("synthetic-secret"),
            Err(SecretError::Undecryptable)
        );
    }

    #[cfg(unix)]
    #[test]
    fn the_key_file_is_private_to_the_owner() {
        use std::os::unix::fs::PermissionsExt;

        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("sources.key");
        SecretKey::load_or_create(&path).unwrap();
        let mode = std::fs::metadata(&path).unwrap().permissions().mode();

        assert_eq!(mode & 0o777, 0o600);
    }
}
