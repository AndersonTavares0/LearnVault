# Security model

LearnVault is a self-hosted study space. This document describes what the code enforces today, which
tests hold those guarantees in place, and which limits are accepted on purpose. It covers the Rust
server, the React SPA it serves, the Tauri desktop application, and the shared core.

Security fixes target the latest release from the default branch. Report vulnerabilities through the
private channel described in [`.github/SECURITY.md`](../../.github/SECURITY.md); never open a public
issue.

## 1. Threat model

**Assets**

- Course files, which are read-only input and never a write target.
- Local account credentials and session cookies.
- Study data: progress, notes, PDF annotations, bookmarks, flashcards, reviews, preferences.
- The server filesystem layout outside the course library.
- The release channel used by the desktop updater.

**Adversaries considered**

- An unauthenticated client on the network.
- An authenticated user trying to read another user's data, a course they were not granted, or a file
  outside the library root.
- A crafted course tree: hostile file names, symlinks, junctions, oversized or malformed sidecars.
- A compromised or malicious dependency.
- Someone with access to a shared browser profile.

**Out of scope**

Host compromise, a malicious administrator (an admin can already read every course and manage every
account), traffic analysis, and denial of service beyond the request limits and rate limiting
described below. The desktop application is single-user and has no accounts.

## 2. Authentication and sessions

- Passwords are hashed with Argon2id at 64 MiB memory, 2 iterations, 1 lane, 32-byte output, stored as
  a PHC string. Verification returns a boolean and never leaks hash details
  (`crates/core/src/auth.rs`).
- Username and password bounds are validated before hashing, so oversized input never reaches Argon2.
- Every login runs the same-origin check and a per-source rate limit: 5 failed attempts per 60-second
  window, with the tracking table capped at 1024 sources. Excess requests receive `429 try again
  later` (`crates/server/src/state.rs`).
- The client address comes from forwarding headers only for explicitly configured trusted proxies;
  otherwise the socket address is used, so a client cannot spoof its way out of the limit.
- A wrong username and a wrong password return the same generic `invalid credentials`, so the app
  does not reveal which accounts exist.
- Session tokens are 32 random bytes. Only the SHA-256 hash is persisted; the raw token exists only
  in the cookie.
- The cookie is `learnvault_session` with `Path=/; HttpOnly; SameSite=Strict`, plus `Secure` when the
  server is configured for TLS. The SPA cannot read it.
- Changing credentials deletes every session of that account and issues a fresh token, so a password
  change signs out other devices.
- An account with `mustChangeCredentials` can only reach the credential-change flow; other endpoints
  reject the session until the change is complete.
- Sessions carry an expiry, are filtered on every lookup, and expired rows are discarded.

## 3. CSRF and transport

- There is no CSRF token. Protection relies on three layers: `SameSite=Strict` cookies, a same-origin
  check on every state-changing handler (login, credentials, logout, user management, course access),
  and the absence of CORS headers.
- A request without a matching `Origin` is rejected with `403`, not accepted by default.
- Every `/api` response is `no-store`; authentication and update responses use `private, no-store`.
- Network deployments must terminate HTTPS at a reverse proxy. With TLS configured, cookies carry
  `Secure`.

## 4. Authorization and user isolation

- The owner is always taken from the session, never from a request body or query parameter. Handlers
  forward the authenticated `user_id` to the core.
- Study data is scoped per owner in the SQL itself (`WHERE owner_id = ?`), not filtered afterwards in
  the UI: progress, notes, PDF annotations, bookmarks, flashcards, review logs, favorites, focus
  sessions, and player preferences.
- Search returns notes only for their owner. Catalog results are limited to courses the user may
  access.
- Course access is per user. With no `user_course_access` rows the user sees every course; with rows,
  only the listed courses, and the sentinel `__none__` means no access. New users start with no
  access until an admin grants it.
- Admin-only operations: listing, creating, and deleting users, setting course access, catalog
  rescan, course import and removal, covers, manual ordering, media impact and removal, and database
  backup download. These check the admin role in the handler.
- The update check requires a valid session. Non-admins never see the update surface in the SPA.

## 5. Media and the course library

- The client never sends a filesystem path. Media endpoints accept a `library_id` and a `media_id`;
  the core revalidates the stored relative path, confines it to the library root, and opens a
  read-only descriptor that the adapter streams (`crates/core/src/media.rs`).
- On Unix every path component is opened relative to its parent directory descriptor and symlinks are
  refused. On Windows the file is opened relative to a directory capability, so an intermediate
  junction cannot redirect access outside the root.
- Hard links to files outside the root are indistinguishable from ordinary files. This is a known,
  documented limit of the confinement model.
- Range requests are supported for streaming. No code path writes, renames, moves, or deletes inside
  the course library; the scanner and indexer are read-only.
- Subtitle sidecars are served through the same authenticated media route with the same confinement,
  not through a separate static handler.
- Remote sources (WebDAV, SMB) keep this contract: read-only access, credentials encrypted at
  rest, never logged, and never echoed back in API responses. WebDAV refuses cross-host redirects
  and SMB disables DFS referrals for the same reason: credentials must not follow a server-chosen
  redirect to another host.

## 6. Browser hardening

- Each response carries a Content Security Policy: `default-src 'self'`; `script-src 'self'` plus
  per-build inline hashes; `style-src 'self' 'unsafe-hashes'` plus the same hash mechanism;
  `img-src 'self' data: blob:`; `media-src 'self' data: blob:`; `frame-ancestors 'none'`.
- Additional headers: `x-content-type-options: nosniff`, `x-frame-options: DENY`,
  `referrer-policy: no-referrer`.
- `dangerouslySetInnerHTML` is rejected by ESLint (`no-restricted-syntax` in
  `apps/web-next/eslint.config.js`). Notes, titles, and file names render as text. Markdown is parsed
  into React elements by `apps/web-next/src/lib/markdown.ts`; there is no raw HTML rendering path.
- There are no third-party requests: no CDN, web fonts, analytics, or telemetry. Fonts are bundled.
  The only outbound request belongs to the server's opt-in update check, never to the browser.
- Browser storage holds preferences and caches only. Session tokens and secrets are never written to
  `localStorage`, `sessionStorage`, or IndexedDB.
- URLs composed from user or file data are encoded, and external links use
  `rel="noopener noreferrer"`.
- Signing out clears per-user browser data: avatar cache, player preferences, the focus outbox, and
  per-user thumbnail entries. If an unsynced focus outbox exists, sign-out warns first instead of
  discarding it.

## 7. Desktop application (Tauri)

- The webview CSP in `crates/desktop/tauri.conf.json` allows `'self'`, the IPC scheme, and the custom
  `learnvault-media` and `learnvault-cover` schemes only. `object-src 'none'` and `base-uri 'self'`.
- Capabilities in `crates/desktop/capabilities/default.json` are an explicit allowlist: core window
  controls plus the application's own commands. There is no shell, process, HTTP, broad filesystem,
  dialog, store, remote, or direct updater permission.
- The study owner is fixed in Rust. IPC commands never accept an owner from the webview.
- "Open in system player" accepts a media id only, never a path or URL.
- The updater consumes a signed manifest; pre-releases are ignored, so a beta never moves a stable
  installation.
- The desktop database lives in the application data directory and is not shared with the server.

## 8. Secrets and configuration

- `.env` files, databases, course data, and generated media stay outside version control. CI runs
  repository hygiene checks (`scripts/repo-hygiene.mjs`) on every pull request.
- Initial credentials can be supplied through a file instead of a command line, keeping them out of
  process listings and shell history.
- Passwords are only stored hashed, session tokens only as hashes, and credentials never appear in
  logs or error responses.
- Update and backup responses are served with `private, no-store`.

## 9. Supply chain

- `Cargo.lock` and `pnpm-lock.yaml` are committed and installs use frozen lockfiles. Rust and Node
  versions are pinned by `rust-toolchain.toml` and `packageManager`.
- CI runs CodeQL, dependency review on pull requests, `cargo audit --deny unsound`, and
  `pnpm audit --audit-level=low`.
- Three advisories are accepted with documented rationale rather than silently ignored:
  - `GHSA-g3hv-75wg-m249` (minimatch, dev tooling) is ignored in `pnpm-workspace.yaml`; forcing the
    patched major breaks `eslint-plugin-jsx-a11y`.
  - `GHSA-gcqx-w48m-w2h4` (deepmerge) is ignored for the same reason: it is reachable only through the
    `shadcn` component CLI, no patched release exists, and it never reaches the shipped bundles.
  - `RUSTSEC-2024-0429` (glib, reached through Tauri's GTK stack) is ignored in `.cargo/audit.toml`;
    the patched release needs a GTK major upgrade that no current Tauri version provides.
- Each new runtime dependency must be justified in its pull request, and Dependabot alerts stay open
  until the upgrade is actually possible.

## 10. Data, privacy, and retention

- There is no analytics, telemetry, or crash reporting. The instance contacts GitHub only for the
  release check, and the desktop only for its update manifest.
- All study data lives in the local SQLite database. Nothing is uploaded anywhere by default.
- Backups are generated on demand and downloadable by an admin only.

## 11. Accepted risks and known gaps

- **Live in-memory rate limiting.** Counters reset on restart and are not shared between processes;
  running several instances behind a proxy weakens the limit.
- **No CSRF token.** The design depends on `SameSite=Strict` plus the `Origin` check. A deployment
  that relaxes the cookie policy would weaken it.
- **No "sign out everywhere" and no sign-in history.** A password change does revoke all sessions.
- **User deletion keeps study rows.** Deleting a user removes the sessions and the course-access
  rows, and the account becomes unreachable, but owner-keyed study rows remain on disk. Erasing them
  requires a manual cleanup or a future deletion job.
- **No encryption at rest.** The SQLite file relies on OS permissions and, ideally, full-disk
  encryption.
- **Hard links escape the media confinement check** in the sense that a hard link to a file outside
  the root looks like a normal file inside it.
- **Desktop has no accounts and no at-rest encryption.** Anyone with access to the machine profile
  can read the local database.
- **Three Dependabot advisories remain open** for the reasons in section 9.
- **Remote source coverage is uneven.** WebDAV and SMB credentials are encrypted at rest with the
  same store, but remote sources have no custom cover, manual ordering, search indexing or
  transcoding. SMB uses NTLM only (no Kerberos) and no end-to-end test against a real SMB server
  runs in CI; URL validation, error mapping and credential non-disclosure are covered by unit tests.

## 12. Verification

The guarantees above are covered by automated checks; a pull request that touches these areas must
keep them green.

| Area | Evidence |
| --- | --- |
| Login, cookie flags, CSRF, rate limiting, generic errors | `crates/server/tests/auth_http.rs` |
| CSP and security headers on the SPA and API | `crates/server/tests/spa_http.rs` |
| Owner isolation for notes, progress, bookmarks, flashcards, search | `crates/core/tests/*`, `crates/server/tests/*_http.rs` |
| Media confinement | `crates/core/tests/media.rs` |
| Catalog access filtering | `crates/server/tests/catalog_http.rs` |
| Desktop IPC and capability surface | `crates/desktop/tests/ipc.rs`, `crates/desktop/capabilities/default.json` |
| No raw HTML injection in the SPA | `pnpm --filter learnvault-web-next lint` |
| Dependency and supply-chain checks | `.github/workflows/ci.yml` |
| No secrets or generated media in git | `scripts/repo-hygiene.mjs` |

## 13. Reporting

Follow [`.github/SECURITY.md`](../../.github/SECURITY.md). Do not open a public issue. Include the
affected version, reproduction steps, impact, and logs with secrets removed, and never include course
files, passwords, session cookies, tokens, or private network details.
