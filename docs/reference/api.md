# Referência da API

A implementação atual é o servidor Rust, que expõe as APIs abaixo, e a SPA web usa o mesmo contrato
de Study nos dois transportes. Todas as rotas usam JSON, salvo
indicação contrária. A autenticação usa um cookie de sessão HttpOnly com SameSite=Strict.

No servidor Rust, cada pasta de curso registrada deve permanecer sob controle confiável do
processo entre as operações administrativas. O registro vive em `courses.json`, ao lado do
banco, e os caminhos nunca cruzam a API: a interface conhece apenas a identidade opaca e o
título de cada curso. O rescan revalida cada raiz e recusa mudanças detectáveis, mas trocas
concorrentes de junctions, symlinks ou reparse points durante a abertura ainda dependem das
garantias específicas do sistema operacional; essa limitação não deve ser tratada como
isolamento absoluto contra um escritor local concorrente.

Na implementação Rust, o cookie também recebe `Secure` por padrão e só é enviado pelo
navegador em HTTPS. Para desenvolvimento local sem TLS, use explicitamente `--insecure-http`;
esse modo não deve ser usado em uma instalação exposta na rede. Mutações de autenticação exigem
uma origem HTTP(S) compatível, além de SameSite=Strict.

## Entrega da SPA no servidor Rust

A entrega é opcional e fica desabilitada por padrão. Use `--web-root` apontando para o diretório
gerado pelo build estático (`crates/server/web/build`), que contém o shell `200.html`. Arquivos
existentes são servidos diretamente; caminhos sem extensão usam `200.html` como fallback; assets
ausentes com extensão retornam 404, e caminhos `/api/*` nunca são convertidos em HTML. Se o shell
não existir, o servidor retorna 503. O shell usa `Cache-Control: no-cache`; assets sob
`_app/immutable/` usam cache imutável; respostas de erro da SPA usam `no-store`.
A validação do diretório não expõe caminhos em respostas. O diretório web é um artefato
de deploy confiável e somente leitura: publicar um shell novo exige reiniciar o processo.

## Saúde e sessão

| Método | Rota | Autenticação | Finalidade |
| --- | --- | --- | --- |
| `GET` | `/api/healthz` | Não | Saúde do processo e do banco, incluindo a versão em execução |
| `POST` | `/api/login` | Não | Aceita `{ "username": "...", "password": "..." }` |
| `POST` | `/api/logout` | Sessão | Invalida a sessão atual |
| `GET` | `/api/me` | Sessão | Retorna a conta atual |
| `POST` | `/api/account/credentials` | Sessão | Aceita `{ "current_password": "...", "username": "...", "password": "..." }`, troca as credenciais, invalida as sessões anteriores e emite uma nova |

O primeiro administrador temporário pode ser `admin/admin` somente no bootstrap; a resposta marca
`must_change_credentials` e todos os endpoints protegidos recusam acesso com
`403 credentials_change_required` até a troca. A senha atual é verificada antes da troca e os
novos valores obedecem aos limites públicos. A operação é transacional: uma falha não substitui
credenciais nem sessões.

## Compatibilidade e migração do banco

O servidor Rust detecta a assinatura do banco legado Go (`users`, `sessions` e
`schema_migrations`) antes de aplicar qualquer bootstrap ou migração e encerra com a mensagem
estável `legacy_go_database_requires_migration: back up the database and migrate it before using the Rust server`.
Ele não cria `admin/admin` nem sobrescreve dados. A API de núcleo `legacy_import` e o comando
`learnvault-server migrate-go` fazem a conversão explícita com backup-first, origem somente leitura
e saída temporária validada; o subcomando não inicia o listener HTTP. Um banco novo vazio e um
banco Rust existente continuam sendo abertos normalmente.

## Catálogo, cursos e mídia

| Método | Rota | Autenticação | Finalidade |
| --- | --- | --- | --- |
| `GET` | `/api/catalog` | Sessão | Retorna um nó de topo por curso registrado (`role: course`, com `course_id` opaco e `title`), com a subárvore de módulos e aulas derivada dos dados persistidos, tanto na árvore canônica `nodes` quanto na lista de cursos de topo `categories` |
| `PUT` | `/api/catalog/order` | Sessão + mesma origem | Salva `{ "course_id": "...", "parent_path": "...", "child_paths": ["..."] }` para a lista completa de módulos/aulas irmãos; a identidade do dono vem da sessão |
| `DELETE` | `/api/catalog/order` | Sessão + mesma origem | Recebe `{ "course_id": "...", "parent_path": "..." }` e remove somente a preferência daquela lista de irmãos |
| `POST` | `/api/admin/courses/preview` | Sessão admin + mesma origem | Aceita `{ "path": "..." }` e devolve uma árvore física somente leitura, com itens encontrados, ignorados, inacessíveis e não suportados, além de sugestões heurísticas explicáveis; não registra curso, mídia ou classificação |
| `POST` | `/api/admin/courses` | Sessão admin | Aceita `{ "path": "..." }` e registra a pasta como um curso; a pasta precisa existir, ser um diretório legível e não ser link/junction. Importar não indexa: a indexação é o `rescan` |
| `DELETE` | `/api/admin/courses/{id}` | Sessão admin | Remove o curso do registro, sem apagar arquivos nem dados; remover um curso desconhecido responde `removed: false` |
| `POST` | `/api/admin/catalog/rescan` | Sessão admin | Faz scan explícito de todos os cursos registrados e atualiza cada catálogo de forma preservadora: insere arquivos novos, reafirma os conhecidos e marca ausentes os que sumiram, sem excluir nenhum registro. Sem cursos registrados responde `400 no_courses_registered`; a resposta agrega `courses` e `failed_courses` |
| `GET`, `HEAD` | `/api/media?course=...&path=...` | Sessão | Transmite um arquivo aprovado do curso indicado com suporte a HTTP Range; caminho relativo é interpretado dentro da pasta daquele curso |
| `GET` | `/api/admin/media/{id}/impact` | Sessão admin | Consulta o impacto da remoção de uma mídia: retorna `{ media_id, relative_path, availability, progress_owners, already_removed }` |
| `DELETE` | `/api/admin/media/{id}` | Sessão admin | Remove explicitamente a mídia do catálogo (tombstone persistente). Aceita `{ "confirm_progress": bool }`. Se houver progresso e `confirm_progress` for falso, responde `409 media_removal_needs_confirmation`. Repetir é idempotente |


### Organização dos cursos registrados

Pastas de módulo contendo vídeos diretamente são apresentadas como módulos recolhíveis,
com uma aula por vídeo, sem alterar caminhos, arquivos ou identidades de mídia. Uma pasta
mista, com vídeos diretos e subpastas, mantém ambos: os vídeos viram aulas filhas e as
subpastas continuam módulos aninhados, sem alternar papéis por causa da profundidade.
Materiais complementares continuam disponíveis no módulo e legendas correspondentes
permanecem associadas à mídia. A árvore de diretórios já existente é preservada.

A sequência do catálogo usa ordenação natural dos caminhos originais: `2` precede `10`,
inclusive em módulos, aulas e materiais. Os prefixos são usados na ordenação mesmo quando
omitidos no título exibido. Essa organização é derivada na leitura: não renomeia arquivos
nem modifica o progresso salvo.

Sem nenhum curso registrado o catálogo responde vazio. `LIBRARY_DIR`/`--library-root` continua
aceito como semente de implantação: quando o registro ainda não existe, a pasta informada é
registrada como o primeiro curso com a identidade histórica `library-1`, preservando mídias,
varreduras e progresso de estudo já gravados; uma remoção explícita do curso semente não o
traz de volta no próximo start.

Cada usuário pode substituir a ordem automática de módulos ou aulas dentro da mesma lista de
irmãos. A preferência é persistida no banco LearnVault e isolada por usuário, curso e pai; não
altera a pasta do curso. Os novos itens não presentes na preferência são acrescentados em ordem
natural, e caminhos já salvos são mantidos durante reindexações mesmo quando temporariamente
ausentes. `DELETE /api/catalog/order` restaura a ordem natural somente para o pai indicado. A
rota de escrita aceita corpos JSON de até 1 MiB; `course_id` é opaco e os caminhos são relativos,
nunca caminhos absolutos do sistema.

O scanner lê somente entradas e metadados de diretórios. Links simbólicos não são seguidos — nem
as raízes dos cursos, nem entradas internas. Ignora diretórios temporários de download, classifica
mídias, legendas e materiais reconhecidos, e trata `.ts` como vídeo somente quando é maior que
1 MiB para não confundir TypeScript com MPEG-TS. As pastas de curso são externas, montadas como
somente leitura e nunca são modificadas.

No desktop, o protocolo `learnvault-media://localhost/<media-id>` entrega materiais, vídeo, áudio e arquivos de legenda para download usando somente o `media_id` opaco. `GET` e `HEAD` aceitam uma única faixa; respostas completas, parciais e recusadas usam, respectivamente, `200`, `206`, `413` (corpo acima do limite) ou `416` (faixa inválida ou múltipla), com `Content-Range` quando aplicável. No Windows, a SPA usa `http://learnvault-media.localhost/<media-id>` e o runtime reverte a origem antes do handler. O player desktop usa essa fonte local, renderiza legendas VTT e sincroniza posição e conclusão pelo IPC de Study; SRT, ASS e SSA ficam disponíveis para download, e o build HTTP pode converter SRT sob demanda.

### Cursos e mídia no desktop (IPC)

O desktop não tem rotas para catálogo, cursos e remoção: a SPA chama comandos Tauri por `invoke`, sem
`fetch`. O webview nunca envia nem recebe um caminho — só identidades opacas e o booleano
`configured`.

| Comando | Argumentos | Finalidade |
| --- | --- | --- |
| `desktop_catalog` | — | Catálogo agregado dos cursos registrados, um nó de topo por curso em `nodes` (árvore canônica) e em `categories` |
| `desktop_reorder_catalog` | `courseId`, `parentPath`, `childPaths` | Salva a ordem completa de irmãos no banco local LearnVault; o dono é fixo no runtime Rust e não é fornecido pelo webview |
| `desktop_reset_catalog_order` | `courseId`, `parentPath` | Restaura a ordem natural somente para a lista de irmãos indicada |
| `desktop_import_course` | — | Abre o seletor nativo e registra a pasta escolhida como um curso; importar uma pasta já registrada é idempotente |
| `desktop_remove_course` | `courseId` | Remove o curso do registro, sem tocar nos arquivos nem nos dados |
| `desktop_rescan` | — | Indexa explicitamente todos os cursos registrados; raízes removidas contam como falha agregada |
| `desktop_media_removal_impact` | `mediaId` | Consulta o impacto da remoção de uma mídia (identidade, caminho, donos com progresso) |
| `desktop_remove_media` | `mediaId`, `confirmProgress` | Remove explicitamente a mídia do catálogo local com confirmação de progresso |

O registro fica em `library.json` (versão 2), no diretório de configuração do aplicativo, com
caminhos canonicalizados e revalidados a cada carregamento. O formato anterior (versão 1, com uma
única raiz de biblioteca) é migrado para um curso com a identidade histórica `library-1`.

O rescan é uma atualização preservadora, não uma substituição: ele insere arquivos novos, atualiza
os conhecidos no mesmo caminho (reutilizando o `media_id`, inclusive após um arquivo ausente
reaparecer) e marca como ausentes os que deixaram de aparecer. Nenhum rescan exclui registros,
reescreve a identidade de mídia ou apaga o estado associado. A resposta `200` devolve apenas o
resumo sanitizado do núcleo — contagens, incluindo `ambiguous_count` — e nunca os caminhos
relativos das chaves ambíguas, que exporiam a estrutura da biblioteca.
No servidor Rust e no desktop, a remoção explícita de mídia é gravada como tombstone em
`media_removal`, chaveado pela identidade de mídia `media_id`. Nenhum rescan cria, apaga ou reescreve esse
registro, e o carimbo `removed_at` não é alterado por uma varredura. A leitura do catálogo
filtra os itens removidos, então a mídia sai de `/api/catalog` (e do `desktop_catalog`) e
`/api/media` responde 404, mesmo que o arquivo reapareça no disco depois. A remoção não apaga a
linha de catálogo nem o progresso de estudo associado, e repetir a remoção é idempotente. Um arquivo
que deixou de aparecer no scan também deixa de ser autorizado por `/api/media`, ainda que exista
uma resposta ou referência antiga em cache.

O `GET /api/catalog` do servidor Rust expõe a árvore recursiva canônica em `nodes` — o
contrato preferido pela SPA, com `media_id` obrigatório em cada arquivo, caminho relativo
completo, disponibilidade e verificação. Cada nó de topo é um curso registrado: papel `course`,
`path` vazio e a identidade opaca em `course_id`, com a subárvore de categorias, módulos e aulas
sob ele. Nenhuma aula e nenhum arquivo deixa de ser alcançável por `nodes`, inclusive os
arquivos soltos diretamente numa categoria, num curso ou num módulo. `categories` traz a mesma
lista de cursos de topo (`role: course`, `course_id` opaco e as mesmas subárvores), então um
consumidor que só lê `categories` também encontra todos os cursos.
O `desktop_catalog` expõe o mesmo par por IPC. O `complete: bool` do servidor Rust/desktop marca
o contrato canônico: presente, precisa ser booleano — um
`null`/string ou a ausência do campo é recusada — e o payload é normalizado estritamente pelo contrato Rust/desktop.
Sem nós canônicos, um `categories[].courses` no formato legado do Go é recusado de forma fail-closed em vez de virar conteúdo vazio.
Payloads sem o marcador `complete` são recusados como inválidos.

Ao normalizar, a SPA exige `media_id` não vazio nos arquivos dos nós canônicos Rust/desktop,
incluindo os arquivos soltos da raiz, e recusa de forma
determinística um payload que ultrapasse os limites explícitos de profundidade, número de nós ou
de arquivos — nunca truncando a árvore: o payload inteiro é rejeitado como inválido. Um `kind`
que a SPA ainda não conhece é preservado como material (balde catch-all) na normalização, no
roteador e na UI, de modo que abrir uma aula por deep-link ou pelo `media_id` nunca descarta o
arquivo.

## Estudo

O progresso é sempre do usuário autenticado: nenhuma rota aceita um `owner_id` vindo do
cliente, então uma sessão só lê e escreve o próprio progresso. Posição de retomada e
conclusão são independentes — avançar não desfaz a conclusão e concluir não move o ponto de
retomada — e reenviar um valor idêntico não altera o instante da última mudança.

Este contrato está implementado no servidor Rust e na SPA web
(paridade das entregas #86, #88, #90 e #91). O transporte desktop expõe o mesmo contrato por IPC,
descrito ao final desta seção; ele não usa rotas HTTP.

| Método | Rota | Autenticação | Finalidade |
| --- | --- | --- | --- |
| `GET` | `/api/study/recent?limit=N` | Sessão | Lista o progresso do usuário, do mais recente ao mais antigo (padrão 20, teto fixo de 100) |
| `GET` | `/api/study/analytics` | Sessão | Retorna contagens e tempo acumulado do estudo do usuário, excluindo mídias removidas |
| `GET` | `/api/study/progress?media_id=...` | Sessão | Lê a posição e a conclusão do usuário em uma mídia conhecida |
| `POST` | `/api/study/position` | Sessão | Aceita `{ "media_id": "...", "position_seconds": 0 }` e grava o ponto de retomada |
| `POST` | `/api/study/completed` | Sessão | Aceita `{ "media_id": "...", "completed": true }` e marca ou desmarca a conclusão |

Uma mídia conhecida sem progresso responde `200` com `recorded: false`; a ausência não é um
erro. Uma mídia desconhecida no catálogo responde `404 media_not_found`, uma posição negativa
responde `422 invalid_position` e uma consulta malformada responde `400 invalid_query`. As
mutações exigem a mesma origem (CSRF) e uma sessão que ainda precise trocar as credenciais
temporárias recebe `403 credentials_change_required`. Nenhuma rota grava `media`, `scan`,
`library` ou a biblioteca física; a mídia marcada como ausente continua aceitando leitura e
escrita de progresso. Uma mídia removida do catálogo também mantém a leitura e a escrita de
progresso — a identidade sobrevive à remoção —, mas deixa de aparecer em
`/api/study/recent`, como no núcleo de remoção.

### Study no desktop (IPC)

O desktop não tem rotas: a SPA chama comandos Tauri por `invoke`, sem `fetch`, e recebe o mesmo
shape snake_case (`media_id`, `position_seconds`, `completed`, `updated_at`, `recorded`).

| Comando | Argumentos | Finalidade |
| --- | --- | --- |
| `desktop_study_progress` | `mediaId` | Lê a posição e a conclusão do dono local em uma mídia conhecida |
| `desktop_record_position` | `mediaId`, `positionSeconds` | Grava o ponto de retomada, preservando a conclusão |
| `desktop_set_completed` | `mediaId`, `completed` | Marca ou desmarca a conclusão, preservando a posição |
| `desktop_recent_study` | `limit` (opcional) | Lista o progresso do dono local, do mais recente ao mais antigo |

O dono é um identificador local fixo resolvido no Rust: nenhum comando aceita `owner_id` como
argumento, então o webview não escolhe nem conhece o dono. Uma mídia conhecida sem progresso
responde com `recorded: false`; uma mídia desconhecida vira o erro estável `media_not_found`, uma
posição negativa vira `invalid_position` e as demais falhas viram `study_unavailable` ou
`database_unavailable`. Sem `limit`, o histórico usa o padrão do contrato e o teto do núcleo
continua valendo. Os comandos não exigem nenhum curso registrado e nunca gravam `media`,
`scan`, `library` nem as pastas de curso.

Limitação desta fatia: o desktop atende um único dono local, sem sessão nem seletor de contas.

Para limites de arquitetura e deploy, consulte a [visão de arquitetura](../architecture/overview.md).
