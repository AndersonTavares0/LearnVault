# Distribuição e operações

**Para quem é:** quem vai publicar, instalar ou operar o LearnVault. Se você vai desenvolver, leia
[Getting-Started](../wiki/getting-started.md); se quer entender o produto, leia
[Home](../wiki/home.md).

**Resumo:** o LearnVault publica um instalador Windows para o desktop, um tarball Linux para o
servidor e uma imagem OCI opcional. O Flatpak ainda não está pronto. O servidor Rust substituiu o
serviço Go na Fase 5, e Docker é um adaptador de deploy, não um requisito de runtime.

## O que está disponível hoje

| Alvo | Artefato | Uso | Status |
| --- | --- | --- | --- |
| Linux | tarball Linux Rust (`learnvault-linux-<arch>.tar.gz`) | Único release Linux suportado | Disponível |
| Self-hosted | imagem OCI Rust via Compose (Docker/Podman) | Deploy opcional em servidor | Opt-in explícito; imagem local, não publicada |
| Windows | instalador `.exe` via NSIS | Execução local; instalação por usuário, sem administrador | Automatizado, não assinado |
| Desktop Linux | Flatpak | Formato Linux preferencial do desktop | Backend `xdg-portal` configurado; build e smoke test no sandbox pendentes |

Não há `.deb` nem `.rpm` como artefatos públicos nesta fase. O instalador Windows NSIS continua
habilitado no release automático; o Flatpak não é construído nem publicado automaticamente.

## Como escolher o caminho

- **Windows, uso local:** use o instalador NSIS. Ele instala só para o usuário atual e não pede
  senha de administrador.
- **Linux, servidor:** use o tarball `learnvault-linux-<arch>.tar.gz`.
- **Self-hosted com container:** use o Compose como opt-in explícito. A imagem é construída
  localmente e não é publicada.
- **Linux, desktop:** o Flatpak é o formato preferencial, mas ainda depende de build e smoke test no
  sandbox.

### Aviso manual de atualização

Quando habilitado nas configurações, o servidor consulta a API pública de releases do GitHub por
HTTPS e compara a tag estável com a versão local usando SemVer estrito. A consulta tem timeout de
5 segundos e aceita no máximo 64 KiB de resposta; há cache em processo de uma hora, ignorado pelo
botão de verificação manual. O resultado exige sessão autenticada e não é armazenado por caches
HTTP. O link exibido é construído para o repositório oficial, sem confiar no `html_url` retornado
pela API. Nenhum arquivo é baixado ou instalado automaticamente. Builds locais com versão
`0.0.0` não consultam a API, pois não possuem versão de release para comparar.

### Uso do tarball Linux

O tarball `learnvault-linux-<arch>.tar.gz` traz o binário `learnvault-linux-<arch>`, o `LICENSE` e a
SPA canônica em `web/`, com `web/200.html`. Extraia e aponte o servidor para essa pasta:

```sh
tar -xzf learnvault-linux-amd64.tar.gz
./learnvault-linux-amd64 --web-root web
```

Sem `--web-root` o servidor não serve a interface (a rota da SPA responde 404); a API continua
funcionando.

### Instalador Windows (NSIS)

O instalador do desktop é um `.exe` NSIS construído no workflow de release e publicado com os
demais artefatos, conforme a matriz do [ADR 0001](../architecture/adr/0001-reescrita-em-rust.md).
Ele instala somente para o usuário atual (`currentUser`), em
`%LOCALAPPDATA%\LearnVault`, sem exigir senha de administrador. Os textos do instalador estão em
português (Brasil) e inglês, com seletor de idioma, e o WebView2 ausente é baixado pelo próprio
instalador (`downloadBootstrapper`). A internet só é necessária nesse caso.

Para gerar o instalador localmente no Windows, usando o mesmo comando do workflow de release (sem
o override de versão):

```sh
pnpm install --frozen-lockfile
pnpm run build:desktop
pnpm exec tauri build --bundles nsis --config crates/desktop/tauri.conf.json
```

O artefato fica em `target/release/bundle/nsis/` (`LearnVault_<versão>_x64-setup.exe`) e é
publicado no release como `learnvault-windows-<tag>.exe`. Sem o override de release, a versão vem
do workspace (`0.0.0`); no release, a tag `vX.Y.Z` define a versão publicada. O instalador não é
assinado: o Windows pode exibir o aviso do SmartScreen ("O Windows protegeu o PC"). Em "Mais
informações", a opção "Executar assim mesmo" prossegue com a instalação; confira o `SHA256SUMS` do
release antes de distribuir um download.

#### Dados do usuário no Windows

O desktop guarda os dados no perfil do usuário, fora da biblioteca, que permanece externa e
somente leitura e nunca é copiada.

| Caminho | Conteúdo |
| --- | --- |
| `%APPDATA%\app.learnvault.desktop\library.json` | raiz da biblioteca escolhida (configuração versionada, gravação atômica) |
| `%APPDATA%\app.learnvault.desktop\learnvault.sqlite` | catálogo indexado e progresso de estudo (SQLite local) |
| `%LOCALAPPDATA%\app.learnvault.desktop\` | dados internos do WebView2, mantidos pelo próprio Windows |

Atualizar o aplicativo por cima preserva esses dados. Na desinstalação, o desinstalador exibe uma
caixa de seleção desmarcada por padrão: "Remover dados do programa" no idioma português e "Delete
the application data" no inglês: sem marcá-la, os dados permanecem; marcada, os diretórios
`%APPDATA%\app.learnvault.desktop` e `%LOCALAPPDATA%\app.learnvault.desktop` são removidos.
Nenhum instalador copia a biblioteca física para os dados da aplicação ou para backups.

## Limites de build

- Cada binário Rust (`crates/server`, `crates/desktop`) é a fonte da verdade do seu alvo.
- A SPA é construída uma vez no artefato canônico `crates/server/web/build` e `crates/desktop/frontend-dist`.
- Definições de containers ficam em `deploy/container/`, `deploy/compose/` e `deploy/quadlet/`.
- Containers não podem definir comportamento que o binário não possui.
- O CI deve construir cada artefato de plataforma antes de montar instaladores.
- Release Please controla changelog e metadados do GitHub Release.
- O workflow de publicação adiciona binários versionados e `SHA256SUMS` ao release existente.
- O workflow `.github/workflows/release.yml` é acionado quando o GitHub Release é publicado. Ele
  exige uma tag estrita `vX.Y.Z`, constrói os tarballs Linux do servidor Rust e o instalador NSIS nativo
  do Windows, e só então publica esses artefatos no release. O Flatpak está deliberadamente fora
  desse fluxo até que o bloqueio de permissões seja resolvido.
- Os artefatos desktop são deliberadamente **não assinados**: nenhum certificado ou segredo de
  assinatura é usado nesta etapa. Verifique `SHA256SUMS` antes de distribuir um download.

### Bloqueio do Flatpak

O build e a publicação do Flatpak continuam bloqueados. O desktop usa o backend oficial
`xdg-portal` no Linux e o manifesto não concede acesso amplo à home ou ao host; ainda falta
validar, com um build Flatpak real, o acesso persistente somente ao diretório escolhido e a
reprodução de mídia no sandbox. O manifesto não representa um artefato de release atual. Não há
acesso de rede nem permissão de escrita na biblioteca; ela permanece externa ao diretório da
aplicação e nunca é copiada para dados ou backups.

## Runner self-hosted de validação

O runner persistente `rust-validation-114` da VM 114 do Proxmox não faz parte da CI cotidiana.
A validação Rust oficial permanece no job `Rust tests and lints` do workflow `CI`, executado em
`ubuntu-latest`. O runner Proxmox pode permanecer desligado para preservar recursos e evitar que
um job fique aguardando um executor offline.

Para reativá-lo futuramente, com autorização operacional:

1. Ligue a VM 114 no Proxmox, sem recriar ou reprovisionar seus discos e configurações.
2. Inicie o serviço do Actions Runner com o usuário não-root originalmente configurado.
3. Gere uma credencial de registro temporária no GitHub e registre novamente o runner com o label
   `rust-validation`, seguindo a documentação oficial do repositório.
4. Verifique no GitHub que o runner está `online` antes de adicionar qualquer workflow que use
   `runs-on: [self-hosted, linux, x64, rust-validation]`.
5. Para desativá-lo novamente, remova primeiro o registro no GitHub com a credencial temporária,
   depois pare o serviço e desligue a VM. Não apague arquivos, discos ou configurações da VM.

## Ferramentas de empacotamento

O empacotamento segue o [ADR 0001](../architecture/adr/0001-reescrita-em-rust.md): NSIS no
Windows, Flatpak como formato Linux preferencial e imagem OCI mais binário nativo para instalação
self-hosted. Os builds devem ser versionados e reproduzíveis, sem tornar Docker um requisito
oculto. O Flatpak permanece bloqueado até a validação do build/sandbox real, do acesso persistente
às bibliotecas e da reprodução de mídia.

## Verificação manual de somente leitura

`scripts/verify-readonly.sh` confirma que o contêiner não consegue gravar na biblioteca
montada. O script sobe o serviço definido em `deploy/compose/compose.yaml` e tenta criar um
arquivo dentro de `/library`, esperando que a escrita falhe.

```sh
COMPOSE="docker compose --env-file deploy/compose/.env.example -f deploy/compose/compose.yaml" \
  ./scripts/verify-readonly.sh
```

- Exige a imagem do serviço `learnvault` construída localmente pelo Compose
  (`learnvault-rust:local`, a partir de `deploy/container/Containerfile`) e o Docker disponível.
- Use sempre uma biblioteca de teste. Com o ambiente de exemplo, `LIBRARY_DIR` aponta para
  `testdata/library`. A sonda **tenta escrever** em `/library`; nunca aponte o script para uma
  biblioteca real. No modelo atual, a pasta configurada é registrada como o primeiro curso quando
  o registro ainda não existe; cursos adicionais entram pela interface, um por pasta.
- Não faz parte do CI; é uma verificação manual.

## Validação experimental da imagem Rust (OCI)

O `deploy/container/Containerfile` e o `scripts/verify-rust-oci.sh` permitem validar localmente que
o servidor Rust (`crates/server`)
compila com a SPA canônica e roda em um contêiner. O caminho self-hosted do Compose constrói
essa imagem Rust localmente; ela continua experimental, não publicada nem assinada e não é um
artefato de release. O CI e o workflow de release constroem o servidor Rust a partir do mesmo
`deploy/container/Containerfile`.

```sh
./scripts/verify-rust-oci.sh
CONTAINER_ENGINE=podman ./scripts/verify-rust-oci.sh
```

- Exige Docker ou Podman e acesso à rede para baixar as dependências Rust durante o build.
  Sem um dos dois, o script encerra com erro explicando o pré-requisito.
- Usa uma fixture sintética gerada em diretório temporário (removido ao final) e a monta
  como `/library` somente leitura; nunca aponta para uma biblioteca real.
- Verifica a versão do binário, `/api/healthz` (200 com `no-store`), o
  `HEALTHCHECK` da imagem (`healthy`), a rejeição não autenticada de `/api/catalog` e
  `/api/media` (401), a execução como usuário não-root (uid 10001) sem privilégios
  adicionais e sem capabilities, a gravação em `/data`, a montagem somente leitura e o
  bloqueio de escrita em `/library`, o bloqueio de escrita na raiz do contêiner, a
  recuperação após reinício com persistência dos dados e a remoção do contêiner ao final.
- O servidor Rust ainda não possui interface de cache (`CACHE_DIR`); não há diretório de
  cache para validar nesta fase.
- Não faz parte do CI e não substitui `cargo fmt`, `cargo clippy` e `cargo test`.
- A referência de imagem do Quadlet (`localhost/learnvault:latest`) ainda não tem uma origem
  publicada; a imagem precisa ser construída localmente.

## Container Rust no Compose (opt-in explícito)

Decisão de 2026-10-17: o caminho self-hosted do Compose é um **opt-in explícito** do
servidor Rust. Quem sobe `deploy/compose/compose.yaml` escolhe o servidor Rust de
propósito; ninguém é migrado por acidente, porque o release Linux suportado é o tarball do
servidor Rust, o mesmo artefato que o CI e o release constroem. Isto não é suporte geral: é
um caminho documentado, com os limites declarados abaixo.

O arquivo constrói a imagem a partir de `deploy/container/Containerfile`:

```sh
docker compose --env-file deploy/compose/.env.example \
  -f deploy/compose/compose.yaml up -d --build
```

Antes de apontar `DATA_DIR` para um banco existente, migre com
`learnvault-server migrate-go`: o servidor Rust **falha fechado** em banco no formato Go
v7/v8 e recusa iniciar.

Contrato aplicado pelo arquivo:

- `read_only: true` na raiz do contêiner, `cap_drop: [ALL]`,
  `security_opt: [no-new-privileges:true]` e `/tmp` em `tmpfs` `noexec,nosuid`.
- A biblioteca é montada somente leitura (`/library:ro`); a escrita fica restrita a
  `/data` e `/cache`.
- O `HEALTHCHECK` consulta `/api/healthz`; sem resposta saudável o contêiner não é
  considerado pronto.

Evidências executadas:

- Comando executado em 2026-10-17 num host de desenvolvimento:

  ```sh
  docker compose --env-file deploy/compose/.env.example -f deploy/compose/compose.yaml config
  ```

  A saída resolve as variáveis e renderiza o contrato acima, com a biblioteca somente
  leitura.
- O job `Container build (Rust)` da CI, verde na run `39002977974` sobre a `main`,
  constrói o mesmo `deploy/container/Containerfile`, roda `scripts/verify-rust-oci.sh` e aplica Trivy
  HIGH/CRITICAL.

Limites declarados:

- A imagem não é publicada nem assinada, e não há artefato `arm64`.
- Não houve soak nem instalação real em servidor; o cenário em produção segue sendo
  responsabilidade do operador.
- `CACHE_DIR` é montado, mas o servidor Rust ainda não tem interface de cache; o
  diretório não é usado nesta fase.
- Expor o serviço à rede exige proxy HTTPS com timeout de cabeçalhos, limite de conexões
  por origem e limite de tamanho de cabeçalho (ver "Proteção do listener HTTP").
- Os tarballs Linux Rust continuam o único release Linux suportado.

## Modelo de configuração

Instalações nativas usam variáveis de ambiente ou arquivo de configuração adjacente para
`DATA_DIR`, `CACHE_DIR`, `LISTEN_ADDR`, `PORT` e o bootstrap de autenticação. `LIBRARY_DIR`
continua aceito como semente do primeiro curso (a pasta vira um curso com a identidade histórica
`library-1` quando o registro ainda não existe); os demais cursos são registrados pela interface,
em `courses.json`, ao lado do banco. As pastas de curso permanecem externas e somente leitura em
todas as plataformas. Instaladores nunca devem copiar as pastas de curso para o diretório da
aplicação ou para backups.

### Credenciais iniciais por arquivo de segredo

O bootstrap aceita `ADMIN_USERNAME` e `ADMIN_PASSWORD` diretamente, por compatibilidade, ou
`ADMIN_USERNAME_FILE` e `ADMIN_PASSWORD_FILE`, apontando para arquivos de segredo. Para cada
credencial, configure somente uma das duas origens; definir ambas é erro de configuração. Os arquivos
devem ser UTF-8 e não vazios. Um único `LF` final (e o `CR` que o precede) é removido; outros espaços
e quebras são preservados. Erros de leitura são genéricos e não exibem o valor nem o caminho.

Essas credenciais são usadas apenas para criar o administrador quando o banco ainda não tem um. Não
alteram uma conta existente. Sem credenciais configuradas, o fluxo atual de primeiro acesso permanece
inalterado. O servidor não verifica o modo/permissões do arquivo: o operador deve restringir a leitura
ao usuário/grupo do serviço e evitar diretórios ou backups acessíveis a outros usuários. O arquivo é
lido na inicialização; para trocar credenciais existentes, use o fluxo autenticado de alteração de
credenciais.

#### Compose com secrets locais

O arquivo de override `deploy/compose/compose.secrets.yaml` monta secrets de arquivos locais em
`/run/secrets/` e aponta as variáveis `*_FILE` para eles. Crie `deploy/compose/secrets/admin_username`
e `admin_password` fora do controle de versão (a pasta já está ignorada pelo Git), com conteúdo
sintético apenas em testes. Em um host Linux, restrinja os arquivos ao UID não-root `10001`, usado
pelo container, por exemplo com modo `0400`. O conteúdo não deve ser colocado em argumentos de
comando, shell history ou arquivo `.env`.

```sh
docker compose --env-file deploy/compose/.env.example \
  -f deploy/compose/compose.yaml \
  -f deploy/compose/compose.secrets.yaml config
docker compose --env-file deploy/compose/.env.example \
  -f deploy/compose/compose.yaml \
  -f deploy/compose/compose.secrets.yaml up -d --build
```

O override limpa as variáveis diretas do `.env.example`, evitando conflito com `*_FILE`. Os arquivos
locais são bind mounts de segredo do Compose; proteja também a origem no host. Não adicione esses
arquivos, valores reais ou caminhos privados a commits.

#### Quadlet com credenciais systemd

O exemplo `deploy/quadlet/learnvault.container` usa `LoadCredential=` e disponibiliza os arquivos
protegidos pelo systemd ao processo via `%d/admin_username` e `%d/admin_password`. Coloque as fontes
em `%h/stacks/learnvault/secrets/`, restrinja-as ao usuário do serviço (por exemplo, modo `0600`) e
remova `ADMIN_USERNAME`/`ADMIN_PASSWORD` antigos de `learnvault.env`; mantenha ali apenas as demais
configurações. O systemd prepara um diretório de credenciais privado para cada execução do serviço.

Em instalações que preferem o armazenamento nativo de secrets do Podman, Quadlet também aceita
`Secret=` (equivalente a `podman --secret`, montado em `/run/secrets/<nome>` por padrão). Nesse caso,
use `ADMIN_USERNAME_FILE=/run/secrets/<nome>` e `ADMIN_PASSWORD_FILE=/run/secrets/<nome>` e não
configure as variáveis diretas correspondentes.

## Proteção do listener HTTP

O timeout de 30 segundos do serviço Rust é aplicado depois que a requisição HTTP foi aceita pelo
router; ele não limita clientes que mantêm uma conexão aberta enquanto enviam cabeçalhos lentamente.
Em deploys expostos à rede, coloque o serviço atrás de um proxy HTTPS que imponha timeout de leitura
de cabeçalhos, limite de conexões por origem e limites de tamanho de cabeçalho. Esses controles são
responsabilidade do proxy ou da infraestrutura de implantação: esta documentação não afirma que o
servidor, sozinho, elimina slowloris ou outros limites de rede externos.

## Release candidate Rust (opt-in)

Esta subseção é **aditiva** e não altera a matriz de releases acima. O workflow
`.github/workflows/rust-server-rc.yml` produz um artefato experimental do servidor
Rust (`crates/server`) para Linux amd64 musl, como candidato de release candidate
(opt-in). Ele é acionado **apenas manualmente**; não roda em `release`, em tags, em
`push` nem por agendamento.

```sh
gh workflow run rust-server-rc.yml --ref main -f ref=main -f version=0.4.0-rc.1
```

- Entradas do dispatch: `ref` (branch ou SHA a construir, padrão `main`), `version`
  (versão injetada no binário; obrigatória, e o workflow falha de forma fechada se ela
  vier vazia) e `target` (fixo em `x86_64-unknown-linux-musl`). O workflow falha de forma fechada contra o
  namespace de tags estáveis: a guarda pré-checkout rejeita tanto um `vX.Y.Z` puro
  quanto qualquer `refs/tags/*` e, como defesa em profundidade, uma etapa
  pós-checkout falha se o HEAD for exatamente uma tag `vX.Y.Z`. Esse namespace
  pertence a `.github/workflows/release.yml`.
- O artefato contém `learnvault-server-linux-x86_64.tar.gz` (binário estático
  `learnvault-server`, a SPA canônica em `web/` com `web/200.html` e `LICENSE`) e
  `SHA256SUMS.rust-rc`. A verificação de checksum e de inventário do tar ocorre no
  próprio job.
- O job faz um smoke sem Docker: confere que `--version` reporta exatamente a versão do
  dispatch, sobe o binário contra diretórios temporários sintéticos (nunca `/mnt/cursos`
  ou biblioteca real) e confirma `/api/healthz` (200 com `no-store`, com a mesma versão)
  e a entrega do shell da SPA.
- É **deliberadamente não publicado**: não há upload para release, `gh release` nem
  `--clobber` nesta etapa. O artefato sobe apenas no run do Actions com o nome
  `rust-server-rc` (sem o prefixo `release-`), portanto o workflow de release não o
  captura.
- **Não é o artefato Linux padrão**, não é publicado nem assinado e não é usado por Quadlet,
  CI ou release. O Compose o constrói localmente; os tarballs Rust continuam sendo o único
  release Linux suportado.

Permanece **não comprovado** por esta etapa:

- Identidade de versão: a versão vem da entrada `version` do dispatch; o workflow não
  comprova a seleção da versão de release além desse valor fornecido.
- Assinatura: o artefato não é assinado.
- Soak/rollout: nenhum teste de duração ou implantação em produção é executado.
- arm64 não é produzido; também não há validação de runtime fora do smoke do próprio job.

## Publicação experimental do RC Rust

O workflow manual **Rust server RC publish (draft)** publica uma draft prerelease com tag
`rust-rc-*`. O artefato é não assinado e não é o release Linux suportado; os tarballs Rust continuam
sendo o release suportado. Despache-o informando a versão, o run do build e a tag:

```sh
gh workflow run "Rust server RC publish (draft)" --ref main \
  -f version=<v> -f run_id=<rust-server-rc-run-id> -f tag=rust-rc-<v>
```

## Verificação de atualização

O servidor Rust expõe `GET /api/update`, uma consulta autenticada (mesma proteção das rotas de
estudo: sessão válida e sem troca de credenciais pendente) que informa se há uma release publicada
mais nova. A resposta é sempre `private, no-store` e tem o formato:

```json
{ "current": "0.3.0", "enabled": true, "available": false, "latest": null, "releasedAt": null, "url": null, "checkedAt": "1760702400", "error": null }
```

- `current` é a versão em execução; `enabled` indica se a checagem está ligada.
- `available` é `true` apenas quando a release publicada é mais nova que a versão em execução.
  Nesse caso, `latest` traz a versão (sem o `v` inicial), `releasedAt` a data de publicação e
  `url` o link da release. Fora disso, esses três campos vêm nulos.
- Rascunhos (`draft`) e pré-lançamentos (`prerelease`) são ignorados: o canal estável só anuncia
  releases publicados. A comparação é numérica por componente (`0.10.0` > `0.9.0`), com o `v`
  inicial opcional e um pré-lançamento sempre abaixo do lançamento correspondente.
- `?force=1` (ou `?force=true`) ignora o intervalo mínimo de cache. É o que o botão manual usa.

A checagem acontece **no servidor local**, nunca no navegador. O servidor faz uma única
requisição de saída, por HTTPS, para um destino fixo no código (não há URL vinda da requisição,
o que evita SSRF):

```text
GET https://api.github.com/repos/AndersonTavares0/LearnVault/releases/latest
Accept: application/vnd.github+json
User-Agent: learnvault/<versão em execução>
```

O orçamento de rede é de 5 segundos. Uma falha de rede **nunca** vira erro HTTP: a resposta
continua `200`, com `available: false`, `latest: null` e um `error` curto em português, e o
último resultado bem-sucedido permanece em cache. Estar offline não quebra nem trava a tela de
Configurações.

Checagens externas respeitam um intervalo mínimo em processo, de **3600 segundos** (uma hora) por
padrão, e requisições concorrentes são coalescidas em uma única chamada ao GitHub.

### Como desligar

- Flag de linha de comando: `--no-update-check`.
- Variável de ambiente: `LEARNVAULT_UPDATE_CHECK=off` (aceita `off`/`false` para desligar e
  `on`/`true` para ligar; qualquer outro valor é rejeitado). A flag tem precedência sobre a
  variável.

Desligada, a checagem responde com `enabled: false` e não faz **nenhuma** chamada de rede.

### Sem download nem instalação automática

O produto apenas **verifica e avisa**: a SPA mostra a versão atual e, quando há algo novo, o
número da release e um link para a página do GitHub. Nada é baixado, verificado por checksum,
instalado ou aplicado automaticamente. A atualização continua manual, pelo mesmo caminho de
distribuição descrito neste documento (tarball Linux, instalador Windows ou Flatpak).

## Próximos passos

- [[Getting-Started]] para o ambiente de desenvolvimento.
- [[Architecture]] para entender os limites do sistema.
- [[Development-and-Governance]] para o fluxo de release e checks.
