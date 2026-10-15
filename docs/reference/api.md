# Referência da API

A implementação Go permanece a referência funcional. O servidor Rust expõe as mesmas APIs
abaixo e pode servir a SPA estática quando configurado. Todas as rotas usam JSON, salvo indicação
contrária. A autenticação usa um cookie de sessão HttpOnly com SameSite=Strict.

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

## Catálogo e mídia

| Método | Rota | Autenticação | Finalidade |
| --- | --- | --- | --- |
| `GET` | `/api/catalog` | Sessão | Retorna a árvore persistida de categorias, cursos, módulos e aulas |
| `POST` | `/api/admin/catalog/rescan` | Sessão admin | Faz scan explícito de `LIBRARY_DIR` e substitui o catálogo |
| `GET`, `HEAD` | `/api/media?path=...` | Sessão | Transmite um arquivo aprovado da biblioteca com suporte a HTTP Range |

O scanner lê somente entradas e metadados de diretórios. Ignora diretórios temporários de
download, classifica mídias, legendas e materiais reconhecidos, e trata `.ts` como vídeo
somente quando é maior que 1 MiB para não confundir TypeScript com MPEG-TS. A biblioteca
física é externa, montada como somente leitura e nunca é modificada.

Para limites de arquitetura e deploy, consulte a [visão de arquitetura](../architecture/overview.md).
