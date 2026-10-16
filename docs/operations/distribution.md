# Distribuição e alvos de release

A implementação atual é um serviço Go com SPA SvelteKit embutida, mantida como referência durante
a reescrita em Rust aprovada no [ADR 0001](../architecture/adr/0001-reescrita-em-rust.md). Docker
é um adaptador de deploy para servidores, não um requisito de runtime nem o modelo principal de
desenvolvimento.

## Matriz de releases

| Alvo | Artefato | Uso | Status |
| --- | --- | --- | --- |
| Linux | binário nativo | Servidor, terminal e instalações leves | Disponível |
| Self-hosted | imagem OCI Rust (Docker/Podman) | Deploy opcional em servidor | Em validação OCI/Compose |
| Windows | instalador `.exe` via NSIS | Execução local e instalação gerenciada | Planejado |
| Desktop Linux | Flatpak | Formato Linux preferencial do desktop | Planejado, sujeito a validação técnica |

Não há `.deb` nem `.rpm` como artefatos públicos nesta fase; o Flatpak é o formato Linux
preferencial do desktop.

## Limites de build

- O comando Go em `services/go/cmd/learnvault` é a fonte da verdade para builds executáveis da
  implementação atual. Na reescrita, cada binário Rust passa a ser a fonte da verdade do seu alvo.
- A SPA é construída uma vez e embutida no executável.
- Receitas de pacotes ficarão em `deploy/packages/` quando forem implementadas.
- Definições de containers ficam em `deploy/container/`, `deploy/compose/` e `deploy/quadlet/`.
- Containers não podem definir comportamento que o binário não possui.
- O CI deve construir cada artefato de plataforma antes de montar instaladores.
- Release Please controla changelog e metadados do GitHub Release.
- O workflow de publicação adiciona binários versionados e `SHA256SUMS` ao release existente.

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
oculto. O Flatpak permanece sujeito à validação técnica de permissões, acesso persistente às
bibliotecas e reprodução de mídia.

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

## Validação da imagem Rust (OCI/Compose)

Enquanto a reescrita em Rust avança, `deploy/container/Containerfile.rust`, Compose e
`scripts/verify-rust-oci.sh` permitem validar localmente que o servidor Rust (`crates/server`)
compila com a SPA e roda em um contêiner. A imagem Rust ainda está em validação e não é
publicada; o serviço Go, seus serviços auxiliares e suas definições de CI/release permanecem
no repositório para a transição.

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
- A validação OCI/Compose não faz parte do CI e não substitui `cargo fmt`, `cargo clippy` e
  `cargo test`.

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
