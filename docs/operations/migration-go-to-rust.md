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
| G1 identidade de versão do servidor Rust | Um run de `rust-server-rc.yml` demonstra que o binário reporta a versão injetada. | Injetar a versão no run, executar o smoke test e conservar o artefato e sua saída de `--version`. Hoje o binário usa `env!("CARGO_PKG_VERSION")` (`crates/server/src/lib.rs:41`) e o workspace fixa `0.0.0` (`Cargo.toml:18-19`). | agente prepara o workflow e a evidência; operador confere o artefato | **VERMELHO** — a versão ainda é `0.0.0`; não há run de prova aceito |
| G2 artefato Rust publicado e em soak | Artefato Rust publicado no canal suportado e relatório de soak concluído. | Criar o caminho de publicação e executar o soak contra o artefato. Hoje `.github/workflows/release.yml` constrói apenas Go para Linux e o instalador NSIS para Windows. | agente implementa o caminho; operador executa e aceita o soak | **VERMELHO** — não existe caminho de publicação Rust |
| G3 container/Compose Rust suportado ou explicitamente opt-in | Decisão publicada sobre suporte: caminho Rust validado ou opt-in explícito, com instruções e limites. | Validar o Containerfile/Compose Rust em uma instalação suportada e registrar o resultado, ou documentar o opt-in sem alegar suporte geral. | agente documenta e valida; operador executa o cenário | **NÃO COMPROVADO** |
| G4 required checks sem Go | Proteção de branch exige os checks Rust e desktop pertinentes e não exige `Go tests and vet`; `Container build` não depende da imagem Go. | Atualizar workflows e a proteção da branch. Hoje `Go tests and vet` é required; `Container build` é required e depende de Go (`.github/workflows/ci.yml:17`, `:83`, `:91`); `Rust tests and lints` e `Desktop crate compile and guards` rodam, mas não são required (`.github/workflows/ci.yml:108`, `:152`). | agente altera workflow; usuário altera a configuração de branch protection | **VERMELHO** — os required checks atuais ainda incluem Go; o usuário deve alterar a proteção |
| G5 ensaio aceito em volumes reais pareados | Relatório de ensaio com uma cópia dos volumes reais, quiescência, checksum, backup e saída Rust distintos, e aceite operacional. | Executar o procedimento da seção [Gate de cutover do Compose self-hosted](#gate-de-cutover-do-compose-self-hosted) em volumes reais pareados. `scripts/verify-go-to-rust-cutover.sh` é fixture-only e não vale como evidência de volume real. | operador executa; agente conserva o relatório | **VERMELHO** — não comprovado |
| G6 aceite do MVP desktop | Aceite explícito do MVP desktop conforme os requisitos da Fase 4 e da distribuição aprovada. | Executar os fluxos desktop e os checks de distribuição aplicáveis, registrar falhas e aceite. | operador aceita após evidência; agente fornece os artefatos | **NÃO COMPROVADO** |
| G7 rollback por volumes pareados com RPO explícito | Ensaio de rollback que preserve o volume Go, descarte o volume Rust e declare o RPO. | Seguir o procedimento desta página e `.agents/policies/data-safety.md`, incluindo backup verificado antes da promoção e retorno somente ao volume Go. | operador executa e aceita; agente documenta | **NÃO COMPROVADO** — o procedimento está documentado, mas não foi executado como evidência |
| G8 contrato de API/DB que sobrevive à remoção | Verificação do contrato em `docs/reference/api.md`, da detecção/recusa de banco Go e de `PRAGMA user_version = 4` no caminho suportado. | Executar testes e smoke tests contra o contrato Rust, incluindo banco Go recusado e schema esperado, antes de remover a referência. | agente produz testes; operador executa e aceita | **NÃO COMPROVADO** — a documentação e os guardas existem, mas não há execução de aceite registrada |

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
