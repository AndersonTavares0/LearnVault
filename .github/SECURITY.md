# Política de segurança

## Versões suportadas

Apenas a versão mais recente publicada na branch padrão recebe correções de segurança no
momento. Snapshots de desenvolvimento podem conter comportamento inacabado.

## Reportar uma vulnerabilidade

Não abra uma issue pública para uma suspeita de vulnerabilidade. Use o relato privado de
security advisories do GitHub para este repositório ou contate o mantenedor de forma privada
pelo endereço configurado nas configurações de segurança do GitHub.

Inclua a versão afetada, passos de reprodução, impacto e logs com segredos removidos. Nunca
inclua arquivos de cursos, senhas, cookies de sessão, tokens de API ou detalhes de rede
privada.

Mantenha o `.env` e os dados persistentes fora do repositório. Execute o contêiner com a
biblioteca montada somente para leitura e use HTTPS por meio do proxy reverso local.
