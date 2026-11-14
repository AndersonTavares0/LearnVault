# ADR: Virtual course roots for remote sources

## Status

Accepted (2026-11-14), approved by the maintainer. Files outside every chosen root stay visible as a course of the source. Refs #509, #441.

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
- Open-source code read directly: Jellyfin's `SeriesResolver` decides per folder by looking only at its direct children, using name patterns (a child named like a season) and never recursing, so a container of shows is not a show and each subfolder is judged on its own; a `tvshow.nfo` file is the manual override. LearnVault cannot write marker files into a read-only library, so its override lives in the database and is the administrator's confirmation. Local course tools fix the shape instead: OfflineAcademy assumes `root/course/module/lesson` and ignores anything deeper, CourseVault's onboarding hard-codes three levels (category, section, lesson), and OfflineU has no module concept and lets the user point at the course folder. CourseVault also installs a rebuilt index only when lesson positions match the old one and otherwise asks for a decision, which is the same safety rule as our rescan rule (never silently reshuffle identity).
- Conclusion of the research: no existing system detects course boundaries at arbitrary depth. The closest proven pattern is Jellyfin's local, per-subtree decision from direct children plus a manual override, which is what the bottom-up rule and the confirmation step do.
- Sources: [Komga scan guide](https://komga.org/docs/guides/scan-analysis-refresh), [Audiobookshelf book structure](https://www.audiobookshelf.org/docs/documentation/libraries/book-library/directory-structure) and [issue 2208](https://github.com/advplyr/audiobookshelf/issues/2208), [beets auto-tagger](https://docs.beets.io/en/v1.4.8/guides/tagger.html), [Sonarr library import](https://wiki.servarr.com/Sonarr_Quick_Start_Guide), [Plex TV folders thread](https://forums.plex.tv/t/folder-structions-for-tv-shows-problem-with-folders-in-folder/709574.md), [Jellyfin naming overview](https://grandmediastation-website.pages.dev/blog/jellyfin-naming-conventions), [Plex rename detection thread](https://forums.plex.tv/t/detecting-intros-after-renaming-all-media-files/635953), [Jellyfin SeriesResolver](https://github.com/jellyfin/jellyfin/blob/master/Emby.Server.Implementations/Library/Resolvers/TV/SeriesResolver.cs), [OfflineAcademy scanner](https://github.com/afterhours247/offlineacademy/blob/HEAD/lib/scanner.ts), [OfflineU](https://github.com/WhiskeyCoder/OfflineU/blob/HEAD/offlineu_core.py), [CourseVault onboarding](https://github.com/arkodeepg/course-cms/blob/HEAD/scripts/onboard.py). Komga, beets and the source files above were read directly; the Plex and Jellyfin points are forum and third-party reports, not vendor source code.

## Decision

Use **explicit virtual course roots** (option 1 of #509), where **each confirmed root becomes its own library**. Content analysis only produces suggestions.

Why a root is a library and not a side table: the course identity is `media.library_id` in many queries (the access check in `crates/core/src/search.rs` compares `user_course_access.course_id` with `m.library_id`; removal, `media_removal`, bookmarks, flashcards and sync also key on it; 21 source files mention `library_id`). A table that only maps `course_id` to `(source, prefix)` while media stays under the source library would force changes in all of them. Giving each root its own `library_id` keeps those queries correct as they are.

1. A new additive table `library_root(library_id, source_id, prefix)` says that `library_id` is the part of `source_id` under `prefix` (the full relative path, so equal folder names under different parents never collide). `media.relative_path` stays relative to the source, so reads and range requests are unchanged.
2. A source with no roots behaves exactly as today: one course whose `course_id` is the source id. Existing IDs, favorites, access lists, ordering and study state stay valid.
3. Confirming a root moves the matching `media` rows (and their `media_removal` and scan history rows) to the new `library_id` in one transaction. `media_id` does not change, so study state, notes, bookmarks, annotations and flashcards follow. Removing a root reverses it. Files outside every root stay in the source library and remain a course while any exist. Overlapping roots (the same folder, or one inside another) are rejected, so a path belongs to at most one root.
4. Only an administrator creates roots, by **browsing the source like a file picker**: the screen lists one folder level at a time (listing is lazy, so a huge tree is never scanned to pick a course) and the administrator walks to the folder and picks it. Two picks are offered on a folder: "This folder is a course" and "Every folder inside is a course" (for groups such as `Cursos/Pentest`, `Cursos/Infra`, `Cursos/DevOps`). Picking is always explicit and nothing is guessed. Automatic suggestions (the bottom-up rule above) are an optional later helper that only highlights folders in the browser; they are never required and never applied by themselves.
5. Scan reads the source once and applies the result per library by prefix. Media and transcode requests resolve the `course` to a root, then require the path to be inside its prefix after normalization. Per-user access is checked on the root's `library_id`, and a root is only reachable if its source is.
6. Rescan keeps a root while its prefix exists. A missing prefix marks the root unavailable and keeps history. A rename or move is never guessed: an administrator re-points the root. Removing a root only removes the mapping, never source files or study rows.

## Consequences

- No change to existing tables; one new table, plus a transactional promote and demote of media rows.
- The catalog emits one course per root (and the leftover source course if it still has files); ordering is keyed by the root `library_id` and search, access and removal work unchanged. Covers stay unsupported for remote roots until decided separately.
- More administrator work (confirm roots) in exchange for no silent mistakes.
- Rejected: a side table with media left in the source library (touches every `library_id` query), content-based auto-splitting (wrong boundaries would silently hide or reset study state) and keeping only the source-level view (does not solve the problem).

## Implementation slices

Each slice is its own small PR; slices 2 to 4 need independent review (persistence, identity, authorization, media confinement). The picker needs no heuristic, so the heuristic is the last and optional slice.

1. Core, pure: partition a `ScanOutcome` by prefixes (overlap rejection), with synthetic fixtures: flat, grouped, multi-group, direct-media, mixed-depth, duplicate-name, empty.
2. Core, persistence: `library_root` migration and the transactional promote and demote. Tests prove study state, notes, flashcards and favorites survive both directions.
3. Server: admin-only browse endpoint (one level of a source, folders first, with a per-folder media count of that level only; the path must stay inside the source), roots API, per-root scan, catalog assembly, media and access resolution with prefix confinement. Security tests: no escape from the prefix or the source in browse, no access bypass, no cross-root reads.
4. Web: in Remote sources, a "Choose courses" folder browser with the two picks and a review before confirming; `tr(en, ptBr)` copy and component tests.
5. Optional: highlight suggested folders in the browser from the bottom-up rule.
6. Docs and a final independent review.

## Validation and rollback

- Synthetic fixtures: flat, grouped, multi-group, direct-media, mixed-depth, duplicate-name, empty and ambiguous trees.
- Security tests: a virtual course cannot read outside its prefix (`..`, encoded separators, case and Unicode variants), cannot use another user's access, and cannot bypass source-level access.
- Cover media/range/transcode, favorites, notes, bookmarks, PDF annotations, flashcards, search and course removal through a root.
- Independent read-only review of persistence, identity, authorization and media confinement before merge.
- Rollback: the table is additive; dropping it restores the single-source view.
