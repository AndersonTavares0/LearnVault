# Wiki do LearnVault

O LearnVault transforma pastas de cursos que você já tem em um espaço de estudo próprio. Ele lê os
arquivos, monta um catálogo navegável com cursos, aulas e materiais, e guarda seu progresso. Os
arquivos originais ficam onde estão: o LearnVault nunca os move, altera ou copia.

A Wiki é a porta de entrada para entender o produto, rodar o projeto e contribuir. A documentação
em `docs/` é a fonte da verdade; a Wiki é uma cópia navegável das páginas mapeadas.

## Por onde começar

- **Quero usar ou rodar o projeto:** [[Getting-Started]].
- **Quero entender como as peças se encaixam:** [[Architecture]].
- **Quero saber o que vem por aí:** [[Roadmap]].
- **Preciso citar um requisito:** [[Requirements]].
- **Quero contribuir ou revisar uma mudança:** [[Development-and-Governance]].
- **Vou instalar ou operar um servidor:** [[Operations]].

## Dois modos de uso

No **servidor self-hosted**, uma pessoa administra a biblioteca e as contas; as demais acessam pela
rede com login. No **desktop offline**, uma pessoa usa o aplicativo na própria máquina, sem login e
sem abrir portas de rede.

## Como o código está dividido

- `crates/core` concentra as regras de domínio, o catálogo e o progresso de estudo.
- `crates/server` expõe a API HTTP e entrega a interface web (SPA).
- `crates/desktop` empacota a mesma interface em uma janela Tauri.
- `apps/web` contém a interface Svelte 5 + Vite usada nos dois modos.

O backend Go foi removido. O servidor Rust ainda importa bancos Go antigos quando o operador pede,
mas o projeto não executa mais o serviço antigo.

## Termos que aparecem na Wiki

- **Biblioteca:** a pasta de cursos que você já tem. O LearnVault lê essa pasta e não altera nada
  nela.
- **Catálogo:** o índice navegável que o LearnVault monta com cursos, aulas, materiais e legendas.
- **Releitura (rescan):** a ação explícita de reler a biblioteca para encontrar arquivos novos ou
  ausentes.
- **SPA:** a interface web de página única usada no navegador e dentro do desktop.
- **IPC:** o canal de comandos entre a janela do desktop e o código Rust que acessa arquivos.

## O que está disponível hoje

O instalador Windows NSIS e o tarball Linux do servidor são publicados a cada release. O desktop
Linux/Flatpak e a publicação da imagem OCI ainda dependem de validação. O [[Roadmap]] mostra o que
falta e a página [[Operations]] detalha cada formato.
