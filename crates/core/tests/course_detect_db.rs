//! Automatic course roots (ADR 0004 addendum): what a scan may change, and what the
//! administrator's removals keep from coming back.

use learnvault_core::{
    Database, KeyMode, MediaKind, ScanOutcome, ScannedFile, auto_promote_roots,
    dismiss_root_prefix, list_roots, promote_root, relative_path_key, restore_root_prefix,
};

type TestResult = Result<(), Box<dyn std::error::Error>>;

const SRC: &str = "src";

fn outcome(paths: &[String], inaccessible: &[&str]) -> ScanOutcome {
    ScanOutcome {
        files: paths
            .iter()
            .map(|path| ScannedFile {
                relative_path: path.clone(),
                relative_path_key: relative_path_key(path, KeyMode::Sensitive).unwrap(),
                kind: MediaKind::Media,
                size: 10,
                mtime: 1,
            })
            .collect(),
        inaccessible: inaccessible.iter().map(|path| (*path).to_owned()).collect(),
    }
}

/// `Rust` and `Go`, two numbered modules each.
fn library() -> Vec<String> {
    ["Rust", "Go"]
        .iter()
        .flat_map(|course| {
            ["01_Intro", "02_Types"]
                .iter()
                .map(move |module| format!("{course}/{module}/a.mp4"))
        })
        .collect()
}

fn db() -> Result<Database, Box<dyn std::error::Error>> {
    let mut db = Database::open_in_memory()?;
    learnvault_core::apply_scan(&mut db, SRC, &outcome(&library(), &[]), KeyMode::Sensitive)?;
    Ok(db)
}

fn prefixes(db: &Database) -> Vec<String> {
    list_roots(db, SRC)
        .unwrap()
        .into_iter()
        .map(|root| root.prefix)
        .collect()
}

#[test]
fn a_library_gets_one_root_per_course_and_a_second_scan_changes_nothing() -> TestResult {
    let mut db = db()?;
    let scan = outcome(&library(), &[]);
    assert_eq!(
        auto_promote_roots(&mut db, SRC, &scan, KeyMode::Sensitive)?,
        2
    );
    assert_eq!(prefixes(&db), ["Go", "Rust"]);
    assert_eq!(
        auto_promote_roots(&mut db, SRC, &scan, KeyMode::Sensitive)?,
        0
    );
    Ok(())
}

#[test]
fn a_scan_that_could_not_read_a_folder_leaves_the_roots_alone() -> TestResult {
    let mut db = db()?;
    let scan = outcome(&library(), &["Rust/03_More"]);
    assert_eq!(
        auto_promote_roots(&mut db, SRC, &scan, KeyMode::Sensitive)?,
        0
    );
    assert!(prefixes(&db).is_empty());
    Ok(())
}

#[test]
fn a_root_the_administrator_chose_is_kept_and_only_the_rest_is_detected() -> TestResult {
    let mut db = db()?;
    promote_root(&mut db, SRC, "root-go", "Go", KeyMode::Sensitive)?;
    let scan = outcome(&library(), &[]);
    // `Go` is kept as it is; what is left outside it (`Rust`) is detected on its own.
    assert_eq!(
        auto_promote_roots(&mut db, SRC, &scan, KeyMode::Sensitive)?,
        1
    );
    assert_eq!(prefixes(&db), ["Go", "Rust"]);
    Ok(())
}

#[test]
fn a_dismissed_folder_stays_out_until_it_is_chosen_again() -> TestResult {
    let mut db = db()?;
    dismiss_root_prefix(&db, SRC, "Rust", KeyMode::Sensitive)?;
    let scan = outcome(&library(), &[]);
    assert_eq!(
        auto_promote_roots(&mut db, SRC, &scan, KeyMode::Sensitive)?,
        1
    );
    assert_eq!(prefixes(&db), ["Go"]);
    restore_root_prefix(&db, SRC, "Rust", KeyMode::Sensitive)?;
    assert_eq!(
        auto_promote_roots(&mut db, SRC, &scan, KeyMode::Sensitive)?,
        1
    );
    assert_eq!(prefixes(&db), ["Go", "Rust"]);
    Ok(())
}

#[test]
fn a_source_with_too_many_courses_is_left_for_the_picker() -> TestResult {
    let mut db = Database::open_in_memory()?;
    let paths: Vec<String> = (0..150)
        .map(|n| format!("Course {n:03}/01_Intro/a.mp4"))
        .collect();
    let scan = outcome(&paths, &[]);
    learnvault_core::apply_scan(&mut db, SRC, &scan, KeyMode::Sensitive)?;
    assert_eq!(
        auto_promote_roots(&mut db, SRC, &scan, KeyMode::Sensitive)?,
        0
    );
    assert!(prefixes(&db).is_empty());
    Ok(())
}
