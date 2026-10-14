# Data safety policy

- `/mnt/cursos` is external, read-only user data. Never inspect, scan, write, move,
  rename, duplicate, or include it in a backup during agent work.
- Tests use `t.TempDir()`, fake fixtures under `testdata/`, or in-memory fakes only.
- Never commit generated data, credentials, personal study data, course media, database
  contents, or homelab secrets.
- Preserve authenticated media access and the read-only library boundary in every change.
