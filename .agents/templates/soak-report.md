# Relatório de soak do artefato Rust (G2)

> Registro factual de uma execução. Não inclua credenciais, tokens, cookies, bancos de
> dados, mídia ou outros dados privados neste relatório.

## Identificação

- Versão reportada pelo binário: `______________________________`
- Tag da draft: `______________________________`
- `run_id` de origem: `______________________________`
- SHA-256 declarado: `______________________________`
- SHA-256 conferido: `______________________________`
- Data/hora de início (com fuso): `______________________________`
- Duração total: `______________________________`
- Host / SO / arquitetura: `______________________________`
- Como o artefato foi obtido: `______________________________`

## Origem dos dados

- Biblioteca sintética ou cópia de biblioteca real usada (sem anexar dados):
  `____________________________________________________________`
- Houve importação `migrate-go`? `sim / não`
- Se sim, caminhos usados (somente registre os caminhos; não anexe os arquivos):
  - input: `__________________________________________________`
  - backup: `________________________________________________`
  - output: `________________________________________________`

## Verificações

| Verificação | Resultado observado | Resultado esperado | Passou? |
| --- | --- | --- | --- |
| Health HTTP 200, `cache-control: no-store` e versão correta | | 200; `no-store`; versão igual à identificada acima | | |
| Login | | Login concluído e sessão aceita nas chamadas seguintes | | |
| Catálogo | | Catálogo retornado sem erro inesperado | | |
| Reprodução de mídia com `Range` | | Resposta parcial `206` para o intervalo solicitado | | |
| Persistência após reinício | | Dados e sessão/estado verificável continuam disponíveis conforme o cenário | | |
| Ausência de 4xx/5xx inesperado | | Nenhum 4xx/5xx fora dos resultados esperados e registrados | | |

## Recursos

- CPU aproximada: `______________________________`
- RSS aproximado: `______________________________`
- Reinícios do processo (quantidade e horários): `______________________________`

## Falhas e anomalias

Liste cada ocorrência com horário e evidência observável (por exemplo, saída de comando ou
status HTTP), sem incluir segredos ou dados privados:

1. `__________________________________________________________________________`
2. `__________________________________________________________________________`
3. `__________________________________________________________________________`

## Parecer final do operador

- Parecer: `aceito / aceito com ressalvas / rejeitado`
- Justificativa: `____________________________________________________________`
- Nome de quem assina: `______________________________`
- Data/hora da assinatura (com fuso): `______________________________`

## Limites

Este relatório não prova produção. O artefato avaliado pode ser não assinado ou uma build de
teste, e não substitui uma release publicada; as releases publicadas são as versões
suportadas. O registro também não substitui a revisão das demais evidências do projeto.
