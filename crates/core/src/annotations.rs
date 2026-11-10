//! PDF annotations: the reader's marks on a page of a PDF lesson.
//!
//! The course file is never written. Highlights, underlines and notes live in the database, tied
//! to a media id and a page. Rectangles are stored as fractions of the page (0 to 1), so a mark
//! lands in the same place at any zoom. Every operation is scoped to an owner: one reader never
//! sees or changes another reader's marks.

use rusqlite::{OptionalExtension, params};

use crate::db::{Database, now_unix_seconds};
use crate::error::{Error, Result};
use crate::study::{require_media, validate_owner};

/// Highest page number accepted; matches the table constraint.
pub const MAX_PDF_PAGE: i64 = 100_000;
/// Most rectangles one annotation may have (a long highlight spans many lines).
pub const MAX_PDF_RECTS: usize = 200;
const MAX_QUOTE_CHARS: usize = 4_000;
const MAX_CONTENT_CHARS: usize = 10_000;

/// What the mark does to the text.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PdfAnnotationKind {
    /// A colored band behind the text.
    Highlight,
    /// A line under the text.
    Underline,
    /// A note anchored to the text; it must have text of its own.
    Note,
}

impl PdfAnnotationKind {
    /// The value stored in the database and sent over the API.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Highlight => "highlight",
            Self::Underline => "underline",
            Self::Note => "note",
        }
    }

    /// Reads a stored or received value; anything else is refused.
    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "highlight" => Some(Self::Highlight),
            "underline" => Some(Self::Underline),
            "note" => Some(Self::Note),
            _ => None,
        }
    }
}

/// The four marker colors.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PdfAnnotationColor {
    /// Yellow.
    Yellow,
    /// Green.
    Green,
    /// Blue.
    Blue,
    /// Pink.
    Pink,
}

impl PdfAnnotationColor {
    /// The value stored in the database and sent over the API.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Yellow => "yellow",
            Self::Green => "green",
            Self::Blue => "blue",
            Self::Pink => "pink",
        }
    }

    /// Reads a stored or received value; anything else is refused.
    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "yellow" => Some(Self::Yellow),
            "green" => Some(Self::Green),
            "blue" => Some(Self::Blue),
            "pink" => Some(Self::Pink),
            _ => None,
        }
    }
}

/// A rectangle on a page, as fractions of the page width and height, origin at the top left.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PdfRect {
    /// Left edge, 0 to 1.
    pub x: f64,
    /// Top edge, 0 to 1.
    pub y: f64,
    /// Width, above 0 and up to 1.
    pub width: f64,
    /// Height, above 0 and up to 1.
    pub height: f64,
}

/// A stored annotation.
#[derive(Debug, Clone, PartialEq)]
pub struct PdfAnnotation {
    /// Opaque identifier (UUID v7).
    pub annotation_id: String,
    /// The PDF the mark belongs to.
    pub media_id: String,
    /// 1-based page.
    pub page: i64,
    /// Highlight, underline or note.
    pub kind: PdfAnnotationKind,
    /// Marker color.
    pub color: PdfAnnotationColor,
    /// Where the mark sits on the page.
    pub rects: Vec<PdfRect>,
    /// The marked text, kept so the mark can be listed and turned into a card.
    pub quote: String,
    /// The reader's own words; required for a note, optional otherwise.
    pub content: String,
    /// Creation time, Unix seconds.
    pub created_at: i64,
    /// Last edit time, Unix seconds.
    pub updated_at: i64,
}

/// What it takes to create an annotation.
#[derive(Debug, Clone, PartialEq)]
pub struct NewPdfAnnotation {
    /// 1-based page.
    pub page: i64,
    /// Highlight, underline or note.
    pub kind: PdfAnnotationKind,
    /// Marker color.
    pub color: PdfAnnotationColor,
    /// Where the mark sits on the page.
    pub rects: Vec<PdfRect>,
    /// The marked text.
    pub quote: String,
    /// The reader's own words.
    pub content: String,
}

const COLUMNS: &str =
    "annotation_id, media_id, page, kind, color, rects, quote, content, created_at, updated_at";

/// Rectangles as `x,y,width,height` groups separated by `;`, five decimals each.
fn encode_rects(rects: &[PdfRect]) -> Result<String> {
    if rects.is_empty() || rects.len() > MAX_PDF_RECTS {
        return Err(Error::InvalidPdfAnnotation);
    }
    let mut parts = Vec::with_capacity(rects.len());
    for rect in rects {
        let finite = [rect.x, rect.y, rect.width, rect.height]
            .iter()
            .all(|value| value.is_finite());
        // A small tolerance absorbs rounding at the page edge.
        let inside = (0.0..=1.0).contains(&rect.x)
            && (0.0..=1.0).contains(&rect.y)
            && rect.width > 0.0
            && rect.height > 0.0
            && rect.x + rect.width <= 1.0001
            && rect.y + rect.height <= 1.0001;
        if !finite || !inside {
            return Err(Error::InvalidPdfAnnotation);
        }
        parts.push(format!(
            "{:.5},{:.5},{:.5},{:.5}",
            rect.x, rect.y, rect.width, rect.height
        ));
    }
    Ok(parts.join(";"))
}

fn decode_rects(text: &str) -> Option<Vec<PdfRect>> {
    text.split(';')
        .map(|part| {
            let mut numbers = part.split(',').map(|value| value.parse::<f64>().ok());
            let rect = PdfRect {
                x: numbers.next()??,
                y: numbers.next()??,
                width: numbers.next()??,
                height: numbers.next()??,
            };
            numbers.next().is_none().then_some(rect)
        })
        .collect()
}

fn from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<PdfAnnotation> {
    let corrupt = |column: usize, what: &str| {
        rusqlite::Error::FromSqlConversionFailure(column, rusqlite::types::Type::Text, what.into())
    };
    let kind: String = row.get(3)?;
    let color: String = row.get(4)?;
    let rects: String = row.get(5)?;
    Ok(PdfAnnotation {
        annotation_id: row.get(0)?,
        media_id: row.get(1)?,
        page: row.get(2)?,
        kind: PdfAnnotationKind::parse(&kind).ok_or_else(|| corrupt(3, "unknown kind"))?,
        color: PdfAnnotationColor::parse(&color).ok_or_else(|| corrupt(4, "unknown color"))?,
        rects: decode_rects(&rects).ok_or_else(|| corrupt(5, "malformed rectangles"))?,
        quote: row.get(6)?,
        content: row.get(7)?,
        created_at: row.get(8)?,
        updated_at: row.get(9)?,
    })
}

fn checked_text(value: &str, limit: usize) -> Result<String> {
    let trimmed = value.trim();
    if trimmed.chars().count() > limit {
        return Err(Error::InvalidPdfAnnotation);
    }
    Ok(trimmed.to_owned())
}

/// Lists the owner's annotations on one PDF, by page and then by creation time.
pub fn list_pdf_annotations(
    db: &Database,
    owner_id: &str,
    media_id: &str,
) -> Result<Vec<PdfAnnotation>> {
    validate_owner(owner_id)?;
    let conn = db.connection();
    require_media(conn, media_id)?;
    let mut statement = conn.prepare(&format!(
        "SELECT {COLUMNS} FROM pdf_annotation
         WHERE owner_id = ?1 AND media_id = ?2
         ORDER BY page, created_at, annotation_id"
    ))?;
    let rows = statement.query_map(params![owner_id, media_id], from_row)?;
    let mut items = Vec::new();
    for row in rows {
        items.push(row?);
    }
    Ok(items)
}

/// Creates an annotation. A note needs text; a highlight or underline may have none.
pub fn create_pdf_annotation(
    db: &Database,
    owner_id: &str,
    media_id: &str,
    new: NewPdfAnnotation,
) -> Result<PdfAnnotation> {
    validate_owner(owner_id)?;
    if !(1..=MAX_PDF_PAGE).contains(&new.page) {
        return Err(Error::InvalidPdfAnnotation);
    }
    let rects = encode_rects(&new.rects)?;
    let quote = checked_text(&new.quote, MAX_QUOTE_CHARS)?;
    let content = checked_text(&new.content, MAX_CONTENT_CHARS)?;
    if new.kind == PdfAnnotationKind::Note && content.is_empty() {
        return Err(Error::InvalidPdfAnnotation);
    }

    let conn = db.connection();
    require_media(conn, media_id)?;
    let annotation_id = crate::ids::new_annotation_id();
    let now = now_unix_seconds();
    conn.execute(
        "INSERT INTO pdf_annotation (annotation_id, owner_id, media_id, page, kind, color, rects, quote, content, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?10)",
        params![
            annotation_id,
            owner_id,
            media_id,
            new.page,
            new.kind.as_str(),
            new.color.as_str(),
            rects,
            quote,
            content,
            now
        ],
    )?;
    Ok(PdfAnnotation {
        annotation_id,
        media_id: media_id.to_owned(),
        page: new.page,
        kind: new.kind,
        color: new.color,
        rects: new.rects,
        quote,
        content,
        created_at: now,
        updated_at: now,
    })
}

/// Changes an annotation's color and/or text. Where it sits and what it marks never change.
pub fn update_pdf_annotation(
    db: &Database,
    owner_id: &str,
    annotation_id: &str,
    color: Option<PdfAnnotationColor>,
    content: Option<&str>,
) -> Result<PdfAnnotation> {
    validate_owner(owner_id)?;
    let conn = db.connection();
    let existing = conn
        .query_row(
            &format!(
                "SELECT {COLUMNS} FROM pdf_annotation WHERE owner_id = ?1 AND annotation_id = ?2"
            ),
            params![owner_id, annotation_id],
            from_row,
        )
        .optional()?
        .ok_or_else(|| Error::PdfAnnotationNotFound {
            annotation_id: annotation_id.to_owned(),
        })?;

    let content = match content {
        Some(text) => checked_text(text, MAX_CONTENT_CHARS)?,
        None => existing.content.clone(),
    };
    if existing.kind == PdfAnnotationKind::Note && content.is_empty() {
        return Err(Error::InvalidPdfAnnotation);
    }
    let color = color.unwrap_or(existing.color);
    let now = now_unix_seconds();
    conn.execute(
        "UPDATE pdf_annotation SET color = ?1, content = ?2, updated_at = ?3
         WHERE owner_id = ?4 AND annotation_id = ?5",
        params![color.as_str(), content, now, owner_id, annotation_id],
    )?;
    Ok(PdfAnnotation {
        color,
        content,
        updated_at: now,
        ..existing
    })
}

/// Deletes one of the owner's annotations. Returns whether it existed.
pub fn delete_pdf_annotation(db: &Database, owner_id: &str, annotation_id: &str) -> Result<bool> {
    validate_owner(owner_id)?;
    let changes = db.connection().execute(
        "DELETE FROM pdf_annotation WHERE owner_id = ?1 AND annotation_id = ?2",
        params![owner_id, annotation_id],
    )?;
    Ok(changes > 0)
}
