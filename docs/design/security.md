# User security and privacy in the redesign

The React app (epic #347) must not weaken anything the server already enforces. It should also make
safe behaviour the default for the people using it. Every phase PR checks the relevant items. The
cutover (#340) also requires a security review of `apps/web-next`.

## What the server already enforces (do not regress)

- Same-origin check on every state-changing request (CSRF), in `crates/server/src/auth.rs`.
- Per-username login rate limiting (`allow_login_attempt`).
- Random session tokens in a cookie the SPA cannot read. A new token is issued when credentials change.
- CSP `default-src 'self'`, scripts only from `'self'` plus hashes, styles only from `'self'`
  plus hashes, and `frame-ancestors 'none'`.
- `x-frame-options: DENY`, `x-content-type-options: nosniff`, `referrer-policy: no-referrer`.
- Desktop CSP in `crates/desktop/tauri.conf.json`. IPC and the custom media/cover schemes are the
  only allowed sources.

## Code rules (enforced by lint or tests)

- [ ] No `dangerouslySetInnerHTML` (ESLint `react/no-danger`). Notes, titles, and file names
      always render as text.
- [ ] No runtime `<style>` or `<script>` injection and no `eval`. Libraries that need them
      are rejected.
- [ ] No third-party requests: no CDN, web fonts, analytics, or telemetry. Fonts are bundled. The
      only outbound request is the opt-in update check, which the server makes, not the browser.
- [ ] No secrets or session data in `localStorage`, `sessionStorage`, or IndexedDB. Browser storage
      holds preferences and caches only.
- [ ] URLs built from user or file data use `encodeURIComponent`, and navigation uses the router.
      Links to external sites use `rel="noopener noreferrer"`.
- [ ] Dependencies are pinned, the lockfile is frozen, and pnpm `minimumReleaseAge` applies. The dependency
      audit runs in CI. Each new runtime dependency is justified in its PR.
- [ ] Desktop: "Open in system player" accepts a media id only, never a path or URL.
      Tauri capabilities stay minimal.

## Sign-in and session

- [ ] A single generic error for a wrong username or password, so the app does not reveal
      whether an account exists.
- [ ] When rate-limited, say so and say when to try again. Do not show the generic error.
- [ ] `autocomplete="username"` and `autocomplete="current-password"` are set, paste is allowed,
      and there is a show/hide password toggle. Password change uses `new-password`.
- [ ] An expired session (401) returns to sign-in and then to the same route. Unsaved note
      drafts survive.
- [ ] The login screen never lists or suggests other users.

## Shared computers

- [ ] Sign out clears per-user browser data: the avatar cache, player preferences, the focus outbox,
      and per-user thumbnail entries.
- [ ] If the focus outbox holds unsynced sessions, sign out warns first and offers to sync, instead
      of silently discarding them.

## Admin and destructive actions

- [ ] Removing a user or a course opens a dialog that states the impact (lessons, progress,
      notes affected) and requires explicit confirmation.
- [ ] New users get no course access by default. Access is shown as a clear per-course table.
- [ ] Avatar and cover uploads are checked for type and size in the browser before upload. The
      server remains the authority.
- [ ] Non-admin users never see server filesystem paths. Catalog paths appear to be relative to the
      course root. Confirm this in Phase 2 (#336).
- [ ] Update settings and history are admin-only.

## Later, with backend changes (#341)

- Sign out all other sessions.
- Recent sign-in history (time, client), visible to the user.
