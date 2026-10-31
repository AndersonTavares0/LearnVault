//! Testes do domínio de cartões de estudo pessoais com revisão espaçada (Flashcards).

use learnvault_core::paths::relative_path_key;
use learnvault_core::{
    Database, Error, KeyMode, ReviewRating, create_flashcard, delete_flashcard, list_flashcards,
    review_flashcard, update_flashcard,
};

type TestResult = Result<(), Box<dyn std::error::Error>>;

const LIB: &str = "bib-cards";
const OWNER: &str = "aluno-flash-1";
const OTHER: &str = "aluno-flash-2";
const MEDIA_A: &str = "midia-a";
const MEDIA_B: &str = "midia-b";

fn key(path: &str) -> String {
    relative_path_key(path, KeyMode::for_host()).expect("caminho de teste válido")
}

fn library_db() -> Result<Database, Box<dyn std::error::Error>> {
    let db = Database::open_in_memory()?;
    db.connection().execute(
        "INSERT INTO library (library_id, available) VALUES (?1, 1)",
        [LIB],
    )?;
    for media_id in [MEDIA_A, MEDIA_B] {
        let path = format!("curso/{media_id}.mp4");
        db.connection().execute(
            "INSERT INTO media (media_id, library_id, relative_path, relative_path_key, size, mtime, availability)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, 'available')",
            rusqlite::params![media_id, LIB, path, key(&path), 1024, 1_760_000_000],
        )?;
    }
    Ok(db)
}

#[test]
fn flashcard_crud_and_isolation_flow() -> TestResult {
    let db = library_db()?;

    // 1. Criar cartão válido
    let card1 = create_flashcard(
        &db,
        OWNER,
        MEDIA_A,
        "O que é idempotência?",
        "Propriedade de uma operação que pode ser aplicada várias vezes sem mudar o resultado inicial.",
    )?;
    assert_eq!(card1.front, "O que é idempotência?");
    assert_eq!(card1.interval_days, 0, "novo cartão deve ter intervalo 0");
    assert!(
        card1.next_due <= card1.created_at + 2,
        "novo cartão vence imediatamente"
    );

    let card2 = create_flashcard(
        &db,
        OWNER,
        MEDIA_A,
        "O que é ACID?",
        "Atomicidade, Consistência, Isolamento e Durabilidade.",
    )?;

    // 2. Listar cartões do OWNER na MEDIA_A
    let cards = list_flashcards(&db, OWNER, MEDIA_A)?;
    assert_eq!(cards.len(), 2);
    assert_eq!(cards[0].card_id, card1.card_id);
    assert_eq!(cards[1].card_id, card2.card_id);

    // 3. Isolamento por proprietário: OTHER não vê os cartões do OWNER
    let other_cards = list_flashcards(&db, OTHER, MEDIA_A)?;
    assert!(
        other_cards.is_empty(),
        "outro dono não deve ver cartões alheios"
    );

    // 4. Isolamento por mídia: na MEDIA_B a lista está vazia
    let b_cards = list_flashcards(&db, OWNER, MEDIA_B)?;
    assert!(
        b_cards.is_empty(),
        "mídia sem cartões deve devolver lista vazia"
    );

    // 5. Atualizar cartão (frente e verso)
    let updated = update_flashcard(
        &db,
        OWNER,
        &card1.card_id,
        "O que é idempotência na prática?",
        "Operação que pode rodar repetidamente produzindo o mesmo efeito.",
    )?;
    assert_eq!(updated.front, "O que é idempotência na prática?");
    assert_eq!(updated.interval_days, 0, "atualização preserva agendamento");
    assert_eq!(
        updated.next_due, card1.next_due,
        "atualização preserva next_due"
    );

    // OTHER não consegue atualizar o cartão do OWNER
    let unauthorized_update = update_flashcard(&db, OTHER, &card1.card_id, "Tentativa", "Falha");
    assert!(matches!(
        unauthorized_update,
        Err(Error::FlashcardNotFound { .. })
    ));

    // 6. Excluir cartão
    let deleted = delete_flashcard(&db, OWNER, &card1.card_id)?;
    assert!(deleted, "exclusão com sucesso deve retornar true");

    let remaining = list_flashcards(&db, OWNER, MEDIA_A)?;
    assert_eq!(remaining.len(), 1);
    assert_eq!(remaining[0].card_id, card2.card_id);

    // Exclusão por outro dono não afeta nada
    let other_deleted = delete_flashcard(&db, OTHER, &card2.card_id)?;
    assert!(
        !other_deleted,
        "outro dono não consegue excluir cartão alheio"
    );
    assert_eq!(list_flashcards(&db, OWNER, MEDIA_A)?.len(), 1);

    Ok(())
}

#[test]
fn flashcard_spaced_repetition_scheduling_rules() -> TestResult {
    let db = library_db()?;

    let card = create_flashcard(&db, OWNER, MEDIA_A, "Pergunta 1", "Resposta 1")?;
    assert_eq!(card.interval_days, 0);

    // 1. Revisão 'Good' em cartão novo (interval_days == 0):
    // Deve definir intervalo de 1 dia (86400s)
    let rev1 = review_flashcard(&db, OWNER, &card.card_id, ReviewRating::Good)?;
    assert_eq!(rev1.interval_days, 1);
    assert!(rev1.next_due >= rev1.updated_at + 86400 - 2);

    // 2. Revisão 'Good' subsequente (interval_days == 1):
    // Deve dobrar o intervalo para 2 dias
    let rev2 = review_flashcard(&db, OWNER, &card.card_id, ReviewRating::Good)?;
    assert_eq!(rev2.interval_days, 2);
    assert!(rev2.next_due >= rev2.updated_at + 2 * 86400 - 2);

    // 3. Revisão 'Good' subsequente (interval_days == 2):
    // Deve dobrar para 4 dias
    let rev3 = review_flashcard(&db, OWNER, &card.card_id, ReviewRating::Good)?;
    assert_eq!(rev3.interval_days, 4);

    // 4. Revisão 'Again' em cartão com intervalo:
    // Deve zerar o intervalo para 0 e vencer em 10 minutos (600s)
    let rev_again = review_flashcard(&db, OWNER, &card.card_id, ReviewRating::Again)?;
    assert_eq!(rev_again.interval_days, 0, "Again zera o intervalo");
    assert!(rev_again.next_due >= rev_again.updated_at + 600 - 2);
    assert!(rev_again.next_due <= rev_again.updated_at + 600 + 5);

    // 5. Novo 'Good' após 'Again' começa novamente em 1 dia
    let rev_good_after_again = review_flashcard(&db, OWNER, &card.card_id, ReviewRating::Good)?;
    assert_eq!(rev_good_after_again.interval_days, 1);

    // 6. Teste do teto de 365 dias
    // Simula manualmente um cartão com intervalo de 200 dias
    db.connection().execute(
        "UPDATE flashcard SET interval_days = 200 WHERE card_id = ?1",
        [&card.card_id],
    )?;
    let rev_capped = review_flashcard(&db, OWNER, &card.card_id, ReviewRating::Good)?;
    assert_eq!(
        rev_capped.interval_days, 365,
        "intervalo deve respeitar o teto de 365 dias"
    );

    // Good após 365 continua 365
    let rev_capped_again = review_flashcard(&db, OWNER, &card.card_id, ReviewRating::Good)?;
    assert_eq!(rev_capped_again.interval_days, 365);

    // 7. Isolamento de revisão: OTHER não pode revisar
    let unauthorized_rev = review_flashcard(&db, OTHER, &card.card_id, ReviewRating::Good);
    assert!(matches!(
        unauthorized_rev,
        Err(Error::FlashcardNotFound { .. })
    ));

    Ok(())
}

#[test]
fn flashcard_validation_rejects_empty_whitespace_and_oversized() -> TestResult {
    let db = library_db()?;

    // Vazio ou apenas espaços
    assert!(matches!(
        create_flashcard(&db, OWNER, MEDIA_A, "", "Resposta"),
        Err(Error::InvalidFlashcardContent)
    ));
    assert!(matches!(
        create_flashcard(&db, OWNER, MEDIA_A, "   \n\t  ", "Resposta"),
        Err(Error::InvalidFlashcardContent)
    ));
    assert!(matches!(
        create_flashcard(&db, OWNER, MEDIA_A, "Pergunta", ""),
        Err(Error::InvalidFlashcardContent)
    ));
    assert!(matches!(
        create_flashcard(&db, OWNER, MEDIA_A, "Pergunta", "   "),
        Err(Error::InvalidFlashcardContent)
    ));

    // Excede 4000 escalares Unicode
    let oversized = "a".repeat(4001);
    assert!(matches!(
        create_flashcard(&db, OWNER, MEDIA_A, &oversized, "Resposta"),
        Err(Error::InvalidFlashcardContent)
    ));
    assert!(matches!(
        create_flashcard(&db, OWNER, MEDIA_A, "Pergunta", &oversized),
        Err(Error::InvalidFlashcardContent)
    ));

    // Exatamente 4000 escalares é aceito
    let boundary_4000 = "a".repeat(4000);
    let card = create_flashcard(&db, OWNER, MEDIA_A, &boundary_4000, "Resposta")?;
    assert_eq!(card.front.len(), 4000);

    // Mídia desconhecida
    assert!(matches!(
        create_flashcard(&db, OWNER, "midia-inexistente", "Pergunta", "Resposta"),
        Err(Error::MediaNotFound { .. })
    ));

    Ok(())
}
