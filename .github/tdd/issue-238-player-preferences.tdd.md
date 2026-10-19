# Evidências TDD — issue #238

## Origem e jornadas

Origem: [issue #238](https://github.com/AndersonTavares0/LearnVault/issues/238).

- Como estudante, quero que velocidades de áudio e vídeo sejam lembradas independentemente entre sessões.
- Como estudante, quero escolher mídia e legenda por aula, inclusive desligar legendas, e recuperar essas escolhas depois.
- Como estudante, quero atalhos de teclado descritos e disponíveis somente quando o grupo do player está focado.
- Como estudante, quero optar pelo avanço automático para a próxima aula na sequência do catálogo, sem avanço automático por padrão.
- Como estudante, quero manter reprodução, legendas, erros de carregamento, progresso e retomada existentes.

## Evidência dos checkpoints

A implementação foi integrada em uma branch separada da `main`; a worktree/branch de origem `feat/player-controls` foi preservada. Os nove checkpoints anteriores continuam na sequência da branch atual: testes e implementação de preferências; testes e implementação da seleção lembrada; testes e implementação de atalhos/autoavanço; testes e integração dos controles; atualização do roadmap. O histórico registra RED/GREEN em pares de commits; os logs RED originais desses pares não foram mantidos, então não são reproduzidos como evidência aqui.

Para a melhoria de acessibilidade nesta integração:

1. Commit `a8a9ba8` adicionou um teste SSR que exige a descrição visual e `aria-keyshortcuts`.
2. `pnpm exec vitest run src/player.test.ts` foi executado antes da implementação: 20 testes passaram e o novo teste falhou porque o atributo e o hint estavam ausentes (RED intencional).
3. Commit `b897975` adicionou o hint visual em pt-BR, `aria-keyshortcuts="Space ArrowLeft ArrowRight"` e comentário de acessibilidade em pt-BR.
4. O mesmo comando foi executado após a implementação: 21 testes passaram (GREEN); `pnpm exec svelte-check --tsconfig ./tsconfig.json` encontrou 0 erros e 0 avisos.

## Garantias verificadas

| Garantia | Teste | Tipo | Resultado |
|---|---|---|---|
| Preferências persistem, são normalizadas e isoladas pelo escopo da conta/dispositivo; storage indisponível não impede reprodução | `apps/web/src/player-preferences.test.ts` | Unitário | PASS na suíte Vitest |
| A mídia/legenda lembrada é restaurada; escolhas removidas recaem na primeira elegível; legenda pode ser desligada | `apps/web/src/player.test.ts`, `player-preferences.test.ts` | Unitário/SSR | PASS na suíte Vitest |
| Velocidade de áudio e vídeo é independente; autoplay inicia desligado | `apps/web/src/player.test.ts` | SSR | PASS na suíte Vitest |
| Atalhos Space/←/→ só são elegíveis com foco no grupo; o player documenta-os visual e semanticamente | `apps/web/src/player.test.ts`, teste `identifica os atalhos do player visual e semanticamente` | Unitário/SSR | RED observado antes; GREEN depois |
| Autoavanço só ocorre quando habilitado e há próxima aula | `apps/web/src/player.test.ts` | Unitário | PASS na suíte Vitest |
| Progresso, retomada e disponibilidade mantêm as garantias existentes | `apps/web/src/player.test.ts` e `apps/web/src/study.test.ts` | Unitário/SSR | PASS na suíte Vitest |

## Verificação final e lacunas

Após integrar na main atualizada, `pnpm exec vitest run` passou com 17 arquivos e 263 testes; `pnpm exec svelte-check --tsconfig ./tsconfig.json` passou com 0 erros/0 avisos; `pnpm verify` completo terminou com “Todos os checks passaram”, incluindo builds SPA/desktop, auditoria de dependências, fmt, clippy e testes Rust do workspace. Não foi executada interação em browser real/WebView/Tauri; SSR e testes unitários não substituem essa validação. Nenhum comando de cobertura foi executado nesta integração.
