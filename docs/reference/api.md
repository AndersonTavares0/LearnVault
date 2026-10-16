# Referência da API

A implementação Go permanece a referência funcional. O servidor Rust expõe as mesmas APIs abaixo
e a SPA web usa o mesmo contrato de Study nos dois transportes. Todas as rotas usam JSON, salvo
indicação contrária. A autenticação usa um cookie de sessão HttpOnly com SameSite=Strict.

No servidor Rust, a raiz da biblioteca deve permanecer sob controle confiável do processo entre
as operações administrativas. O rescan recanonicaliza a raiz e recusa mudanças detectáveis, mas
trocas concorrentes de junctions, symlinks ou reparse points durante a abertura ainda dependem
das garantias específicas do sistema operacional; essa limitação não deve ser tratada como
isolamento absoluto contra um escritor local concorrente.

Na implementação Rust, o cookie também recebe `Secure` por padrão e só é enviado pelo
navegador em HTTPS. Para desenvolvimento local sem TLS, use explicitamente `--insecure-http`;
esse modo não deve ser usado em uma instalação exposta na rede. Mutações de autenticação exigem
uma origem HTTP(S) compatível, além de SameSite=Strict.

## Entrega da SPA no servidor Rust

A entrega é opcional e fica desabilitada por padrão. Use `--web-root` apontando para o diretório
gerado pelo build estático (`services/go/cmd/learnvault/web/build`), que contém o shell `200.html`. Arquivos
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
Ele não cria `admin/admin`, não converte automaticamente e não sobrescreve dados. Faça uma cópia
de segurança do arquivo antes de qualquer migração; a conversão do banco Go está fora desta fatia
e será publicada separadamente. Um banco novo vazio e um banco Rust existente continuam sendo
abertos normalmente.

## Catálogo e mídia

| Método | Rota | Autenticação | Finalidade |
| --- | --- | --- | --- |
| `GET` | `/api/catalog` | Sessão | Retorna a árvore persistida de categorias, cursos, módulos e aulas |
| `POST` | `/api/admin/catalog/rescan` | Sessão admin | Faz scan explícito de `LIBRARY_DIR` e atualiza o catálogo de forma preservadora: insere arquivos novos, reafirma os conhecidos e marca ausentes os que sumiram, sem excluir nenhum registro |
| `GET`, `HEAD` | `/api/media?path=...` | Sessão | Transmite um arquivo aprovado da biblioteca com suporte a HTTP Range |

O scanner lê somente entradas e metadados de diretórios. Links simbólicos não são seguidos — nem
a raiz, nem entradas internas. Ignora diretórios temporários de download, classifica mídias,
legendas e materiais reconhecidos, e trata `.ts` como vídeo somente quando é maior que 1 MiB para
não confundir TypeScript com MPEG-TS. A biblioteca física é externa, montada como somente leitura e
nunca é modificada.

O rescan é uma atualização preservadora, não uma substituição: ele insere arquivos novos, atualiza
os conhecidos no mesmo caminho (reutilizando o `media_id`, inclusive após um arquivo ausente
reaparecer) e marca como ausentes os que deixaram de aparecer. Nenhum rescan exclui registros,
reescreve a identidade de mídia ou apaga o estado associado. A resposta `200` devolve apenas o
resumo sanitizado do núcleo — contagens, incluindo `ambiguous_count` — e nunca os caminhos
relativos das chaves ambíguas, que exporiam a estrutura da biblioteca.

O catálogo Go adota uma política fail-closed e persistente para remoções. Uma remoção
explícita é gravada como tombstone em `media_removal`, chaveado pelo caminho relativo canônico
completo — a identidade de mídia do serviço Go. Nenhum rescan cria, apaga ou reescreve esse
registro, e o carimbo `removed_at` não é alterado por uma varredura. A leitura do catálogo
filtra os caminhos removidos, então a mídia sai de `/api/catalog` e `/api/media` responde 404,
mesmo que o arquivo reapareça no disco depois. A remoção não apaga a linha de catálogo nem
qualquer dado associado, e repetir a remoção é idempotente. Um arquivo que deixou de aparecer
no scan também deixa de ser autorizado por `/api/media`, ainda que exista uma resposta ou
referência antiga em cache.

O `GET /api/catalog` do servidor Rust expõe a árvore recursiva canônica em `nodes` — o
contrato preferido pela SPA, com `media_id`, caminho relativo completo, disponibilidade e
verificação em cada arquivo — e mantém `categories` na forma legada do serviço Go
(`categories[].courses[].modules[].lessons[]`), para os consumidores antigos. Os dois campos
são shapes diferentes de propósito: a projeção legada desce por nós intermediários e preserva
as aulas e seus arquivos, enquanto o que não cabe na gramática legada — arquivos soltos de
categoria, curso ou módulo e nós de topo que não são categoria — permanece apenas em `nodes`.
O `desktop_catalog` expõe o mesmo par por IPC, e a SPA continua aceitando um payload apenas com
`categories` no formato legado do serviço Go como fallback durante a transição.

Ao normalizar, a SPA exige `media_id` não vazio nos arquivos dos nós canônicos Rust/desktop (o
payload aditivo e o fallback legado do Go não emitem identidade e seguem sem ela) e recusa de
forma determinística um payload que ultrapasse os limites explícitos de profundidade, número de
nós ou de arquivos — nunca truncando a árvore: o payload inteiro é rejeitado como inválido.

## Estudo

O progresso é sempre do usuário autenticado: nenhuma rota aceita um `owner_id` vindo do
cliente, então uma sessão só lê e escreve o próprio progresso. Posição de retomada e
conclusão são independentes — avançar não desfaz a conclusão e concluir não move o ponto de
retomada — e reenviar um valor idêntico não altera o instante da última mudança.

Este contrato está implementado no servidor Rust, na SPA web e no serviço Go de referência
(paridade das entregas #86, #88, #90 e #91). O transporte desktop expõe o mesmo contrato por IPC,
descrito ao final desta seção; ele não usa rotas HTTP.

| Método | Rota | Autenticação | Finalidade |
| --- | --- | --- | --- |
| `GET` | `/api/study/recent?limit=N` | Sessão | Lista o progresso do usuário, do mais recente ao mais antigo (padrão 20, teto fixo de 100) |
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
continua valendo. Os comandos não exigem raiz de biblioteca configurada e nunca gravam `media`,
`scan`, `library` nem a biblioteca física.

Limitação desta fatia: o desktop atende um único dono local, sem sessão nem seletor de contas, e
ainda não entrega a mídia ao player — sem reprodução não há posição real, então a barra de estudo
do player permanece fechada (a entrega parcial de mídia está em #76, Draft, não mesclada).

Para limites de arquitetura e deploy, consulte a [visão de arquitetura](../architecture/overview.md).
