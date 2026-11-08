# Rust Artifact Soak Report (G2)

> Factual record of an execution. Never include credentials, tokens, cookies, database
> dumps, course media, or other private data in this report.

## Identification

- Version reported by binary: `______________________________`
- Draft tag: `______________________________`
- Source `run_id`: `______________________________`
- Declared SHA-256: `______________________________`
- Verified SHA-256: `______________________________`
- Start timestamp (with timezone): `______________________________`
- Total duration: `______________________________`
- Host / OS / architecture: `______________________________`
- Artifact acquisition method: `______________________________`

## Data Provenance

- Synthetic library or copy of real library used (do not attach data):
  `____________________________________________________________`
- Was `migrate-go` executed? `yes / no`
- If yes, paths used (record paths only; do not attach files):
  - input: `__________________________________________________`
  - backup: `________________________________________________`
  - output: `________________________________________________`

## Verification Checklist

| Check | Observed Result | Expected Result | Passed? |
| --- | --- | --- | --- |
| Health HTTP 200, `cache-control: no-store`, correct version | | 200; `no-store`; version matches ID above | |
| Authentication | | Login succeeds and session cookie persists across requests | |
| Catalog | | Hierarchical course catalog returned without errors | |
| Media Streaming with `Range` | | Partial content `206` for requested byte range | |
| Persistence across restart | | Data, progress, and sessions persist across restarts | |
| Absence of unexpected 4xx/5xx | | Zero unexpected error responses | |

## Resource Utilization

- Approximate CPU: `______________________________`
- Approximate RSS: `______________________________`
- Process restarts (count and timestamps): `______________________________`

## Failures and Anomalies

List each occurrence with timestamp and observable evidence (e.g. command output or HTTP status), without secrets or private data:

1. `__________________________________________________________________________`
