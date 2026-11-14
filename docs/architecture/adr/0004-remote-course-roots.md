# ADR: Virtual course roots for remote sources

## Status

Proposed (2026-11-14). Needs maintainer approval before any code. Refs #509, #441.

## Context

A remote WebDAV/SMB source is one registered library today. Verified in the code:

- The source `id` is the `library_id`, the catalog `course_id` and the media route `course` parameter (`crates/server/src/catalog.rs`, `crates/server/src/media.rs`). `build_catalog(database, &source.id)` turns the whole source into one course node.
- `media` rows are keyed by `(library_id, relative_path_key)` and study state, notes, bookmarks, annotations and flashcards hang off `media_id` (`crates/core/src/migrations.rs`). Favorites and per-user course access are keyed by `course_id`. Manual ordering is keyed by the source id.
- The recursive scan already keeps relative paths, so a source holding grouping folders and several real courses appears as one large course tree.

Folder names, language and nesting depth must not be hardcoded, and original files stay read-only.

## Decision

Use **explicit virtual course roots** (option 1 of #509), with content analysis only as a suggestion.

1. A source keeps its physical identity (`library_id`, `media_id`, study state). A new additive table maps an opaque `course_id` to `(source_id, relative_prefix)`. The prefix is the full relative path, so equal folder names under different parents never collide.
2. A source with no roots behaves exactly as today: one course whose `course_id` is the source id. Existing IDs, favorites, access lists, ordering and study state stay valid.
3. Roots are created only by an administrator. The scan may show suggested roots (for example, folders that directly contain lessons) with a confidence level, but nothing is applied without confirmation. Ambiguous trees stay as the single source view.
4. Media, range and transcode requests resolve `course_id` to the source, then require the requested path to be inside the prefix after normalization. Per-user access is checked on the virtual `course_id`.
5. Rescan keeps a root while its prefix exists. A missing prefix marks the root unavailable and keeps study history. A rename or move is never guessed: an administrator re-points the root. Removing a root removes only the mapping, never source files or study rows.

## Consequences

- No change to `media`, `study_state` or other existing tables; one new table and API for roots.
- Catalog assembly splits one source into several course nodes, so ordering, search indexing and covers need a per-root key. Covers stay unsupported for remote roots until decided separately.
- More administrator work (confirm roots) in exchange for no silent mistakes.
- Rejected: content-based auto-splitting (wrong boundaries would silently reset or hide study state) and keeping only the source-level view (does not solve the problem).

## Validation and rollback

- Synthetic fixtures: flat, grouped, multi-group, direct-media, mixed-depth, duplicate-name, empty and ambiguous trees.
- Security tests: a virtual course cannot read outside its prefix (`..`, encoded separators, case and Unicode variants), cannot use another user's access, and cannot bypass source-level access.
- Cover media/range/transcode, favorites, notes, bookmarks, PDF annotations, flashcards, search and course removal through a root.
- Independent read-only review of persistence, identity, authorization and media confinement before merge.
- Rollback: the table is additive; dropping it restores the single-source view.
