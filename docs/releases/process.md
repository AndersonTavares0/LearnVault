# Processo de release

O LearnVault usa Semantic Versioning: `MAJOR.MINOR.PATCH`.

1. Faça merge das mudanças aprovadas com Conventional Commits na `main`.
2. O Release Please abre ou atualiza uma PR de release com a versão e as entradas do `docs/releases/CHANGELOG.md`.
3. Faça merge da PR de release; ela cria a tag semântica e o GitHub Release.
4. O workflow de release publicado constrói os artefatos Linux, injeta a versão da tag e envia `SHA256SUMS`.
5. Verifique os assets e os checksums antes da distribuição.

O workflow de release recebe a tag publicada por variável de ambiente e valida que ela segue
o formato `vMAJOR.MINOR.PATCH` antes de usá-la. A tag nunca é interpolada diretamente em
comandos de shell.

O Release Please é o único responsável pelo `docs/releases/CHANGELOG.md` e pelas notas do
release. Não use notas geradas pelo GitHub como uma segunda fonte. Use prefixos como `feat:`,
`fix:`, `docs:` e `chore:` com descrições em pt-BR para manter as versões previsíveis e o
changelog consistente.

O workflow exige o secret `RELEASE_PLEASE_TOKEN`; credenciais de automação devem ficar nos
secrets do GitHub Actions, nunca em arquivos versionados. O token precisa abrir PRs de release
e publicar tags/releases para que o workflow de artefatos reaja ao release publicado.

A configuração fica em `.github/release-please/release-please-config.json` e o manifesto em
`.github/release-please/release-please-manifest.json`. Em modo manifest, o config precisa
declarar a chave `packages`; sem ela o Release Please não prepara nenhuma release.

Migrações de banco devem ser documentadas nas notas de release. Releases nunca devem conter
a biblioteca física, bancos locais, credenciais ou arquivos de cache.
