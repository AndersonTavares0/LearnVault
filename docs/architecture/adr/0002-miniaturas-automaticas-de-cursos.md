# ADR 0002: Generate course thumbnails in the WebView

**Status:** Accepted

## Context

The current covers are generated from the course title. A thumbnail extracted from the first
available video in the lesson sequence could make the cards more recognizable, but LearnVault
must keep working offline on the desktop and must never write to the course folders. The desktop
delivers media through its own Tauri protocol, whose origin differs from the SPA's and varies by
platform; exporting a frame from a canvas requires a clean origin and therefore strict CORS. The
cache must be disposable, bounded and separated per profile.

## Decision

Use the WebView's native video decoder and a canvas to capture the frame, without adding FFmpeg or
any other executable or codec dependency. Generate on demand when the card enters the visible area;
store JPEGs in local IndexedDB, keyed by profile, course, media, size, `mtime_seconds` and
generator version. On the desktop, the media protocol will emit `Access-Control-Allow-Origin` only
when the request's `Origin` is exactly the origin of the Tauri window that started it; no wildcard
and no credentials. Failures of origin, CORS, decode, canvas or cache keep the cover generated from
the title. The precedence will be the custom cover, when available, then the automatic thumbnail
and, last, the generated cover; implementing manual upload stays outside this ADR and belongs to
issue #237.

## Consequences

- The desktop and the server use the decoders they already need to play video, with no binary,
  packaging cost or new codec license.
- Extraction is lazy and bounded: at most two simultaneous tasks, a frame at 10% of the duration
  capped at 10 seconds, an image of up to 320×180, JPEG, a maximum of 256 KiB per image, 256
  images and 64 MiB per profile; remove old entries when the limits are exceeded.
- The media revision (`size` and `mtime_seconds`) must be available in the catalog DTO to
  invalidate the cache without sending absolute paths. File paths are never stored in the cache
  nor sent over IPC.
- IndexedDB can be cleared by the browser or environment. The thumbnail is derived and
  disposable; if it cannot be created or read, the card keeps using the existing generated cover.
- The CORS origin is obtained from the window identified by the protocol context and compared with
  the `Origin` header; responses do not reflect arbitrary origins, do not allow credentials and
  must vary by `Origin`. The capture uses anonymous CORS mode.

## Validation and rollback

- Test with synthetic videos and temporary profiles and caches: a valid frame/canvas,
  format/size, cache hit, invalidation on a change of size, mtime or version, limit and eviction,
  and profile isolation.
- Test desktop CORS: only the exact origin receives ACAO; a missing, different, malformed or
  arbitrary origin does not receive ACAO and does not allow exporting the canvas. The media
  request is still served so that normal playback is not affected; exercise capture and fallback
  in the HTTP and desktop builds.
- Check cards visually in desktop and mobile viewports and confirm the fallback after decode or
  CORS fails; run the SPA and desktop checks documented in the playbook.
- Before enabling the thumbnail on each supported desktop platform, validate in that platform's
  real WebView that the protocol response allows exporting the canvas. If it fails, keep the
  fallback; do not widen the allowlist or weaken the CSP to work around the failure.
- Rollback: remove the generation/read path and the IndexedDB table/version. No migration or
  restoration of course data is needed, because the library is never modified and the cache is
  disposable.
