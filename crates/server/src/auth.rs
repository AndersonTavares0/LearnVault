//! Primitivas de autenticação do servidor.

use argon2::{
    Algorithm, Argon2, Params, PasswordHash, PasswordHasher, PasswordVerifier, Version,
    password_hash::SaltString,
};
use rand_core::OsRng;
use sha2::{Digest, Sha256};
use thiserror::Error;

const ARGON_MEMORY_KIB: u32 = 64 * 1024;
const ARGON_ITERATIONS: u32 = 2;
const ARGON_LANES: u32 = 1;
const ARGON_OUTPUT_BYTES: usize = 32;
const SESSION_TOKEN_BYTES: usize = 32;

/// Erros internos das primitivas de autenticação.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum AuthError {
    /// O nome de usuário não atende aos limites do produto.
    #[error("invalid username")]
    InvalidUsername,
    /// A senha inicial não atende aos limites do produto.
    #[error("invalid password")]
    InvalidPassword,
    /// O hashing Argon2id falhou.
    #[error("password hashing failed")]
    PasswordHashing,
    /// A geração criptograficamente segura do token falhou.
    #[error("secure session token generation failed")]
    TokenGeneration,
}

/// Gera um hash PHC Argon2id compatível com os dados legados do Go.
pub fn hash_password(password: &str) -> Result<String, AuthError> {
    validate_password(password)?;
    let params = Params::new(
        ARGON_MEMORY_KIB,
        ARGON_ITERATIONS,
        ARGON_LANES,
        Some(ARGON_OUTPUT_BYTES),
    )
    .map_err(|_| AuthError::PasswordHashing)?;
    let argon = Argon2::new(Algorithm::Argon2id, Version::V0x13, params);
    let mut rng = OsRng;
    let salt = SaltString::generate(&mut rng);
    argon
        .hash_password(password.as_bytes(), &salt)
        .map(|hash| hash.to_string())
        .map_err(|_| AuthError::PasswordHashing)
}

/// Verifica uma senha sem diferenciar senha errada de hash persistido inválido.
pub fn verify_password(password: &str, encoded_hash: &str) -> Result<bool, AuthError> {
    validate_password(password)?;
    let Ok(parsed) = PasswordHash::new(encoded_hash) else {
        return Ok(false);
    };
    Ok(Argon2::default()
        .verify_password(password.as_bytes(), &parsed)
        .is_ok())
}

/// Valida o limite público do nome de usuário sem revelar dados da configuração.
pub fn validate_username(username: &str) -> Result<(), AuthError> {
    let length = username.chars().count();
    if !(2..=64).contains(&length)
        || username.trim() != username
        || username.chars().any(char::is_control)
    {
        return Err(AuthError::InvalidUsername);
    }
    Ok(())
}

/// Valida o limite público da senha inicial ou de login.
pub fn validate_password(password: &str) -> Result<(), AuthError> {
    let length = password.chars().count();
    if !(12..=1024).contains(&length) {
        return Err(AuthError::InvalidPassword);
    }
    Ok(())
}

/// Gera um token de sessão opaco, codificado em hexadecimal.
pub fn generate_session_token() -> Result<String, AuthError> {
    let mut token = [0u8; SESSION_TOKEN_BYTES];
    getrandom::fill(&mut token).map_err(|_| AuthError::TokenGeneration)?;
    Ok(hex_encode(&token))
}

/// Calcula o hash SHA-256 que será persistido, nunca o token em claro.
pub fn hash_session_token(token: &str) -> String {
    let digest = Sha256::digest(token.as_bytes());
    hex_encode(&digest)
}

fn hex_encode(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        output.push(HEX[(byte >> 4) as usize] as char);
        output.push(HEX[(byte & 0x0f) as usize] as char);
    }
    output
}

#[cfg(test)]
mod tests {
    use argon2::{Argon2, PasswordHash, password_hash::PasswordVerifier};
    use base64::{Engine, engine::general_purpose::STANDARD_NO_PAD};

    use super::{
        generate_session_token, hash_password, hash_session_token, validate_password,
        validate_username, verify_password,
    };

    #[test]
    fn password_hash_uses_the_legacy_compatible_argon2id_parameters() {
        let encoded = hash_password("uma senha local muito segura").unwrap();
        let parsed = PasswordHash::new(&encoded).unwrap();

        assert_eq!(parsed.algorithm.as_str(), "argon2id");
        assert_eq!(parsed.params.get_decimal("m").unwrap(), 65_536);
        assert_eq!(parsed.params.get_decimal("t").unwrap(), 2);
        assert_eq!(parsed.params.get_decimal("p").unwrap(), 1);
        assert_eq!(
            STANDARD_NO_PAD
                .decode(parsed.salt.unwrap().as_str())
                .unwrap()
                .len(),
            16
        );
        assert_eq!(parsed.hash.unwrap().as_bytes().len(), 32);
        assert!(
            Argon2::default()
                .verify_password("uma senha local muito segura".as_bytes(), &parsed)
                .is_ok()
        );
    }

    #[test]
    fn password_verification_is_boolean_and_does_not_leak_hash_details() {
        let encoded = hash_password("uma senha local muito segura").unwrap();

        assert!(verify_password("uma senha local muito segura", &encoded).unwrap());
        assert!(!verify_password("senha errada", &encoded).unwrap());
        assert!(!verify_password("qualquer senha", "formato inválido").unwrap());
    }

    #[test]
    fn credentials_have_explicit_bounds() {
        assert!(validate_username("ab").is_ok());
        assert!(validate_username(&"a".repeat(64)).is_ok());
        assert!(validate_username("a").is_err());
        assert!(validate_username(&"a".repeat(65)).is_err());
        assert!(validate_password(&"a".repeat(12)).is_ok());
        assert!(validate_password(&"a".repeat(1024)).is_ok());
        assert!(validate_password(&"a".repeat(11)).is_err());
        assert!(validate_password(&"a".repeat(1025)).is_err());
    }

    #[test]
    fn session_tokens_are_random_and_only_their_hash_is_persisted() {
        let first = generate_session_token().unwrap();
        let second = generate_session_token().unwrap();

        assert_ne!(first, second);
        assert_eq!(first.len(), 64);
        assert_eq!(second.len(), 64);
        assert_ne!(first, hash_session_token(&first));
        assert_eq!(hash_session_token(&first), hash_session_token(&first));
    }
}
