<div align="center">
  <img src="apps/web/public/favicon.svg" alt="" width="64" height="64">
  <h1>LearnVault</h1>
  <p>Seus cursos, organizados em um espaço de estudo que você controla.</p>
  <p>
    <a href="https://github.com/AndersonTavares0/LearnVault/actions/workflows/ci.yml"><img src="https://github.com/AndersonTavares0/LearnVault/actions/workflows/ci.yml/badge.svg" alt="CI"></a>
    <a href="LICENSE"><img src="https://img.shields.io/badge/licen%C3%A7a-MIT-4f8f68.svg" alt="Licença MIT"></a>
  </p>
  <p>
    <a href="https://github.com/AndersonTavares0/LearnVault/releases/latest">Baixar</a> ·
    <a href="#o-que-voc%C3%AA-pode-fazer">Conhecer o app</a> ·
    <a href="#contribuir">Contribuir</a>
  </p>
</div>

## Estude no seu ritmo

O LearnVault organiza cursos que você já tem em pastas. Ele cria um catálogo para encontrar aulas, assistir a vídeos, ouvir áudios e acompanhar seu progresso, sem mover nem alterar os arquivos originais.

Você pode usar o servidor pela Web ou abrir a biblioteca no aplicativo desktop offline.

## O que você pode fazer

- Reunir cursos e materiais locais em um catálogo pesquisável.
- Navegar por módulos e aulas e continuar de onde parou.
- Reproduzir vídeos e áudios ou baixar materiais e legendas.
- Manter os arquivos originais no lugar: o LearnVault lê a pasta do curso, mas não a altera.

## Escolha como usar

| | Servidor + Web | Desktop offline |
| --- | --- | --- |
| Para quem | Quer acessar pelo navegador ou hospedar na própria rede | Quer estudar localmente, sem conta ou servidor |
| Disponibilidade | Tarballs Linux para x86-64 e ARM64 | Instalador Windows `.exe` |
| Linux desktop | Não há pacote publicado | Flatpak continua em validação |

Baixe os arquivos disponíveis na [página de releases](https://github.com/AndersonTavares0/LearnVault/releases/latest). A distribuição Flatpak e a publicação OCI para instalação self-hosted ainda não estão disponíveis. Consulte o [guia de distribuição](docs/operations/distribution.md) antes de instalar ou atualizar.

## Começar a desenvolver

O LearnVault está em desenvolvimento ativo. Para rodar a versão local, instale Rust 1.99, Node.js 24 e pnpm 11.28.5. Docker é opcional.

```sh
pnpm install --frozen-lockfile
pnpm dev
```

Abra a URL da SPA exibida no terminal. `pnpm dev` também inicia o servidor local; `pnpm dev:tauri` abre o aplicativo desktop durante o desenvolvimento.

Antes de enviar uma mudança, rode:

```sh
pnpm verify
```

Veja o [guia de contribuição](.github/CONTRIBUTING.md) para os checks, convenções de PR, segurança dos dados e configuração completa.

## Contribuir

1. Procure uma issue ou Discussion existente. Abra uma issue antes de propor mudança de comportamento; use Discussion quando a solução ainda estiver em debate.
2. Faça uma alteração focada e teste o fluxo afetado. Para mudanças de interface, inclua uma captura de tela na pull request.
3. Não envie cursos, credenciais, dados pessoais, bancos locais ou arquivos gerados.
4. Abra uma pull request com o problema, a solução e os testes executados. Aguarde a revisão e os checks obrigatórios.

O projeto usa `pnpm` para dependências do frontend e Conventional Commits para títulos e commits. Consulte [CONTRIBUTING.md](.github/CONTRIBUTING.md) antes de começar.

## Releases

O Release Please prepara versões e changelog. Cada release publicada oferece o instalador Windows, tarballs do servidor Linux e checksums. A lista automática de contribuidores ignora commits identificados como Dependabot. O [processo de release](docs/releases/process.md) explica a publicação e os assets.

Flatpak, distribuição desktop Linux e publicação OCI/self-hosted permanecem no [roadmap](docs/product/roadmap.md); não são formatos de release disponíveis hoje.

## Mais informações

- [Guia de início e uso](docs/wiki/getting-started.md)
- [Documentação do projeto](docs/README.md)
- [Arquitetura](docs/architecture/overview.md)
- [Roadmap](docs/product/roadmap.md)
- [Reportar vulnerabilidade](.github/SECURITY.md)
- [Licença MIT](LICENSE)
