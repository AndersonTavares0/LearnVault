# Arquitetura

O LearnVault é um serviço portátil único. Pode rodar como executável nativo ou por meio de
um adaptador opcional de container:

```text
Proxy HTTPS local
        |
        v
Serviço HTTP Go -- SQLite (/data)
        |-- cache (/cache)
        `-- biblioteca de cursos (/library:ro)
```

A aplicação SvelteKit é construída como SPA estática e embutida no binário Go. O serviço Go
é responsável por autenticação, autorização, persistência do catálogo e entrega autenticada
de mídia.

A biblioteca física é externa e somente leitura. O SQLite contém metadados e estado pessoal
de estudo, não mídia de cursos. Cache e mídia derivada são descartáveis. Ações administrativas
são explícitas; a aplicação não faz novo scan silenciosamente na inicialização.

Podman rootless com Quadlet e Docker Compose são adaptadores de deploy, não dependências da
aplicação. Builds nativos para desktop e servidor continuam sendo prioridades. Dados duráveis
e cache descartável usam locais separados em todos os adaptadores.

Consulte [distribuição](../operations/distribution.md) para executáveis e pacotes.
