# Distribuição e alvos de release

A implementação atual é um serviço Go com SPA SvelteKit embutida, mantida como referência durante
a reescrita em Rust aprovada no [ADR 0001](../architecture/adr/0001-reescrita-em-rust.md). Docker
é um adaptador de deploy para servidores, não um requisito de runtime nem o modelo principal de
desenvolvimento.

## Matriz de releases

| Alvo | Artefato | Uso | Status |
| --- | --- | --- | --- |
| Linux | binário nativo | Servidor, terminal e instalações leves | Disponível |
| Self-hosted | imagem OCI Go (Docker/Podman) | Deploy opcional em servidor | Disponível |
| Windows | instalador `.exe` via NSIS | Execução local; instalação por usuário, sem administrador | Automatizado, não assinado |
| Desktop Linux | Flatpak | Formato Linux preferencial do desktop | Backend `xdg-portal` configurado; build e smoke test no sandbox pendentes |

Não há `.deb` nem `.rpm` como artefatos públicos nesta fase. O instalador Windows NSIS continua
habilitado no release automático; o Flatpak não é construído nem publicado automaticamente.

### Instalador Windows (NSIS)

O instalador do desktop é um `.exe` NSIS construído no workflow de release e publicado com os
demais artefatos, conforme a matriz do [ADR 0001](../architecture/adr/0001-reescrita-em-rust.md).
Ele instala somente para o usuário atual (`currentUser`), em
`%LOCALAPPDATA%\LearnVault`, sem exigir senha de administrador. Os textos do instalador estão em
português (Brasil) e inglês, com seletor de idioma, e o WebView2 ausente é baixado pelo próprio
instalador (`downloadBootstrapper`) — a internet só é necessária nesse caso.

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
caixa de seleção desmarcada por padrão — "Remover dados do programa" no idioma português e "Delete
the application data" no inglês: sem marcá-la, os dados permanecem; marcada, os diretórios
`%APPDATA%\app.learnvault.desktop` e `%LOCALAPPDATA%\app.learnvault.desktop` são removidos.
Nenhum instalador copia a biblioteca física para os dados da aplicação ou para backups.

## Limites de build

- O comando Go em `services/go/cmd/learnvault` é a fonte da verdade para builds executáveis da
  implementação atual. Na reescrita, cada binário Rust passa a ser a fonte da verdade do seu alvo.
- A SPA é construída uma vez no artefato canônico `crates/server/web/build`. O serviço Go copia
  essa saída para o caminho que usa em seus testes e empacotamento; isso é um contrato preparatório
  para a futura cutover coordenada, não a remoção do serviço Go.
- Definições de containers ficam em `deploy/container/`, `deploy/compose/` e `deploy/quadlet/`.
- Containers não podem definir comportamento que o binário não possui.
- O CI deve construir cada artefato de plataforma antes de montar instaladores.
- Release Please controla changelog e metadados do GitHub Release.
- O workflow de publicação adiciona binários versionados e `SHA256SUMS` ao release existente.
- O workflow `.github/workflows/release.yml` é acionado quando o GitHub Release é publicado. Ele
  exige uma tag estrita `vX.Y.Z`, constrói os assets Go Linux existentes e o instalador NSIS nativo
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

- Exige uma imagem `learnvault` construída localmente e o Docker disponível.
- Use sempre uma biblioteca de teste. Com o ambiente de exemplo, `LIBRARY_DIR` aponta para
  `testdata/library`. A sonda **tenta escrever** em `/library`; nunca aponte o script para uma
  biblioteca real.
- Não faz parte do CI; é uma verificação manual.

## Validação experimental da imagem Rust (OCI)

Enquanto a reescrita em Rust avança, `deploy/container/Containerfile.rust` e
`scripts/verify-rust-oci.sh` permitem validar localmente que o servidor Rust (`crates/server`)
compila com a SPA canônica e roda em um contêiner. A receita Rust é explicitamente opt-in,
experimental e não é publicada nem usada por Compose, Quadlet, CI ou release. O
`deploy/container/Containerfile` continua sendo a imagem Go de produção estável/referência;
o serviço Go permanece no repositório até a cutover coordenada final.

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

## Modelo de configuração

Instalações nativas usam variáveis de ambiente ou arquivo de configuração adjacente para
`DATA_DIR`, `CACHE_DIR`, `LIBRARY_DIR`, `LISTEN_ADDR`, `PORT` e o bootstrap de autenticação.
A biblioteca física permanece externa e somente leitura em todas as plataformas. Instaladores
nunca devem copiar a biblioteca para o diretório da aplicação ou para backups.

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
gh workflow run rust-server-rc.yml --ref main -f ref=main
```

- Entradas do dispatch: `ref` (branch ou SHA a construir, padrão `main`) e `target`
  (fixo em `x86_64-unknown-linux-musl`). O workflow falha de forma fechada contra o
  namespace de tags estáveis: a guarda pré-checkout rejeita tanto um `vX.Y.Z` puro
  quanto qualquer `refs/tags/*` e, como defesa em profundidade, uma etapa
  pós-checkout falha se o HEAD for exatamente uma tag `vX.Y.Z`. Esse namespace
  pertence a `.github/workflows/release.yml`.
- O artefato contém `learnvault-server-linux-x86_64.tar.gz` (binário estático
  `learnvault-server`, a SPA canônica em `web/` com `web/200.html` e `LICENSE`) e
  `SHA256SUMS.rust-rc`. A verificação de checksum e de inventário do tar ocorre no
  próprio job.
- O job faz um smoke sem Docker: imprime `--version`, sobe o binário contra
  diretórios temporários sintéticos (nunca `/mnt/cursos` ou biblioteca real) e
  confirma `/api/healthz` (200 com `no-store`) e a entrega do shell da SPA.
- É **deliberadamente não publicado**: não há upload para release, `gh release` nem
  `--clobber` nesta etapa. O artefato sobe apenas no run do Actions com o nome
  `rust-server-rc` (sem o prefixo `release-`), portanto o workflow de release não o
  captura.
- **Não é o artefato Linux padrão** e não é usado por Compose, Quadlet, CI ou release.
  Os tarballs Go continuam sendo o único release Linux suportado.

Permanece **não comprovado** por esta etapa:

- Identidade de versão: o binário se auto-reporta como `0.0.0` (versão do workspace);
  não é um artefato versionado.
- Assinatura: o artefato não é assinado.
- Soak/rollout: nenhum teste de duração ou implantação em produção é executado.
- arm64 não é produzido; também não há validação de runtime fora do smoke do próprio job.
