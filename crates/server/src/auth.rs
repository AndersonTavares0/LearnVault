//! Primitivas de autenticação do servidor.

#[cfg(test)]
mod tests {
    use argon2::{Argon2, PasswordHash, password_hash::PasswordVerifier};
    use base64::{Engine, engine::general_purpose::STANDARD_NO_PAD};

    use super::{generate_session_token, hash_password, hash_session_token, validate_password,
        validate_username, verify_password};

    #[test]
    fn password_hash_uses_the_legacy_compatible_argon2id_parameters() {
        let encoded = hash_password("uma senha local muito segura").unwrap();
        let parsed = PasswordHash::new(&encoded).unwrap();

        assert_eq!(parsed.algorithm.as_str(), "argon2id");
        assert_eq!(parsed.params.get_decimal("m").unwrap(), 65_536);
        assert_eq!(parsed.params.get_decimal("t").unwrap(), 2);
        assert_eq!(parsed.params.get_decimal("p").unwrap(), 1);
        assert_eq!(
            STANDARD_NO_PAD.decode(parsed.salt.unwrap().as_str()).unwrap().len(),
            16
        );
        assert_eq!(
            parsed.hash.unwrap().as_bytes().len(),
            32
        );
        assert!(Argon2::default()
            .verify_password("uma senha local muito segura".as_bytes(), &parsed)
            .is_ok());
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
