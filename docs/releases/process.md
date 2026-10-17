# Processo de release

O LearnVault usa Semantic Versioning: `MAJOR.MINOR.PATCH`.

1. Faça merge das mudanças aprovadas com Conventional Commits na `main`.
2. O Release Please abre ou atualiza uma PR de release com a versão e as entradas do `docs/releases/CHANGELOG.md`.
3. Faça merge da PR de release; ela cria a tag semântica e o GitHub Release.
4. O workflow de release publicado constrói os tarballs Linux suportados e o instalador Windows `.exe`, injeta a versão da tag, publica os três assets com `SHA256SUMS` e atribui os autores dos commits da versão.
5. Verifique os assets, os checksums e a seção **Contribuidores** antes da distribuição.

O workflow de release recebe a tag publicada por variável de ambiente e valida que ela segue
o formato `vMAJOR.MINOR.PATCH`, com componentes numéricos sem zeros à esquerda, antes de
usá-la. A tag nunca é interpolada diretamente em comandos de shell.

O Release Please continua sendo a única fonte do `docs/releases/CHANGELOG.md` e do corpo de mudanças das notas. Depois de publicar os assets, o workflow acrescenta uma seção idempotente **Contribuidores**, derivada dos nomes de autor dos commits entre as tags verificadas; ele não gera uma segunda lista de mudanças nem usa notas geradas pelo GitHub. Use prefixos como `feat:`, `fix:`, `docs:` e `chore:` com descrições em pt-BR para manter as versões previsíveis e o changelog consistente.

O workflow exige o secret `RELEASE_PLEASE_TOKEN`; credenciais de automação devem ficar nos
secrets do GitHub Actions, nunca em arquivos versionados. O token precisa abrir PRs de release
e publicar tags/releases para que o workflow de artefatos reaja ao release publicado.

A configuração fica em `.github/release-please/release-please-config.json` e o manifesto em
`.github/release-please/release-please-manifest.json`. Em modo manifest, o config precisa
declarar a chave `packages`; sem ela o Release Please não prepara nenhuma release.

Migrações de banco devem ser documentadas nas notas de release. Releases nunca devem conter
a biblioteca física, bancos locais, credenciais ou arquivos de cache.
