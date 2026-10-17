# Migração do SQLite Go para Rust

O núcleo Rust oferece uma importação explícita e limitada por biblioteca. Ela não é executada
automaticamente pelo servidor e não modifica o banco Go nem a biblioteca física.

`learnvault_core::import_go_database(LegacyImportOptions)` recebe `input`, `output`, `backup` e
`library_id`. O backup é criado primeiro; a saída fica em arquivo temporário e só é promovida
depois de validações de integridade, chaves estrangeiras e contagens. Os destinos devem ser novos
e distintos.

São importados usuários, sessões, arquivos de catálogo, remoções de mídia representáveis e
progresso. Caminhos são normalizados para `/`, IDs de usuários e mídia são gerados no banco Rust.
Como o schema Rust exige uma mídia para cada tombstone e não possui a coluna Go `is_media`, uma
remoção órfã, com `is_media` nulo ou com `is_media = 0` faz a operação falhar fechadamente; ela
não é descartada nem convertida em uma mídia fictícia. Colisões, papéis desconhecidos, timestamps
inválidos e valores fora dos limites também falham sem promover a saída.

Execute a migração pelo CLI, informando explicitamente os quatro destinos:

```text
learnvault-server migrate-go --input <go.sqlite> --output <rust.sqlite> --backup <go.sqlite.backup> --library-id <id>
```

Esse comando não inicia o listener HTTP e não exige configuração de biblioteca, endereço,
interface web ou credenciais administrativas. Para obter um snapshot estável, execute-o com o
escritor Go parado ou em estado quiescente. Conserve o backup e confirme a saída com
`Database::open` antes de usá-la.

## Gate de cutover do Compose self-hosted

Antes de trocar o serviço, identifique se o `${DATA_DIR}` alvo contém um banco Go v8 ou um banco
Rust. Os formatos são incompatíveis e nunca podem compartilhar o mesmo volume. Em uma instalação
in-place, o `../../data` existente pode conter um banco Go; o servidor Rust falhará fechado nesse
caso. O inverso também é obrigatório: um volume anteriormente usado pelo Compose Rust contém um
banco Rust e não pode ser entregue ao runtime Go.

O gate operacional é:

1. Preserve o volume Go e pare o serviço Go; bloqueie novos escritores e confirme a quiescência
   antes de construir ou iniciar o novo serviço. Construa a imagem com o Compose (`up --build`),
   mas não a promova ainda.
2. Execute o comando `learnvault-server migrate-go` documentado acima, informando
   explicitamente `--input`, `--backup`, `--output` e `--library-id`. Use caminhos novos e
   distintos para entrada, backup e saída; a saída deve ser preparada em um volume Rust separado.
3. Capture como evidência o checksum inalterado da origem, os contadores do relatório e a
   existência de saída e backup, confirmando que os dois últimos são distintos.
4. Mantenha os volumes pareados: conserve o volume Go e o backup da importação; para rollback,
   pare o Rust e descarte o volume de dados Rust, nunca reutilize seu banco no Go.

O RPO do rollback é exatamente o instante do backup da importação. Toda escrita Rust posterior
será perdida; não existe replicação nem reconciliação automática. Promova o Rust somente após a
importação validada, a origem preservada e um smoke test bem-sucedido com a biblioteca somente
leitura. Demova-o se o healthcheck, a abertura do banco, a autenticação, o catálogo ou a
persistência falharem; pare o Rust e retorne ao volume Go preservado, sem entregar o volume Rust
ao runtime Go.

## Ensaio de cutover self-hosted (fixture sintética)

`scripts/verify-go-to-rust-cutover.sh` é um **ensaio manual, não publicante e apenas com
fixtures sintéticas**. Ele não roda na CI, não publica imagens e **não é um cutover de
produção nem evidência para volumes reais**. O objetivo é exercitar, de ponta a ponta, a
sequência operacional abaixo.

Escopo estrito:

- constrói o `Containerfile` Go de referência e o `Containerfile.rust` experimental;
- gera um banco Go v8 real subindo o runtime Go contra uma biblioteca sintética somente
  leitura e catalogando a fixture; nunca constrói nem empacota um banco binário;
- **quiescência**: para o escritor Go (`stop`) e confirma que ele não está mais rodando
  antes de usar o banco como entrada da importação;
- volumes e caminhos distintos: o Go e o Rust usam diretórios de dados separados e o ensaio
  falha fechado se eles resolverem para o mesmo caminho; nenhum runtime é montado contra o
  diretório de dados do outro;
- cria caminhos de saída e backup de importação novos e distintos sob a árvore da fixture
  Rust;
- importa com `migrate-go --input/--backup/--output/--library-id` e confere o relatório, o
  checksum inalterado da origem e a existência e distinção de saída e backup;
- sobe o OCI Rust experimental endurecido apenas contra a saída Rust preparada como
  `/data/learnvault.sqlite` e a mesma `/library:ro`;
- inclui um caminho negativo fail-closed (CLI malformada) que prova que nenhuma saída é
  promovida;
- normaliza o ambiente do diretório temporário **antes de criar qualquer arquivo**: valores
  de `TMPDIR`/`TEMP`/`TMP` apontando para `/mnt` ou para uma árvore `cursos` são recusados e
  descartados, de modo que a fixture nunca nasce nesses locais;
- usa credenciais sintéticas fixas, **nunca configuráveis pelo operador**: se
  `ADMIN_USERNAME` ou `ADMIN_PASSWORD` estiverem definidos no ambiente, o ensaio falha fechado
  antes de iniciar qualquer contêiner, e as credenciais nunca são registradas em log.

Comando:

```sh
./scripts/verify-go-to-rust-cutover.sh
# ou: CONTAINER_ENGINE=podman ./scripts/verify-go-to-rust-cutover.sh
# KEEP=1 mantém a fixture temporária para depuração
```

Verificações do ensaio:

- `/api/healthz` responde `200 {"status":"ok"}`;
- requisição não autenticada a `/api/catalog` responde `401`;
- autenticação do admin migrado (sessão nova) com as credenciais sintéticas;
- catálogo importado contendo a lição sintética conhecida;
- requisição de faixa (`Range: bytes=0-15`) de mídia retorna `206`;
- persistência de estudo (posição gravada e relida) e persistência após reinício;
- execução como uid 10001, sem novos privilégios e com todas as capabilities removidas;
- sistema de arquivos raiz somente leitura e biblioteca somente leitura;
- remoção do contêiner ao final.

### Rollback e RPO após escritas no Rust

Depois que o Rust escreve (sessões, progresso de estudo), o banco Rust passa a divergir do
Go. Os dois volumes são **pareados**: mantenha o volume Go intacto e o backup Go criado pela
importação. O rollback exige parar o Rust e restaurar o volume Go a partir desse snapshot;
qualquer escrita feita no Rust depois do backup é perdida. O RPO é, portanto, o instante do
backup da importação — toda escrita Rust posterior a ele não tem contrapartida no lado Go.
Não há replicação contínua nem reconciliação automática.

### O que permanece não comprovado

Este ensaio **não prova** um cutover de produção, **não comprova** volumes reais nem dados
pessoais, e não valida publicação, assinatura, release, janela de indisponibilidade nem
recuperação sob falha. Ele também não comprova migração de dados reais de estudo e remoção.
Qualquer cutover real exige uma janela de manutenção planejada com quiescência do escritor,
backup verificado e ensaio em uma cópia dos volumes reais antes de promover o Rust.

## Gate de elegibilidade para a remoção do serviço Go

### Autoridade e propósito

Este é o gate terminal da Fase 5. O ADR determina a remoção do serviço Go em um pull request
próprio e preserva a possibilidade de retomar a implementação anterior enquanto a reescrita não
estiver concluída (`docs/architecture/adr/0001-reescrita-em-rust.md:54`,
`docs/architecture/adr/0001-reescrita-em-rust.md:109`). O roadmap registra essa entrega como
“Fase 5 — distribuição por plataforma e remoção do serviço Go” (`docs/product/roadmap.md:26`).
Este documento consolida a evidência necessária; não substitui a revisão do pull request nem a
configuração de proteção da branch.

### Tabela de gates

| gate | evidência exigida | como é produzida | responsável (agente / operador / configuração de branch protection) | status atual |
| --- | --- | --- | --- | --- |
| G1 identidade de versão do servidor Rust | Um run de `rust-server-rc.yml` demonstra que o binário reporta a versão injetada. | Injetar a versão no run, executar o smoke test e conservar o artefato e sua saída de `--version`. O binário prefere `LEARNVAULT_BUILD_VERSION` e cai para `CARGO_PKG_VERSION` (`crates/server/src/lib.rs:42`); o dispatch `gh workflow run rust-server-rc.yml --ref main -f ref=main -f version=<v>` falha fechado se a versão vier vazia. | agente prepara o workflow e a evidência; operador confere o artefato | **VERDE** — run 38993897882 sobre `d499881` com `0.4.0-rc.1`: smoke aprovado (`--version` = `learnvault-server 0.4.0-rc.1`, healthz com `"version":"0.4.0-rc.1"` e `no-store`) e artefato verificado (`learnvault-server-linux-x86_64.tar.gz`, SHA-256 `6afef8e010c9c2feae5fff3096699a1ff9c8b89deecdc102a435961dfd7802aa`) |
| G2 artefato Rust publicado e em soak | Artefato Rust publicado no canal suportado e relatório de soak concluído. | Criar o caminho de publicação e executar o soak contra o artefato. O caminho existe em `.github/workflows/rust-server-rc-publish.yml` como draft prerelease `rust-rc-*`, não assinado; o soak ainda não foi executado nem aceito. | agente implementa o caminho; operador executa e aceita o soak | **VERMELHO** — o soak ainda não foi executado nem aceito |
| G3 container/Compose Rust suportado ou explicitamente opt-in | Decisão publicada sobre suporte: caminho Rust validado ou opt-in explícito, com instruções e limites. | Validar o Containerfile/Compose Rust em uma instalação suportada e registrar o resultado, ou documentar o opt-in sem alegar suporte geral. | agente documenta e valida; operador executa o cenário | **NÃO COMPROVADO** |
| G4 required checks sem Go | Proteção de branch exige os checks Rust, desktop e container Rust pertinentes e não exige `Go tests and vet`; `Container build (Rust)` não depende da imagem Go. | O CI agora prepara `Container build (Rust)`, que constrói/valida `deploy/container/Containerfile.rust` e aplica o scan Trivy HIGH/CRITICAL, sem depender de Go; ainda é necessário remover os caminhos Go e atualizar a proteção da branch. | agente prepara o workflow; mantenedor altera a configuração de branch protection | **VERMELHO** — a proteção de branch ainda exige Go e os novos checks ainda não foram promovidos a required |
| G5 ensaio aceito em volumes reais pareados | Relatório de ensaio com uma cópia dos volumes reais, quiescência, checksum, backup e saída Rust distintos, e aceite operacional. | Executar o procedimento da seção [Gate de cutover do Compose self-hosted](#gate-de-cutover-do-compose-self-hosted) em volumes reais pareados. `scripts/verify-go-to-rust-cutover.sh` é fixture-only e não vale como evidência de volume real. | operador executa; agente conserva o relatório | **VERMELHO** — não comprovado |
| G6 aceite do MVP desktop | Aceite explícito do MVP desktop conforme os requisitos da Fase 4 e da distribuição aprovada. | Executar os fluxos desktop e os checks de distribuição aplicáveis, registrar falhas e aceite. | operador aceita após evidência; agente fornece os artefatos | **NÃO COMPROVADO** |
| G7 rollback por volumes pareados com RPO explícito | Ensaio de rollback que preserve o volume Go, descarte o volume Rust e declare o RPO. | Seguir o procedimento desta página e `.agents/policies/data-safety.md`, incluindo backup verificado antes da promoção e retorno somente ao volume Go. | operador executa e aceita; agente documenta | **NÃO COMPROVADO** — o procedimento está documentado, mas não foi executado como evidência |
| G8 contrato de API/DB que sobrevive à remoção | Verificação do contrato em `docs/reference/api.md`, da detecção/recusa de banco Go e de `PRAGMA user_version = 4` no caminho suportado. | Executar testes e smoke tests contra o contrato Rust, incluindo banco Go recusado e schema esperado, antes de remover a referência. | agente produz testes; operador executa e aceita | **VERDE** — aceite executado em 2026-10-16: SPA construída; `cargo test -p learnvault-core` (206 testes) e `cargo test -p learnvault-server` (100 testes) aprovados; fmt e clippy sem falhas |

### Checklist de remoção do Go no CI (G4)

Na remoção coordenada do Go, o agente deve excluir o job `go` e o staging da SPA que ele exige;
excluir o job Go `Container build` quando `Container build (Rust)` já o cobrir; excluir
`deploy/container/Containerfile`; excluir o workflow de release Go; e aposentar `services/go`.

Depois disso, **somente o mantenedor** deve, na proteção da branch, adicionar `Rust tests and
lints`, `Desktop crate compile and guards` e `Container build (Rust)` ao conjunto de checks
required e remover `Go tests and vet`. Essa alteração de branch protection é maintainer-only e
nenhum agente pode executá-la.

### Registro de aceite do contrato (G8)

Em 2026-10-16, no worktree de aceite, foram executados `pnpm install --frozen-lockfile` (sucesso), `pnpm run build` (sucesso, incluindo `crates/server/web/build/200.html`), `C:\Users\ander\.cargo\bin\cargo.exe test -p learnvault-core` (206 testes, 0 falhas), `C:\Users\ander\.cargo\bin\cargo.exe test -p learnvault-server` (100 testes, 0 falhas), `C:\Users\ander\.cargo\bin\cargo.exe fmt --all -- --check` (sucesso) e `C:\Users\ander\.cargo\bin\cargo.exe clippy -p learnvault-core -p learnvault-server --all-targets -- -D warnings` (sucesso).

O contrato verificado diretamente no código é:

- API Rust (`crates/server/src/lib.rs:52-68`): `/api/healthz`, `/api/login`, `/api/me`, `/api/logout`, `/api/account/credentials`, `/api/catalog`, `/api/admin/catalog/rescan`, `/api/media`, `/api/study/recent`, `/api/study/progress`, `/api/study/position`, `/api/study/completed` e `/api/study/capabilities`. A cobertura é exercitada por `healthz_is_public_and_returns_status_and_version` (`crates/server/src/lib.rs:298`), `login_me_and_logout_use_an_expiring_secure_cookie` (`crates/server/tests/auth_http.rs:60`), `catalog_requires_authentication` e `admin_rescan_populates_the_authenticated_catalog` (`crates/server/tests/catalog_http.rs:72,234`), `media_requires_authentication_and_supports_get_head_and_ranges` (`crates/server/src/lib.rs:416`) e `record_position_round_trips_and_appears_in_recent` (`crates/server/tests/study_http.rs:291`), além dos testes adjacentes de credenciais, mídia e estudo. Não há um único teste que enumere todas as rotas.
- Schema: `SCHEMA_VERSION = 4` (`crates/core/src/migrations.rs:14`), aplicado pela cadeia usada em `crates/core/src/db.rs:54-60,78-94`; `new_empty_database_is_not_mistaken_for_legacy_go` assertiva `schema_version() == 4` (`crates/core/tests/db.rs:168-175`) e `an_existing_v4_database_already_carries_study_state_and_its_constraints` (`crates/server/tests/study_http.rs:769`) cobrem o caminho v4. `database_with_a_newer_schema_is_rejected_without_panicking` também confirma a fronteira (`crates/core/tests/db.rs:121-138`).
- Banco Go legado: `Error::LegacyGoDatabase` é recusado antes do bootstrap (`crates/core/src/db.rs:68-73,99-110`); `legacy_go_database_is_rejected_before_bootstrap_or_migration` (`crates/core/tests/db.rs:141-165`) confirma a recusa e a ausência de tabelas Rust.
- Importer: `migrate-go` exige `--input`, `--output`, `--backup` e `--library-id` (`crates/server/src/config.rs:204-251`), coberto por `parses_migrate_go_without_server_environment`, `migrate_go_requires_all_explicit_options` e `migrate_go_rejects_duplicate_and_unknown_options` (`crates/server/src/config.rs:409-490`). O fluxo backup-first e a importação são cobertos por `imports_users_media_sessions_progress_and_removals_after_backup` e `backup_includes_uncheckpointed_wal_data` (`crates/core/tests/legacy_import.rs:60-166`); recusas antes do backup por `rejects_schema_other_than_go_v8_before_backup`.
- Credenciais/Argon2id: `hash_password` usa Argon2id PHC compatível (`crates/server/src/auth.rs:38-65`), coberto por `password_hash_uses_the_legacy_compatible_argon2id_parameters` e `password_verification_is_boolean_and_does_not_leak_hash_details` (`crates/server/src/auth.rs:477-503`); o contrato HTTP é exercitado por `login_me_and_logout_use_an_expiring_secure_cookie`, `invalid_credentials_have_a_uniform_unauthorized_response` e `credentials_change_is_transactional_and_invalidates_old_sessions` (`crates/server/tests/auth_http.rs:60-411`).

Permanece **NÃO provado**: nenhum banco Go real foi usado (somente fixtures sintéticas temporárias), não houve aceite operacional por operador, nem execução em produção, nem prova de remoção do runtime Go. O resultado não altera o G2, que continua **VERMELHO**.


### Runbook de soak do artefato Rust (G2)

O operador deve despachar `.github/workflows/rust-server-rc.yml`, conferir o artefato e então
usar `.github/workflows/rust-server-rc-publish.yml` com a `version` do artefato, o `run_id` do
primeiro workflow e uma tag `rust-rc-*`. O resultado é uma draft prerelease não assinada e
pré-soak; ela não é o release Linux suportado (os tarballs Go continuam sendo esse release).

Extraia `learnvault-server-linux-x86_64.tar.gz` em um diretório de trabalho, preserve o SHA-256
publicado e inicie o servidor a partir do diretório extraído:

```sh
./learnvault-server --listen-addr 0.0.0.0:<port> \
  --database <dir>/learnvault.sqlite \
  --library-root <library> \
  --web-root ./web
```

No modelo atual, `--library-root` é uma **semente de implantação**: quando o registro de cursos
ainda não existe, a pasta informada é registrada como o primeiro curso com a identidade histórica
`library-1`; cursos adicionais entram pela interface, um por pasta. Uma remoção explícita do curso
semente não o traz de volta no próximo start.

Quando aplicável, migre primeiro um banco Go v8 existente, com o escritor Go parado e os caminhos
distintos:

```sh
learnvault-server migrate-go \
  --input <go.sqlite> --backup <go.sqlite.backup> \
  --output <dir>/learnvault.sqlite --library-id <id>
```

O relatório do soak deve registrar: versão reportada; SHA-256 do artefato; host; data de início;
duração; health `200` com `no-store`; login; catálogo; reprodução de mídia com `Range`; persistência
após reinício; e todas as falhas. O G2 só fecha quando o operador registrar e aceitar esse soak;
um agente não pode executar essa aceitação operacional.

#### Kit operacional reproduzível

Execute os passos na ordem abaixo, substituindo somente os valores de versão, run e caminhos
locais. Não registre credenciais, cookies, bancos ou mídia no relatório.

```sh
# 1. Despache a construção do candidato.
gh workflow run rust-server-rc.yml --ref main -f ref=main -f version=<v>

# 2. Identifique o run recém-criado e aguarde sua conclusão.
gh run list --workflow rust-server-rc.yml -L 1
gh run watch <run-id> --exit-status
run_id=<run-id>

# 3. Baixe do run enquanto o artefato ainda estiver retido (7 dias).
mkdir -p dist
gh run download "$run_id" -n rust-server-rc -D dist

# 4. Confira o SHA-256 declarado no pacote.
(cd dist && sha256sum -c SHA256SUMS.rust-rc)

# 5. Se a draft ainda não existir, publique-a; caso contrário, pule este comando.
gh release view rust-rc-<v> >/dev/null 2>&1 || gh workflow run "Rust server RC publish (draft)" --ref main -f version=<v> -f run_id="$run_id" -f tag=rust-rc-<v>

# 6. Para um soak durável, prefira baixar a draft (o artefato do run expira após 7 dias).
gh release download rust-rc-<v> -D dist
(cd dist && sha256sum -c SHA256SUMS.rust-rc)

# 7. Extraia o servidor e entre no diretório da aplicação.
mkdir -p app
tar -xzf dist/learnvault-server-linux-x86_64.tar.gz -C app
cd app
file learnvault-server
./learnvault-server --version

# 8. Inicie o servidor com uma base e uma biblioteca separadas.
mkdir -p <data-dir>
./learnvault-server --listen-addr 127.0.0.1:8090 \
  --database <data-dir>/learnvault.sqlite \
  --library-root <library> \
  --web-root ./web \
  --insecure-http &
server_pid=$!

# 9. Health: observe status 200, cache-control no-store e a versão do artefato.
curl -fsS -D - -o /dev/null http://127.0.0.1:8090/api/healthz
curl -fsS http://127.0.0.1:8090/api/healthz

# 10. Login: use um arquivo local cujo conteúdo seja {"username":"...","password":"..."};
#     não coloque credenciais no shell nem no relatório.
curl -fsS -c cookies.txt -b cookies.txt \
  -H 'Origin: http://127.0.0.1:8090' \
  -H 'Content-Type: application/json' \
  --data-binary @login.json \
  http://127.0.0.1:8090/api/login

# 11. Catálogo autenticado. Com um banco Rust novo (sem migração) o catálogo começa vazio,
#     então atualize-o a partir da biblioteca montada antes de ler.
curl -fsS -X POST -b cookies.txt -H 'Origin: http://127.0.0.1:8090' \
  http://127.0.0.1:8090/api/admin/catalog/rescan >/dev/null
curl -fsS -b cookies.txt http://127.0.0.1:8090/api/catalog

# 12. Mídia: use um caminho retornado pelo catálogo e observe HTTP 206.
curl -r 0-1023 -b cookies.txt -o /dev/null -w '%{http_code}\n' \
  'http://127.0.0.1:8090/api/media?path=<path-from-catalog>'

# 13. Reinicie e repita health e catálogo para verificar persistência.
kill "$server_pid"
wait "$server_pid" || true
./learnvault-server --listen-addr 127.0.0.1:8090 \
  --database <data-dir>/learnvault.sqlite \
  --library-root <library> \
  --web-root ./web \
  --insecure-http &
server_pid=$!
curl -fsS -D - http://127.0.0.1:8090/api/healthz
curl -fsS -b cookies.txt http://127.0.0.1:8090/api/catalog
```

Resultado esperado por verificação:

- `file` identifica um executável Linux; `--version` informa a versão selecionada.
- O health retorna `200`, `cache-control: no-store` e essa mesma versão no JSON.
- O login retorna sucesso e deixa uma sessão em `cookies.txt` para as chamadas seguintes.
- O catálogo retorna sucesso e os dados esperados para a biblioteca usada (após um rescan
  quando o banco Rust ainda estiver vazio).
- A leitura com `Range` retorna `206`.
- Após o reinício, health e catálogo continuam disponíveis e os dados esperados permanecem.
- Não há 4xx/5xx inesperado; registre qualquer status fora do esperado, saída anômala,
  consumo aproximado de CPU/RSS e reinício no relatório.

Se a migração for aplicável, execute antes do boot o comando existente acima, com input,
backup e output distintos, e registre seus caminhos no relatório sem copiar os arquivos.
Copie [`../../.agents/templates/soak-report.md`](../../.agents/templates/soak-report.md),
preencha-o por execução e anexe o relatório preenchido. O agente não pode executar nem
aceitar este soak; este kit não fecha o G2 por si só.

### Regra de decisão

O pull request dedicado de remoção só pode ser aberto quando **todos os gates G1–G8 estiverem
verdes**, com evidência executada e preservada. Um gate documentado, planejado ou meramente
executável não é verde.

### Pareamento de rollback e RPO

O pareamento é o definido no [Gate de cutover do Compose self-hosted](#gate-de-cutover-do-compose-self-hosted):
o volume Go e o backup da importação permanecem intactos; o volume Rust é separado e deve ser
descartado antes de retornar ao runtime Go. O RPO é exatamente o instante do backup da importação;
escritas posteriores no Rust são perdidas. A operação deve seguir também `.agents/policies/data-safety.md`.

### O que permanece não comprovado

Permanece não comprovada a publicação e o soak do artefato Rust, a decisão de suporte do
container/Compose, a remoção dos required checks Go, o ensaio em volumes reais, o aceite do MVP
desktop, o rollback executado com RPO observado e a verificação de aceite do contrato API/DB.
O ensaio de fixture sintética descrito acima não reduz essas lacunas.

### O que não pode ser removido junto com o Go

Os seguintes componentes permanecem até que a importação, a recusa segura e a compatibilidade
tenham sido substituídas e aceitas:

- `crates/core/src/legacy_import.rs`;
- a detecção/recusa de banco Go em `crates/core/src/db.rs`;
- os erros em `crates/core/src/error.rs`;
- o comando `learnvault-server migrate-go` (`crates/server/src/config.rs`, `crates/server/src/main.rs`);
- a compatibilidade de credenciais (`crates/server/src/auth.rs`);
- os testes/fixtures v8 (`crates/core/tests/legacy_import.rs`, `crates/core/tests/db.rs`).

### Inventário do que a remoção vai exigir

Este é inventário de planejamento apenas; nenhum item abaixo muda neste gate.

- **CI e imagem:** remover o job Go, o staging da SPA e o `needs` do container em
  `.github/workflows/ci.yml:16-41,79-91`; substituir os assets Go e os asserts exatos de release
  (build Linux Go e job de publicação) em `.github/workflows/release.yml:63-79,132-184`;
  reescrever `deploy/container/Containerfile:9-20`; revisar `.dockerignore:15-16` e
  `.gitignore:21`.
- **Automação e governança:** revisar o ecossistema `gomod` em `.github/dependabot.yml:22-36`,
  `area:go` em `.github/labeler.yml:38-41`, as exceções Go em
  `.github/workflows/dependabot-automerge.yml:89-90`, e `release-type: "go"` em
  `.github/release-please/release-please-config.json:2-6`.
- **Políticas e orientação:** revisar `.github/CONTRIBUTING.md:41-45`,
  `.github/pull_request_template.md:25-28`, `.agents/repository.md:16-19`,
  `.agents/policies/governance.md:16-18`, `.agents/playbooks/testing.md:5-6`,
  `README.md:9,14,27,55-58,96` e `docs/**`.
- **Scripts e contrato da SPA:** revisar `scripts/verify.mjs:16-17`,
  `scripts/verify-go-to-rust-cutover.*`, `apps/web/src/transport.ts:58-92` e seu teste
  `apps/web/src/transport.test.ts:4-45`.

### Armadilhas

- `Go Essencial` é apenas texto de fixture de catálogo;
- `musl-gcc` é compilador C;
- `googleapis` na URL do schema do release-please é incidental.

Nenhum deles é dependência do runtime Go.
