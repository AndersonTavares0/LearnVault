# LearnVault — contexto de produto

## Público e objetivo

Pessoas que estudam com cursos armazenados em pastas próprias precisam encontrar uma aula, reproduzi-la e retomar o estudo sem reorganizar os arquivos. O LearnVault atende tanto quem usa o servidor self-hosted no navegador quanto quem usa o aplicativo desktop offline, sem conta ou servidor.

## Jornadas prioritárias

1. Selecionar ou importar uma biblioteca e localizar um curso ou aula.
2. Entender o conteúdo do curso, reproduzir a aula e registrar a conclusão.
3. Retomar o estudo, iniciar uma sessão de foco e consultar a evolução pessoal.
4. Ajustar aparência e foco sem perder o contexto da tarefa.

## Limites do produto

A biblioteca original é somente leitura. Rescan é explícito e preserva registros e progresso; mídias ausentes precisam ser identificadas sem insinuar que o arquivo foi apagado. O desktop funciona offline; a interface não deve pressupor serviços em nuvem. A conclusão da aula é independente da posição de reprodução.

## Princípios de experiência

- Orientar a próxima ação, especialmente em estados vazios, erros e mídia ausente.
- Mostrar progresso pessoal com escopo e significado claros, sem confundi-lo com contagens do acervo.
- Preservar navegação por teclado, foco visível, contraste, movimento reduzido e rótulos acessíveis.
- Trabalhar em janelas desktop e telas menores, nos temas claro e escuro.
- Não substituir evidência de funcionamento por protótipos ou dados fictícios.

Requisitos completos e invariantes: [requisitos do produto](docs/product/requirements.md). Estado de entrega: [roadmap](docs/product/roadmap.md). O redesign e suas evidências são acompanhados em [design review](docs/design-review.md).

## Preferência visual declarada

Para este redesign, o usuário quer uma apresentação mais próxima de plataformas de aprendizagem como Udemy e Coursera: descoberta de cursos clara, cursos visualmente identificáveis, resumo e currículo legíveis e progresso fácil de entender. São referências de qualidade e padrões familiares, não interfaces a copiar. O LearnVault permanece uma biblioteca local-first; não importar compra, preço, matrícula, assinatura, rating ou sincronização em nuvem sem suporte real do produto.

## Compromisso de marca

- A marca deve incluir uma logo própria, discreta e reconhecível em tamanhos pequenos. Evitar cores chamativas, arco-íris, gradientes, brilho, mascotes decorativos e wordmarks excessivamente promocionais.
- Preferir uma marca tipográfica ou símbolo monocromático/duotônico de baixa saturação, com boa leitura no shell desktop, favicon e tema escuro. O símbolo deve sugerir guardar, organizar ou retomar conhecimento sem ilustrar literalmente uma “biblioteca” genérica.
- A logo não deve competir com capas, títulos ou a ação de continuar estudando; ela é infraestrutura de orientação, não o elemento de destaque da Biblioteca.
