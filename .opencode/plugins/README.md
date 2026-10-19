# Plugin: guarda de branch protegida

`main-branch-guard.ts` é um plugin local do opencode que impede o agente de modificar o
repositório quando a branch atual é `main` ou `master`.

## O que ele faz

- Bloqueia as ferramentas de escrita do agente (`edit`, `write`, `apply_patch`/`patch`) para
  arquivos dentro do repositório (para as ferramentas de patch, os caminhos são lidos dos marcadores
  `*** Add File`, `*** Update File`, `*** Delete File` e `*** Move to`). O caminho é
  comparado por segmento: só sai do repositório quando o primeiro segmento é exatamente
  `..`, então um arquivo cujo nome começa com dois pontos (ex.: `..env`) continua
  protegido. A comparação usa o mesmo caminho resolvido contra as raízes conhecidas
  (`worktree` e diretório do projeto), então uma escrita no repositório continua
  bloqueada mesmo quando o `worktree` não é o repositório.
- Bloqueia comandos git mutantes executados via shell (`bash` ou `shell`): `commit`,
  `push`, `add`, `merge`, `rebase`, `reset`, `revert`, `stash`, `tag`,
  `checkout`/`switch` sem criação de branch e `gh pr merge`.
- Bloqueia chamadas de shell cujo comando não pode ser lido (argumento ausente ou em
  formato inesperado): quando a guarda não entende o que será executado, ela falha
  fechada em vez de tratar a chamada como inofensiva.
- Permite leitura, execução de testes/builds e a criação de branch
  (`git checkout -b` / `git switch -c`).
- Opera em **fail-closed**: quando não é possível confirmar a branch atual, operações
  mutantes são bloqueadas.
- Lê a branch com `node:child_process` (sem depender do shell `$` do runtime) e tenta
  `worktree` e `directory` como raiz. Quando a verificação falha, grava um diagnóstico em
  `learnvault-main-branch-guard.log` no diretório temporário do sistema e cita o caminho no
  erro devolvido ao agente.

## Nomes das ferramentas no runtime

A guarda precisa saber o nome exato da ferramenta para decidir se a operação é
mutante, e o opencode mudou esses nomes com o tempo: a ferramenta de shell aparece
como `bash` em runtimes antigos e como `shell` a partir do opencode v2, e a de patch
aparece como `apply_patch` ou como `patch`. A guarda reconhece **os dois nomes** de
cada ferramenta; sem isso ela não enxergaria o comando e a proteção não dispararia.

Nomes verificados no **opencode v2.0.32**. Se o runtime renomear uma ferramenta de novo,
a guarda volta a ficar inerte sem erro visível: confira o nome em
`packages/core/src/tool/plugin/` e acrescente o novo ao conjunto correspondente.

## Comportamento esperado do agente ao ser bloqueado

O erro orienta o agente a **não contornar** com outros comandos: ele deve informar o
usuário e perguntar se pode criar uma branch de trabalho antes de criar qualquer branch
ou repetir a operação.

## Limitações (proteção preventiva)

- É uma proteção preventiva contra erros do agente: **não substitui** a branch protection
  do GitHub nem o fluxo de pull request.
- Não bloqueia todas as formas de escrita via shell; a detecção de comandos usa padrões
  conhecidos e pode ser contornada por sintaxes alternativas.
- Alterações no plugin só entram em vigor após reiniciar o opencode.

## Contrato do módulo

O arquivo do plugin exporta **somente** a função do plugin. O carregador do opencode trata
cada export do módulo como um plugin e exige uma função (ou um objeto com `server`); um
export a mais impede o carregamento do arquivo inteiro. O teste
`main-branch-guard.test.mjs` verifica essa regra.

## Testes

```sh
node --test .opencode/plugins/main-branch-guard.test.mjs
```

O CI executa o mesmo comando no job `SPA checks`.
