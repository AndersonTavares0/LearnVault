# Data safety policy

- The user's course library is external, read-only data. Never inspect, scan,
  write, move, rename, duplicate, or include it in a backup during agent work.
- Tests use `t.TempDir()`, fake fixtures under `testdata/`, or in-memory fakes only.
- Never commit generated data, credentials, personal study data, course media, database
  contents, or private infrastructure secrets.
- Preserve authenticated media access and the read-only library boundary in every change.
