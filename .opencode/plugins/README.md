# Plugin: guarda de branch protegida

`main-branch-guard.ts` é um plugin local do opencode que impede o agente de modificar o
repositório quando a branch atual é `main` ou `master`.

## O que ele faz

- Bloqueia as ferramentas de escrita do agente (`edit`, `write`, `apply_patch`) para
  arquivos dentro do repositório (para `apply_patch`, os caminhos são lidos dos marcadores
  `*** Add File`, `*** Update File`, `*** Delete File` e `*** Move to`).
- Bloqueia comandos git mutantes via Bash (`commit`, `push`, `add`, `merge`, `rebase`,
  `reset`, `revert`, `stash`, `tag`, `checkout`/`switch` sem criação de branch) e
  `gh pr merge`.
- Permite leitura, execução de testes/builds e a criação de branch
  (`git checkout -b` / `git switch -c`).
- Opera em **fail-closed**: quando não é possível confirmar a branch atual, operações
  mutantes são bloqueadas.
- Lê a branch com `node:child_process` (sem depender do shell `$` do runtime) e tenta
  `worktree` e `directory` como raiz. Quando a verificação falha, grava um diagnóstico em
  `learnvault-main-branch-guard.log` no diretório temporário do sistema e cita o caminho no
  erro devolvido ao agente.

## Comportamento esperado do agente ao ser bloqueado

O erro orienta o agente a **não contornar** com outros comandos: ele deve informar o
usuário e perguntar se pode criar uma branch de trabalho antes de criar qualquer branch
ou repetir a operação.

## Limitações (proteção preventiva)

- É uma proteção preventiva contra erros do agente: **não substitui** a branch protection
  do GitHub nem o fluxo de pull request.
- Não bloqueia todas as formas de escrita via Bash; a detecção de comandos usa padrões
  conhecidos e pode ser contornada por sintaxes alternativas.
- Alterações no plugin só entram em vigor após reiniciar o opencode.

## Testes

```sh
node --test .opencode/plugins/main-branch-guard.test.mjs
```

O CI executa o mesmo comando no job `SPA checks`.
