# Editor acessível

**Revisão:** 2026-10-24 · **Estado:** proposta; depende de `DR-04` e do experimento S8

## Resumo (TL;DR)

Acessibilidade do editor significa que **pessoas com deficiência ou neurodivergência conseguem criar mundos**, não só jogar.

Três pilares: **(1)** tudo por teclado, **(2)** leitor de tela e contraste em todo lugar, **(3)** carga cognitiva baixa e controle sobre estímulos. Cada pilar vira **teste automatizado** (gate), não promessa.

## Ponto de partida (herdado)

O Petunia3D já adota, como regra de engenharia (`LOCAL`, `AGENTS.md` §1 e §5):

- Navegação por teclado com `F6`/`Tab` e foco restaurável.
- Alto contraste, redução de movimento, rótulos claros e alternativas sem ponteiro.
- **Camada 6 da pirâmide de testes: acessibilidade como gate** (`QAccessible`, teclado sem armadilhas, contraste).
- Alvos de clique de **24 px** (WCAG 2.2, critério 2.5.8) nas alças de decal.
- Atalhos em **perfis configuráveis** e i18n en/pt-BR em paridade.

A engine herda tudo isso como **critério de aceite de cada tela**.

## Pilar 1 — Teclado completo

| Regra | Detalhe |
|---|---|
| Todo comando alcançável por **paleta de comandos** | O Petunia3D já tem catálogo canônico de comandos (`LOCAL`) |
| Ordem de foco previsível; sem armadilha | Testado automaticamente |
| **Colocar objeto sem mouse** | "Colocar no cursor/câmera", setas movem por passo, campo numérico |
| **Câmera 3D por teclado/controle** | Passos, órbita, "enquadrar seleção" |
| Esculpir/pintar sem arrastar | Pincel "passo": aplicar N vezes com valor numérico |
| Atalhos remapeáveis, perfis salvos | Já existe no Petunia3D (`keymaps`) |
| **Sem exigir gesto complexo** | Nada de pressionar duas teclas e arrastar ao mesmo tempo como única via |
| **Sem tempo limite** | Nenhuma ação depende de rapidez |

## Pilar 2 — Leitor de tela, contraste e escala

| Regra | Detalhe |
|---|---|
| **Árvore de acessibilidade** em todo controle | Nome, papel, estado, valor |
| **Estrutura da cena narrada** | O painel "Estrutura" é uma árvore lida pelo leitor: "Rio 1, largura 8 metros, passa a 3 metros da Estrada A" |
| Foco nunca perdido | Depois de uma ação, o foco volta a um lugar lógico |
| Temas: **claro, escuro, alto contraste** | Tokens semânticos (já existem, `LOCAL`) |
| Escala de interface | De 100% a 300% sem cortar conteúdo |
| **Reduzir movimento** | Desliga animações da interface; segue a configuração do sistema |
| Cor nunca é o único sinal | Ícone ou texto acompanha |

**Viewport 3D e leitor de tela.** O que está **dentro** da cena não tem árvore nativa. Soluções, em ordem de esforço:

1. **Painel de estrutura narrado** (acima).
2. **Descrição por comando:** "Descrever seleção" e "O que está perto?" (em texto e voz).
3. **Consulta em linguagem natural** via MCP (veja abaixo). É extra, não substitui 1 e 2.

Quanto ao toolkit: o AccessKit já está integrado em egui, Slint, Bevy e outros (`WEB`); o Qt tem a própria pilha (`QAccessible`, `LOCAL` no gate do Petunia3D). O Godot 4.5 mostra que um **editor** acessível é trabalho grande e incremental: em 2025 só cobria gerenciador de projetos, controles padrão e inspetor (`WEB`). **Meta realista:** começar pelo **inspetor, estrutura, paleta de comandos, editor de código e Project Doctor**, e depois a barra de ferramentas.

## Pilar 3 — Carga cognitiva e conforto

| Recurso | O que faz |
|---|---|
| **Modo simples por padrão** | Poucas opções visíveis; "Mostrar mais" revela o resto |
| **Modo foco** | Escurece tudo exceto o painel em uso |
| **Layout fixo** | Mesma geografia em todos os modos (princípio herdado) |
| **Mensagens em linguagem simples** | O erro diz o que fazer |
| Sem pop-ups surpresa | Avisos entram na gaveta, não no meio da tela |
| **Calma para mudanças** | Efeitos ao vivo (preview de clima, partículas) com opção de pausa |
| **Desfazer sempre** | Nenhuma ação destrutiva sem desfazer |
| **Autosave visível** | "Salvo agora" discreto; sem diálogos de confirmação em excesso |
| **Resumo ao reabrir** | "Você estava em: modo Mundo, rio 2. Última ação: …" (ajuda a retomar contexto) |
| Tutoriais **interativos e curtos** | Um passo por vez, pode pausar e voltar |

## Editor de código acessível

| Recurso | Para quem |
|---|---|
| **Fonte configurável** (incluindo fonte para dislexia), espaçamento de linha e letra | Dislexia |
| **Régua de foco** (destaca a linha atual) | TDAH, dislexia |
| **Destaque que não depende só de cor** (negrito/sublinhado/ícone) | Daltonismo |
| **Modo calmo**: erros aparecem **ao pausar** de digitar, sem sublinhado ondulante o tempo todo | Sobrecarga sensorial |
| **Autocomplete opcional** e discreto | Controle |
| Leitura de erros por **leitor de tela** com a correção sugerida | Visão |
| **Recolher blocos** e painel de **estrutura do arquivo** | Orientação |
| Mensagens do compilador **em português** | Idioma |
| **Formatação automática** | Uma forma só; menos decisões |
| Navegação por **blocos** (próxima função, próximo erro) | Teclado |

A Aipo já nasceu com este objetivo: "projetada para leitura, escrita e manutenção por pessoas neurodivergentes (TDAH, dislexia, sobrecarga cognitiva)" (`LOCAL`, `SYNTAX.md`).

## Automação como acessibilidade (MCP)

O servidor MCP do Petunia3D passa tudo pelo **mesmo despachante de comandos**, com política que nega por padrão, auditoria e desfazer (`LOCAL`, `crates/mcp`). Isso permite que um assistente (voz ou texto) **execute** pedidos do tipo "cria um rio de A até B" **sem contornar o desfazer**. É um caminho de acesso valioso para pessoas que não usam mouse bem. Condição: **sempre confirmar e sempre desfazer**.

## Como verificamos (gates)

| Camada | Teste |
|---|---|
| Unidade | Todo comando tem rótulo, atalho padrão e descrição |
| Interface | Ordem de foco; nenhuma armadilha de teclado |
| Acessibilidade | Árvore de acessibilidade completa por tela (nome, papel, valor) |
| Visual | Contraste mínimo por tema (WCAG 2.2 AA) |
| **Humano** | Teste periódico com pessoas reais usando leitor de tela e só teclado |

Sem teste humano, **não declaramos "acessível"**. Dizemos "passa nos gates" (`HIPÓTESE` até haver teste).

## Decisões ligadas

`DR-04` UI do editor · `DR-15` gates.
