# Contribuindo com o LearnVault

Obrigado por contribuir com o LearnVault. As contribuições devem preservar o foco do
projeto: uma biblioteca de estudos pequena, sustentável e autocontida, que nunca modifica
a biblioteca de cursos de origem.

## Antes de abrir uma pull request

1. Procure issues, Discussions e pull requests existentes.
2. Use uma Discussion quando ainda houver dúvida ou alternativas de design.
3. Abra uma issue antes de uma feature ou mudança de comportamento.
4. Para typo, dependência ou ajuste simples de CI, a issue é opcional.
5. Mantenha a mudança focada e explique o comportamento afetado.
6. Nunca inclua arquivos de cursos, credenciais, endereços privados, bancos de dados ou mídia gerada.

## Checks de desenvolvimento

```sh
pnpm install --frozen-lockfile
pnpm run check
pnpm run build
pnpm audit --audit-level=low
go test ./...
go vet ./...
```

Os testes devem usar fixtures e diretórios temporários; não devem acessar dados externos.

## Pull requests

- Use um título claro, como `feat: adicionar persistência de progresso da aula`.
- Descreva o problema, a solução e a validação realizada.
- Inclua screenshots para mudanças de interface.
- Documente migrações e mudanças de deploy.
- Adicione ou atualize testes para mudanças de comportamento.
- Relacione a issue quando ela existir.

## Fluxo de colaboração

```text
Discussion (se houver dúvida)
        ↓
Issue (feature ou mudança de comportamento)
        ↓
Branch no mesmo repositório
        ↓
Pull request
        ↓
Revisão e checks
        ↓
Squash merge em main
```

Forks não fazem parte do fluxo atual. A equipe trabalha em branches dentro do mesmo
repositório; essa decisão pode ser revista quando a equipe crescer.

1. Assuma a issue adicionando seu usuário como `assignee`; cada issue deve ter uma pessoa responsável.
2. Crie uma branch a partir de `main`, por exemplo `feat/lesson-progress` ou `fix/catalog-scan`.
3. Faça a mudança e envie a branch para este repositório.
4. Abra a pull request e relacione a issue com `Closes #<número>` quando aplicável.
5. Aguarde o CI, responda aos comentários e mantenha a branch focada.
6. Resolva as conversas e mantenha todos os checks obrigatórios verdes.
7. Faça o merge pelo GitHub usando squash merge; nunca envie diretamente para `main`.

## Estilo de commits

Use Conventional Commits. Os prefixos são fixos em inglês e as descrições devem ser escritas
em pt-BR, mantendo o `CHANGELOG.md` consistente com o Release Please:

```text
feat: adicionar persistência de progresso da aula
fix: tratar sessões expiradas
docs: atualizar guia de contribuição
chore: atualizar dependências
```

O `pnpm` é o gerenciador oficial do frontend. Não use `npm` para alterar dependências ou
o lockfile.

## Política clean-room

Pesquisas em plataformas de cursos podem orientar padrões gerais de produto, mas não copie
código, assets, marca, textos proprietários ou layouts em nível de pixel. A interface do
LearnVault deve manter identidade visual própria.
