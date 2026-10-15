## Resumo

<!-- Explique o impacto para o usuário em poucas frases. Remova este comentário. -->

## Issue e governança

- Issue relacionada: <!-- use "Não aplicável" para typo, dependência ou CI simples. -->
- [ ] Esta mudança não altera governança, permissões, checks obrigatórios ou releases.
- [ ] O impacto de governança está descrito abaixo e documentado em `docs/governance/overview.md`.

<!-- Explique mudanças de governança, permissões, proteção de branch ou releases. -->

## Alterações

- <!-- Liste as alterações relevantes. -->

## Validação

- <!-- Liste somente comandos executados e seus resultados. -->
- [ ] `pnpm install --frozen-lockfile`
- [ ] `pnpm run check`
- [ ] `pnpm run build`
- [ ] `pnpm audit --audit-level=low`
- [ ] `go test -race ./...`
- [ ] `go vet ./...`
- [ ] `cargo fmt --all -- --check` e `cargo clippy --all-targets -- -D warnings`
- [ ] `cargo test --all` ou `cargo llvm-cov --package learnvault-core --all-targets --fail-under-lines 80`
- [ ] Validação de Container/Compose, com o comando exato

## Segurança e idioma

- [ ] Não incluí arquivos de cursos, credenciais, dados pessoais ou segredos privados.
- [ ] Os testes usam fixtures isoladas e não acessam dados externos.
- [ ] O comportamento de biblioteca somente leitura foi preservado.
- [ ] Migrações e documentação de deploy foram atualizadas, quando necessário.
- [ ] Mudanças de interface incluem screenshot ou descrição visual breve.
- [ ] O idioma está consistente: arquivos de comunidade e documentação em pt-BR; apenas a LICENSE permanece em inglês.
- [ ] O prefixo do commit está em inglês e a descrição em pt-BR para o Release Please.

<!-- Remova itens não aplicáveis e registre limitações conhecidas. -->
