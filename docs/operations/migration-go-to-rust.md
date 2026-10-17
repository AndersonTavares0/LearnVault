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
