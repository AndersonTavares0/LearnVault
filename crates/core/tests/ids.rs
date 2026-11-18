//! Tests of the media's opaque identity primitives.

use std::collections::HashSet;
use std::thread::sleep;
use std::time::Duration;

use learnvault_core::ids::{new_library_id, new_media_id};

fn assert_uuid_v7(id: &str) {
    assert_eq!(id.len(), 36, "identificador com tamanho inesperado: {id}");

    let bytes = id.as_bytes();
    for (index, byte) in bytes.iter().enumerate() {
        if index == 8 || index == 13 || index == 18 || index == 23 {
            assert_eq!(*byte, b'-', "separador ausente em {id}");
        } else {
            assert!(byte.is_ascii_hexdigit(), "caractere inválido em {id}");
            assert!(!byte.is_ascii_uppercase(), "o id deve ser minúsculo: {id}");
        }
    }

    assert_eq!(bytes[14], b'7', "versão 7 esperada em {id}");
    // RFC 9562 variant bits: the first nibble of the fourth group must be
    // between 8 and b.
    let variant = bytes[19];
    assert!(
        (b'8'..=b'b').contains(&variant),
        "bits de variante inválidos em {id}"
    );
}

#[test]
fn generated_ids_are_valid_v7_uuids() {
    assert_uuid_v7(&new_media_id());
    assert_uuid_v7(&new_library_id());
}

#[test]
fn ids_are_ordered_by_creation_time() {
    let first = new_media_id();
    sleep(Duration::from_millis(5));
    let second = new_media_id();

    assert!(first < second, "{first} deveria preceder {second}");
}

#[test]
fn ids_are_unique_across_many_generations() {
    let mut media_ids = HashSet::new();
    let mut library_ids = HashSet::new();

    for _ in 0..1000 {
        let media_id = new_media_id();
        assert_uuid_v7(&media_id);
        assert!(
            media_ids.insert(media_id),
            "identificador de mídia repetido"
        );

        let library_id = new_library_id();
        assert_uuid_v7(&library_id);
        assert!(
            library_ids.insert(library_id),
            "identificador de biblioteca repetido"
        );
    }
}
