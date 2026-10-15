# Distribuição e alvos de release

O LearnVault é uma aplicação Go portátil com uma SPA SvelteKit embutida. Docker é um
adaptador de deploy para servidores, não um requisito de runtime nem o modelo principal
de desenvolvimento.

## Matriz de releases

| Alvo | Artefato | Uso | Status |
| --- | --- | --- | --- |
| Linux | binário nativo | Servidor, terminal e instalações leves | Fundação disponível |
| Windows | `.exe` | Execução local direta | Planejado |
| Windows | `.msi` | Instalação e atualização gerenciadas | Planejado |
| Debian/Ubuntu | `.deb` | Gerenciamento nativo de pacotes | Planejado |
| Fedora/RHEL | `.rpm` | Gerenciamento nativo de pacotes | Planejado |
| Desktop Linux | Flatpak | Distribuição desktop isolada | Planejado |
| OCI | Imagem Docker/Podman | Deploy opcional em servidor | Disponível |

## Limites de build

- O comando Go em `cmd/learnvault` é a fonte da verdade para builds executáveis.
- A SPA é construída uma vez e embutida no executável.
- Receitas de pacotes ficarão em `deploy/packages/` quando forem implementadas.
- Definições de containers ficam em `deploy/container/`, `deploy/compose/` e `deploy/quadlet/`.
- Containers não podem definir comportamento que o binário não possui.
- O CI deve construir cada artefato de plataforma antes de montar instaladores.
- Release Please controla changelog e metadados do GitHub Release.
- O workflow de publicação adiciona binários versionados e `SHA256SUMS` ao release existente.

## Ferramentas planejadas

A ferramenta de empacotamento será escolhida após o primeiro marco de binário nativo. Ela deve
suportar builds versionados reproduzíveis sem tornar Docker um requisito oculto. A decisão final
será registrada em um ADR em `docs/architecture/adr/`.

## Modelo de configuração

Instalações nativas usam variáveis de ambiente ou arquivo de configuração adjacente para
`DATA_DIR`, `CACHE_DIR`, `LIBRARY_DIR`, `LISTEN_ADDR`, `PORT` e o bootstrap de autenticação.
A biblioteca física permanece externa e somente leitura em todas as plataformas. Instaladores
nunca devem copiar a biblioteca para o diretório da aplicação ou para backups.
