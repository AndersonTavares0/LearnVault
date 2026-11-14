# ADR: Virtual course roots for remote sources

## Status

Proposed (2026-11-14). Needs maintainer approval before any code. Refs #509, #441.

## Context

A remote WebDAV/SMB source is one registered library today. Verified in the code:

- The source `id` is the `library_id`, the catalog `course_id` and the media route `course` parameter (`crates/server/src/catalog.rs`, `crates/server/src/media.rs`). `build_catalog(database, &source.id)` turns the whole source into one course node.
- `media` rows are keyed by `(library_id, relative_path_key)` and study state, notes, bookmarks, annotations and flashcards hang off `media_id` (`crates/core/src/migrations.rs`). Favorites and per-user course access are keyed by `course_id`. Manual ordering is keyed by the source id.
- The recursive scan already keeps relative paths, so a source holding grouping folders and several real courses appears as one large course tree.

Folder names, language and nesting depth must not be hardcoded, and original files stay read-only.

## Research

- Media servers decide what an item is from a fixed folder depth: Audiobookshelf expects one folder per book (Author/Series/Book), Kavita reads `Library Root/Series Name/`, Plex expects the first subfolder level to be the show and handles extra layers badly, and Jellyfin expects library/series/season for TV and does no extra sorting for books. A fixed depth or naming rule is what #509 forbids, so LearnVault cannot copy it; it can only offer suggestions an administrator confirms.
- Rename and move handling is weak or heuristic elsewhere (Plex is reported to match by path, then a partial file hash; other servers miss renames). LearnVault should not guess: an unconfirmed move keeps history under the old identity and is re-pointed explicitly.
- Depth-agnostic but content-blind: Komga makes a Series for every subfolder at any depth, so a course with module folders would split into many tiny series. Audiobookshelf does the opposite: the folder holding the audio is the item and everything below merges into it, and its request for a more flexible structure (#2208) is still open. Neither offers a manual override.
- Suggest, review, confirm: beets scores every match, applies only a near-certain one, asks otherwise (Apply, More candidates, Skip, Use as-is, enter manually) and has a quiet mode that skips uncertain items; Sonarr's Library Import shows the proposed mapping for each folder and nothing is imported until the user starts the import. LearnVault takes this model but is stricter: nothing is applied without confirmation, because a wrong boundary affects study state. High-confidence suggestions are only pre-selected.
- A folder tree alone cannot tell a course with modules from a group of courses (`Cursos/Programação/` looks like both), but the shape can, if it is read from the lessons upward instead of by absolute depth. For `Cursos/01 Rust/01 Introdução/01 aula.mp4`: the media files are lessons; the folder that directly holds them (`01 Introdução`) is a module; the parent of a numbered sequence of modules (`01 Rust`) is the course; the folders above courses (`Cursos`) are groups. The same numbering at every level is therefore not ambiguous, because only one level holds the media and the level above it holds the numbered sequence.
- The rule, per subtree and independent of language and of fixed depth (so a flat course and a course with modules can sit side by side): (1) a media-holding folder with a numbered sibling sequence is a module, and its parent is the course; (2) a media-holding folder without such a sequence is a flat course; (3) anything above a course is a group. `crates/core/src/structure.rs` already measures numbering.
- Still uncertain, and marked so: unnumbered modules (`Introdução`, `Fundamentos`) that look like flat courses, a course with a single module folder, and a group whose children are numbered like modules. The administrator resolves these in the review; the "select all folders at this level" shortcut covers a whole source at once.
- Sources: [Komga scan guide](https://komga.org/docs/guides/scan-analysis-refresh), [Audiobookshelf book structure](https://www.audiobookshelf.org/docs/documentation/libraries/book-library/directory-structure) and [issue 2208](https://github.com/advplyr/audiobookshelf/issues/2208), [beets auto-tagger](https://docs.beets.io/en/v1.4.8/guides/tagger.html), [Sonarr library import](https://wiki.servarr.com/Sonarr_Quick_Start_Guide), [Plex TV folders thread](https://forums.plex.tv/t/folder-structions-for-tv-shows-problem-with-folders-in-folder/709574.md), [Jellyfin naming overview](https://grandmediastation-website.pages.dev/blog/jellyfin-naming-conventions), [Plex rename detection thread](https://forums.plex.tv/t/detecting-intros-after-renaming-all-media-files/635953). Komga and beets were read on their official pages; the Plex and Jellyfin points are forum and third-party reports, not vendor source code.

## Decision

Use **explicit virtual course roots** (option 1 of #509), where **each confirmed root becomes its own library**. Content analysis only produces suggestions.

Why a root is a library and not a side table: the course identity is `media.library_id` in many queries (the access check in `crates/core/src/search.rs` compares `user_course_access.course_id` with `m.library_id`; removal, `media_removal`, bookmarks, flashcards and sync also key on it; 21 source files mention `library_id`). A table that only maps `course_id` to `(source, prefix)` while media stays under the source library would force changes in all of them. Giving each root its own `library_id` keeps those queries correct as they are.

1. A new additive table `library_root(library_id, source_id, prefix)` says that `library_id` is the part of `source_id` under `prefix` (the full relative path, so equal folder names under different parents never collide). `media.relative_path` stays relative to the source, so reads and range requests are unchanged.
2. A source with no roots behaves exactly as today: one course whose `course_id` is the source id. Existing IDs, favorites, access lists, ordering and study state stay valid.
3. Confirming a root moves the matching `media` rows (and their `media_removal` and scan history rows) to the new `library_id` in one transaction. `media_id` does not change, so study state, notes, bookmarks, annotations and flashcards follow. Removing a root reverses it. Files outside every root stay in the source library and remain a course while any exist. Overlapping roots are rejected; a path belongs to the longest matching prefix.
4. Only an administrator creates roots, through a review screen. The scan produces suggestions from the bottom-up rule above, marked high confidence or uncertain, never applied automatically. The review offers per suggestion: Confirm, Skip, and mark a different folder by hand, plus a bulk "select all folders at this level" shortcut that only ticks items in the review and stores no rule. High-confidence suggestions are pre-selected; uncertain ones are not. Ambiguous trees stay as the single source view.
5. Scan reads the source once and applies the result per library by prefix. Media and transcode requests resolve the `course` to a root, then require the path to be inside its prefix after normalization. Per-user access is checked on the root's `library_id`, and a root is only reachable if its source is.
6. Rescan keeps a root while its prefix exists. A missing prefix marks the root unavailable and keeps history. A rename or move is never guessed: an administrator re-points the root. Removing a root only removes the mapping, never source files or study rows.

## Consequences

- No change to existing tables; one new table, plus a transactional promote and demote of media rows.
- The catalog emits one course per root (and the leftover source course if it still has files); ordering is keyed by the root `library_id` and search, access and removal work unchanged. Covers stay unsupported for remote roots until decided separately.
- More administrator work (confirm roots) in exchange for no silent mistakes.
- Rejected: a side table with media left in the source library (touches every `library_id` query), content-based auto-splitting (wrong boundaries would silently hide or reset study state) and keeping only the source-level view (does not solve the problem).

## Implementation slices

Each slice is its own small PR; slices 2 to 4 need independent review (persistence, identity, authorization, media confinement).

1. Core, pure: partition a `ScanOutcome` by prefixes (longest match, overlap rejection) and suggest roots with the bottom-up rule (lessons, modules, courses, groups), marking the uncertain cases (reusing `structure.rs`). Synthetic fixtures: flat, grouped, multi-group, direct-media, mixed-depth, duplicate-name, empty and ambiguous.
2. Core, persistence: `library_root` migration and the transactional promote and demote. Tests prove study state, notes, flashcards and favorites survive both directions.
3. Server: admin API for suggestions and roots, per-root scan, catalog assembly, media and access resolution with prefix confinement. Security tests: no escape from the prefix, no access bypass, no cross-root reads.
4. Web: in Remote sources, a "Find courses" action lists suggestions and the administrator confirms; `tr(en, ptBr)` copy and component tests.
5. Docs and a final independent review.

## Validation and rollback

- Synthetic fixtures: flat, grouped, multi-group, direct-media, mixed-depth, duplicate-name, empty and ambiguous trees.
- Security tests: a virtual course cannot read outside its prefix (`..`, encoded separators, case and Unicode variants), cannot use another user's access, and cannot bypass source-level access.
- Cover media/range/transcode, favorites, notes, bookmarks, PDF annotations, flashcards, search and course removal through a root.
- Independent read-only review of persistence, identity, authorization and media confinement before merge.
- Rollback: the table is additive; dropping it restores the single-source view.
