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

## Staging da SPA canônica

O binário Linux do Go embute a interface web com `//go:embed web/build/*`, um caminho relativo a
`services/go/cmd/learnvault/`, enquanto o Vite escreve em `crates/server/web/build`. Por isso o job `linux` do
workflow de release copia `crates/server/web/build/.` para `services/go/cmd/learnvault/web/build/` antes de
testar e compilar o Go.

Sem esse passo o embed falha com `pattern web/build/*: no matching files found`, o job Linux morre e a
publicação fica ausente, porque o job de assets depende dele. Foi exatamente a causa da `v0.4.0` publicada sem
assets em 2026-10-17. Ao mexer no job `linux` do workflow de release, mantenha esse staging junto do build Go.

## Reparo de assets

Em evento `release`, o GitHub executa o arquivo de workflow **do commit da tag**, não o da `main`. Uma release
publicada antes de o `release.yml` corrigido estar na `main` fica sem assets, e a correção só alcança as tags
criadas depois dela. Para esses casos o workflow aceita disparo manual, restrito a quem tem permissão de escrita
no repositório:

1. Abra `Actions` → `Release` → `Run workflow`.
2. Informe a tag exata, no formato `vX.Y.Z`, de uma release já publicada.
3. O workflow valida a tag, confirma que a release existe e não é rascunho, reconstrói tudo a partir do commit da
   tag, confere que a tag não se moveu durante o build, reenvia os quatro assets com `--clobber` e acrescenta a
   seção **Contribuidores**.

O caminho de reparo nunca recria tag nem release: ele apenas reenvia assets para a mesma tag. A seção Contribuidores
é idempotente pelo marcador `<!-- learnvault:contributors -->`, então repetir o disparo não duplica a lista.

Evidência executada em 2026-10-17: o reparo foi usado nas releases `v0.3.0` e `v0.4.0` (runs `39006767838` e
`39004818355`) e as releases `v0.5.0` e `v0.6.0` saíram pelo caminho normal do evento `release` (runs `39005054542`
e `39006844672`). Nas quatro, o workflow anexou os quatro assets, o `SHA256SUMS` conferiu com o arquivo baixado e
a seção **Contribuidores** apareceu uma única vez.

## Convenção de nomes de assets

Cada release publica exatamente quatro arquivos, sempre com os mesmos nomes:

| Asset | Nome |
| --- | --- |
| Servidor Linux, x86-64 | `learnvault-linux-amd64.tar.gz` |
| Servidor Linux, ARM64 | `learnvault-linux-arm64.tar.gz` |
| Instalador Windows | `learnvault-windows-vMAJOR.MINOR.PATCH.exe` |
| Checksums SHA-256 | `SHA256SUMS` |

Regras que valem para qualquer ativo novo:

- O prefixo é sempre `learnvault-`; não crie alias, sufixo alternativo de arquitetura (`x64`, `win64`) nem nome
  comercial.
- O vocabulário de arquitetura é o do Go e do GitHub Actions: `amd64` e `arm64`, nunca `x86_64` ou `x64`.
- O instalador Windows carrega a tag no nome porque é o único arquivo que o usuário guarda fora da release; os
  tarballs não carregam, porque o binário embutido reporta a versão em `--version`.
- `SHA256SUMS` é sempre gerado pelo workflow com `sha256sum`, nunca à mão, e cobre exatamente os três assets
  anexados.
- O release candidate Rust mantém namespace separado (`rust-rc-vX.Y.Z-rc.N` e `SHA256SUMS.rust-rc`) porque é
  artefato experimental e não é a distribuição suportada.

O Release Please continua sendo a única fonte do `docs/releases/CHANGELOG.md` e do corpo de mudanças das notas. Depois de publicar os assets, o workflow acrescenta uma seção idempotente **Contribuidores**, derivada dos nomes de autor dos commits entre as tags verificadas; ele não gera uma segunda lista de mudanças nem usa notas geradas pelo GitHub. Use prefixos como `feat:`, `fix:`, `docs:` e `chore:` com descrições em pt-BR para manter as versões previsíveis e o changelog consistente.

O workflow exige o secret `RELEASE_PLEASE_TOKEN`; credenciais de automação devem ficar nos
secrets do GitHub Actions, nunca em arquivos versionados. O token precisa abrir PRs de release
e publicar tags/releases para que o workflow de artefatos reaja ao release publicado.

A configuração fica em `.github/release-please/release-please-config.json` e o manifesto em
`.github/release-please/release-please-manifest.json`. Em modo manifest, o config precisa
declarar a chave `packages`; sem ela o Release Please não prepara nenhuma release.

Migrações de banco devem ser documentadas nas notas de release. Releases nunca devem conter
a biblioteca física, bancos locais, credenciais ou arquivos de cache.
