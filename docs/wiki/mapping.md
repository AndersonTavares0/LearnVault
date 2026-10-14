# GitHub Wiki mapping

The repository is the source of truth. `scripts/sync-wiki.sh` mirrors the following
documents into the GitHub Wiki after a change reaches `main`.

| Wiki page | Canonical source |
| --- | --- |
| `Home` | `docs/wiki/home.md` |
| `Getting-Started` | `docs/wiki/getting-started.md` |
| `Architecture` | `docs/architecture/overview.md` |
| `Roadmap` | `docs/product/roadmap.md` |
| `Development-and-Governance` | `docs/governance/overview.md` |
| `Operations` | `docs/operations/distribution.md` |

The workflow owns only these six pages. Other Wiki pages may be maintained manually.
Do not edit a managed page in GitHub Wiki: the next sync replaces it with this source.
