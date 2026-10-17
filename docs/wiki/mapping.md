# Mapeamento da Wiki do GitHub

O repositório é a fonte da verdade. `scripts/sync-wiki.sh` espelha os documentos abaixo
na Wiki do GitHub depois que uma mudança chega à `main`.

| Página da Wiki | Fonte canônica |
| --- | --- |
| `Home` | `docs/wiki/home.md` |
| `Getting-Started` | `docs/wiki/getting-started.md` |
| `Architecture` | `docs/architecture/overview.md` |
| `Roadmap` | `docs/product/roadmap.md` |
| `Requirements` | `docs/product/requirements.md` |
| `Development-and-Governance` | `docs/governance/overview.md` |
| `Operations` | `docs/operations/distribution.md` |

O workflow gerencia somente estas sete páginas. Outras páginas da Wiki podem ser mantidas
manualmente. Não edite uma página gerenciada diretamente na Wiki: a próxima sincronização
a substituirá por esta fonte.
