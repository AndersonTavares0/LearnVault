# Next design pass: a study shelf

**Status:** Proposal for review. [DESIGN.md](../../DESIGN.md) describes the shipped interface.

## Direction

Make the Library feel like a shelf of the learner's own courses. Keep the next unfinished lesson
easy to resume, then let recognizable covers, readable titles and progress do the work of
distinguishing courses. Use a quiet, ruled layout for denser study surfaces such as the curriculum,
notes and search results.

Start with the shipped Paper & Petrol palette and one accent per screen. Keep Geist for controls
and body text. Evaluate the already bundled Literata for large headings in the Library preview;
leave small screens, controls and everyday reading text in the existing UI face. Use the built-in
cover pipeline for originals and fallbacks, with one consistent treatment per view.

```text
Desktop: [collapsible navigation] [Library title and utilities]
                                 [Resume lesson, when available]
                                 [Filters and view] [Course shelf]

Phone:   [Library title and utilities]
         [Resume lesson, when available]
         [Filters] [Course shelf]
         [Existing bottom navigation]
```

## Scope and sequence

1. **Library and course.** Review a preview using a populated catalog, long titles, missing covers
   and untouched courses. Refine the hierarchy of Resume, catalog controls, covers and progress.
   Keep course cards and navigation working as on `main`; preserve saved display and sorting choices.
2. **Player, Review, Progress, Notes and Search.** Extend the accepted type, spacing and divider
   rules, prioritizing visible player controls, titled results, readable notes and clear loading,
   empty and error states. Keep the existing routes, study actions and collapsible desktop rail.
3. **Settings and shell.** Confirm themes, language and saved appearance settings still apply across
   both builds. Update `DESIGN.md` and reconcile any changed brand guidance in `PRODUCT.md`.

Local `design/cornell` at `c4ab5292` is a visual reference for the shelf, ruled rows and typographic
hierarchy. Build on current `main` rather than merging that branch: it removes standalone Progress
and History pages and assigns a different schema to `V22`. `V22` has already been assigned on
`main`. Collections, transcripts, new routes, preset replacements and database changes are outside
this design pass.

## Acceptance

- Exercise sign-in, resume, playback and captions, notes, flashcard review, progress and search with
  synthetic data. Existing links, study records and preferences remain usable.
- Compare before and after at 375 and 1440 CSS pixels, in both themes and in `pt-BR`. Check the
  English default separately. Include populated and empty states, keyboard focus, no document
  overflow, and text/control contrast against the [accessibility checklist](accessibility.md).
- Run SPA type checks, lint, unit tests, web and desktop frontend builds, and browser end-to-end
  tests. Exercise the native Tauri shell if its development dependencies are available.
- Keep captures outside Git. Have a reviewer inspect the diff and captures and record any untested
  platforms or states before requesting merge.
