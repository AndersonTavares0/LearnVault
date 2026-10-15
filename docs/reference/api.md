# API reference

The API is served by the Go executable. All routes below use JSON unless noted
otherwise. Authentication uses an HttpOnly, SameSite=Strict session cookie.

## Health and session

| Method | Route | Authentication | Purpose |
| --- | --- | --- | --- |
| `GET` | `/api/healthz` | No | Process and database health |
| `POST` | `/api/login` | No | Accepts `{ "username": "...", "password": "..." }` |
| `POST` | `/api/logout` | Session | Invalidates the current session |
| `GET` | `/api/me` | Session | Returns the current account |

## Catalog and media

| Method | Route | Authentication | Purpose |
| --- | --- | --- | --- |
| `GET` | `/api/catalog` | Session | Returns the persisted category/course/module/lesson tree |
| `POST` | `/api/admin/catalog/rescan` | Admin session | Explicitly scans `LIBRARY_DIR` and replaces the catalog |
| `GET`, `HEAD` | `/api/media?path=...` | Session | Streams an approved library file with HTTP Range support |

The scanner reads directory entries and metadata only. It ignores downloader
temporary directories, classifies recognized media/subtitles/materials, and treats
`.ts` as video only when it is larger than 1 MiB to avoid confusing TypeScript with
MPEG-TS. The physical library is external, mounted read-only, and never modified.

For architecture and deployment boundaries, see
[`../architecture/overview.md`](../architecture/overview.md).
