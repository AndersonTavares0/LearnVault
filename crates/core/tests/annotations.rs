//! Tests for PDF annotations: validation, ownership isolation and round-tripping.
//!
//! The core receives opaque `owner_id` and `media_id` values and never touches the file system:
//! the PDF itself is never read or written.

use learnvault_core::paths::relative_path_key;
use learnvault_core::{
    Database, Error, KeyMode, NewPdfAnnotation, PdfAnnotationColor, PdfAnnotationKind, PdfRect,
    create_pdf_annotation, delete_pdf_annotation, list_pdf_annotations, update_pdf_annotation,
};

type TestResult = Result<(), Box<dyn std::error::Error>>;

const OWNER: &str = "reader-1";
const OTHER: &str = "reader-2";
const MEDIA: &str = "pdf-1";

fn database() -> Result<Database, Box<dyn std::error::Error>> {
    let db = Database::open_in_memory()?;
    db.connection().execute(
        "INSERT INTO library (library_id, available) VALUES ('lib', 1)",
        [],
    )?;
    let path = "course/reading.pdf";
    db.connection().execute(
        "INSERT INTO media (media_id, library_id, relative_path, relative_path_key, size, mtime, availability)
         VALUES (?1, 'lib', ?2, ?3, 1, 1, 'available')",
        rusqlite::params![MEDIA, path, relative_path_key(path, KeyMode::for_host())?],
    )?;
    Ok(db)
}

fn rect(x: f64, y: f64, width: f64, height: f64) -> PdfRect {
    PdfRect {
        x,
        y,
        width,
        height,
    }
}

fn highlight(page: i64) -> NewPdfAnnotation {
    NewPdfAnnotation {
        page,
        kind: PdfAnnotationKind::Highlight,
        color: PdfAnnotationColor::Yellow,
        rects: vec![rect(0.1, 0.2, 0.5, 0.03), rect(0.1, 0.24, 0.3, 0.03)],
        quote: "A struct holding a reference needs a lifetime.".into(),
        content: String::new(),
    }
}

#[test]
fn annotations_round_trip_with_their_rectangles() -> TestResult {
    let db = database()?;
    let created = create_pdf_annotation(&db, OWNER, MEDIA, highlight(12))?;
    assert_eq!(created.page, 12);
    assert_eq!(created.rects.len(), 2);

    let listed = list_pdf_annotations(&db, OWNER, MEDIA)?;
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].annotation_id, created.annotation_id);
    assert_eq!(listed[0].kind, PdfAnnotationKind::Highlight);
    assert_eq!(
        listed[0].quote,
        "A struct holding a reference needs a lifetime."
    );
    // Five decimals are kept, so a stored rectangle comes back unchanged.
    assert_eq!(listed[0].rects[1], rect(0.1, 0.24, 0.3, 0.03));
    Ok(())
}

#[test]
fn annotations_are_listed_by_page_and_belong_to_one_owner() -> TestResult {
    let db = database()?;
    create_pdf_annotation(&db, OWNER, MEDIA, highlight(9))?;
    create_pdf_annotation(&db, OWNER, MEDIA, highlight(2))?;
    create_pdf_annotation(&db, OTHER, MEDIA, highlight(5))?;

    let pages: Vec<i64> = list_pdf_annotations(&db, OWNER, MEDIA)?
        .iter()
        .map(|a| a.page)
        .collect();
    assert_eq!(pages, [2, 9]);
    assert_eq!(list_pdf_annotations(&db, OTHER, MEDIA)?.len(), 1);

    let theirs = list_pdf_annotations(&db, OTHER, MEDIA)?.remove(0);
    // Another reader cannot change or delete it, and learns nothing from the attempt.
    assert!(matches!(
        update_pdf_annotation(
            &db,
            OWNER,
            &theirs.annotation_id,
            Some(PdfAnnotationColor::Pink),
            None
        ),
        Err(Error::PdfAnnotationNotFound { .. })
    ));
    assert!(!delete_pdf_annotation(&db, OWNER, &theirs.annotation_id)?);
    assert_eq!(list_pdf_annotations(&db, OTHER, MEDIA)?.len(), 1);
    Ok(())
}

#[test]
fn a_note_needs_text_but_a_highlight_does_not() -> TestResult {
    let db = database()?;
    let mut note = highlight(1);
    note.kind = PdfAnnotationKind::Note;
    assert!(matches!(
        create_pdf_annotation(&db, OWNER, MEDIA, note.clone()),
        Err(Error::InvalidPdfAnnotation)
    ));
    note.content = "  Why does this not compile?  ".into();
    let created = create_pdf_annotation(&db, OWNER, MEDIA, note)?;
    assert_eq!(created.content, "Why does this not compile?");
    // Clearing a note's text is refused; clearing a highlight's comment is fine.
    assert!(matches!(
        update_pdf_annotation(&db, OWNER, &created.annotation_id, None, Some("   ")),
        Err(Error::InvalidPdfAnnotation)
    ));
    let plain = create_pdf_annotation(&db, OWNER, MEDIA, highlight(1))?;
    let edited = update_pdf_annotation(&db, OWNER, &plain.annotation_id, None, Some(""))?;
    assert_eq!(edited.content, "");
    Ok(())
}

#[test]
fn update_changes_color_and_text_only() -> TestResult {
    let db = database()?;
    let created = create_pdf_annotation(&db, OWNER, MEDIA, highlight(3))?;
    let updated = update_pdf_annotation(
        &db,
        OWNER,
        &created.annotation_id,
        Some(PdfAnnotationColor::Green),
        Some("key idea"),
    )?;
    assert_eq!(updated.color, PdfAnnotationColor::Green);
    assert_eq!(updated.content, "key idea");
    assert_eq!(updated.page, 3);
    assert_eq!(updated.rects, created.rects);
    assert_eq!(updated.quote, created.quote);
    assert_eq!(list_pdf_annotations(&db, OWNER, MEDIA)?[0], updated);
    Ok(())
}

#[test]
fn invalid_pages_rectangles_and_sizes_are_refused() -> TestResult {
    let db = database()?;
    let bad = |mutate: &dyn Fn(&mut NewPdfAnnotation)| {
        let mut annotation = highlight(1);
        mutate(&mut annotation);
        matches!(
            create_pdf_annotation(&db, OWNER, MEDIA, annotation),
            Err(Error::InvalidPdfAnnotation)
        )
    };
    assert!(bad(&|a| a.page = 0));
    assert!(bad(&|a| a.page = 100_001));
    assert!(bad(&|a| a.rects.clear()));
    assert!(bad(&|a| a.rects = vec![rect(0.1, 0.1, 0.5, 0.1); 201]));
    assert!(bad(&|a| a.rects = vec![rect(-0.1, 0.1, 0.5, 0.1)]));
    assert!(bad(&|a| a.rects = vec![rect(0.8, 0.1, 0.5, 0.1)]));
    assert!(bad(&|a| a.rects = vec![rect(0.1, 0.1, 0.0, 0.1)]));
    assert!(bad(&|a| a.rects = vec![rect(f64::NAN, 0.1, 0.5, 0.1)]));
    assert!(bad(&|a| a.quote = "x".repeat(4_001)));
    assert!(bad(&|a| a.content = "x".repeat(10_001)));
    assert!(list_pdf_annotations(&db, OWNER, MEDIA)?.is_empty());
    Ok(())
}

#[test]
fn unknown_media_and_empty_owner_are_refused() -> TestResult {
    let db = database()?;
    assert!(matches!(
        create_pdf_annotation(&db, OWNER, "missing", highlight(1)),
        Err(Error::MediaNotFound { .. })
    ));
    assert!(matches!(
        list_pdf_annotations(&db, OWNER, "missing"),
        Err(Error::MediaNotFound { .. })
    ));
    assert!(matches!(
        create_pdf_annotation(&db, " ", MEDIA, highlight(1)),
        Err(Error::InvalidOwnerId)
    ));
    Ok(())
}

#[test]
fn delete_removes_only_the_requested_annotation() -> TestResult {
    let db = database()?;
    let first = create_pdf_annotation(&db, OWNER, MEDIA, highlight(1))?;
    let second = create_pdf_annotation(&db, OWNER, MEDIA, highlight(2))?;
    assert!(delete_pdf_annotation(&db, OWNER, &first.annotation_id)?);
    assert!(!delete_pdf_annotation(&db, OWNER, &first.annotation_id)?);
    let left = list_pdf_annotations(&db, OWNER, MEDIA)?;
    assert_eq!(left.len(), 1);
    assert_eq!(left[0].annotation_id, second.annotation_id);
    Ok(())
}

#[test]
fn a_new_database_has_the_annotation_table_and_the_schema_check_passes() -> TestResult {
    let db = Database::open_in_memory()?;
    assert!(db.has_current_schema()?);
    Ok(())
}
