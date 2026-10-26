# Programação visual e nós 1:1 com a API

**Revisão:** 2026-10-24 (reescrita após ler a [especificação anterior](../01-pesquisa/05-especificacao-anterior.md)) · **Estado:** proposta; precisa do experimento S5

> **Atualização (2026-10-26):** você decidiu por **sistema de nós no lugar do Event Sheet**. A seção "Nível 1 — Event Sheet" abaixo vira a **vista em contorno** do grafo, e a "Vista de nós" deixa de ser opcional e passa a ser o padrão. O desenho novo está em [DR-30](../06-decisoes/registro-de-decisoes.md). O restante (IR 1:1 com a API, "Ver como código", limites, nó-fantasma) continua valendo.

## Resumo (TL;DR)

**1:1 com a API** significa: **cada função da API é exatamente um nó**, com os mesmos parâmetros, tipos, faixas e erros. Nada existe só no visual, nada existe só no texto.

Para isso há uma camada no meio: o **Gameplay IR**, uma representação **tipada e neutra de linguagem** das regras do jogo. As telas visuais (Event Sheet, grafos, nós) e os scripts (Aipo ou Luau) **usam a mesma API**.

Esta é a decisão já tomada na especificação anterior: *"a UI visual não deve ser um espelho da AST da Aipo/Luau"*. Ela é a lição do VisualScript do Godot em prática.

## Por que não espelhar o texto

O Godot removeu o VisualScript porque ele era um espelho 1:1 do texto, sem blocos de alto nível (`WEB`). Quem queria nós acabou preferindo o código.

| Abordagem | Risco |
|---|---|
| Nós = **espelho da sintaxe** (cada `if`, cada `let` vira nó) | Vira "texto com macarrão". Sem valor extra. Frágil se a sintaxe muda |
| Nós = **API + regras de alto nível**, via IR | O valor está nos verbos grandes (`rope.create`, `weather.set`, `rules.on`) |

A segunda é a escolhida. O texto continua sendo uma opção igual, não obrigatória.

## Camadas

```text
        Schema da API (único)
               │
   ┌───────────┴───────────┐
   ▼                       ▼
Gameplay IR           Script textual
(regras, estados)     (Aipo ou Luau)
   │                       │
   ├─ Event Sheet          └─ chama a MESMA API
   ├─ Grafo de estados
   ├─ Árvore de comportamento
   └─ Vista de nós (avançada)
```

| Peça | O que é |
|---|---|
| **Schema da API** | Lista única de funções, parâmetros, tipos, erros e permissões |
| **Gameplay IR** | Regras e comportamentos como **dados tipados**, com ids estáveis |
| **Telas visuais** | **Vistas** do mesmo IR. Editar uma muda o IR |
| **Script** | Código escrito à mão, para o que o IR não expressa. Chama a mesma API |

## Três níveis de autoria visual

A especificação anterior decidiu: **sem um grafo genérico obrigatório para tudo** (`LOCAL`, seção 8.3). Cada tarefa tem a forma mais natural.

### Nível 1 — Event Sheet (o padrão)

Uma lista de regras **QUANDO / SE / FAÇA**. Cresce de cima para baixo. Foi pensada para quem nunca programou, e funciona bem com teclado e leitor de tela (é uma lista).

```text
QUANDO  tocar em  [tag: Door]
  SE    [inventory.has]  quem: jogador · item: "chave_azul"
  FAÇA  [world.set]      alvo: porta · open: verdadeiro
        [sound.play]     som: "porta_abre" · em: porta
  SENÃO
  FAÇA  [ui.say]         texto: "Precisa da chave azul."
```

Cada linha de ação é **uma chamada da API**. Referência de modelo mental: o FPS Creator usava `:condição:ação` (`WEB`), mas com pontuação crua. Aqui são palavras.

### Nível 2 — Grafos especializados

Só onde um grafo ajuda de verdade:

| Grafo | Para quê |
|---|---|
| **Máquina de estados** | Inimigo: Patrulhar → Perseguir → Atacar |
| **Árvore de comportamento** | IA mais rica (opcional, depois) |

### Nível 3 — Vista de nós (avançada)

Fios de **dados** entre nós, para quem quer ver fluxo de valores. É **opcional** e fica no modo avançado. O **fluxo de execução continua empilhado** (de cima para baixo dentro de cada quadro); só dados usam fios. Isso evita o espaguete.

## Alternativa acessível: vista em contorno

Para cada tela visual existe uma **vista em contorno/tabela** (lista hierárquica navegável por teclado e leitor de tela). A especificação anterior já pedia isso para grafos de comportamento (`LOCAL`, seção 24.1). É requisito de acessibilidade, não enfeite.

## Como a Aipo e o Luau entram

| Direção | Comportamento |
|---|---|
| **IR → texto** | Sempre possível. Botão **"Ver como código"** gera Aipo ou Luau legível, pelo **formatador canônico** (a Aipo tem uma forma só) |
| **Texto → IR** | **Não** é exigido para código livre |
| **Editar o código gerado** | O bloco **se separa** do visual e vira script (aviso claro). O inverso não é automático |

A linguagem ativa é **uma por projeto** (decidido; padrão Aipo). Como o IR é neutro, **as telas visuais funcionam nas duas**. Isso corrige a minha primeira proposta, que limitava os nós à Aipo.

### Estudo opcional: ida e volta completa (S5b)

Se um dia você quiser **qualquer** código Aipo virando nós e voltando, é possível, porque a Aipo tem formatador e uma só forma de escrever (`LOCAL`). Teste de aceite:

```text
 formatar(texto)  ==  formatar( nós_para_texto( texto_para_nós( texto ) ) )
```

Rodaria sobre o corpo de conformidade da Aipo (`docs/conformance/programs`, `LOCAL`). **Custo alto e valor incerto** pela lição do Godot. Fica como pesquisa depois da v1.

## O que o schema dá de graça

- **Catálogo de nós/ações** (nome, categoria, cor, pinos, ordem).
- **Controles do inspetor** a partir dos parâmetros. O Petunia3D já faz: `RecipeParamSpec` (`Float{min,max}`, `Int`, `Bool`, `Color`, `Choice`, `LOCAL`).
- **Documentação** idêntica à do texto.
- **Validação de tipo** nos pinos: só conecta o que combina.
- **Permissões:** o nó mostra qual capability exige.

## O que o IR precisa ter (esboço do contrato)

| Campo | Para quê |
|---|---|
| **Id estável** por regra, ação e condição | Referência em saves, diff, depuração, MCP |
| **Tipos** nos valores | Validar antes de rodar |
| **Versão do schema** | Migração explícita |
| **Origem** (tela, script, agente) | Rastreabilidade |
| **Texto de exibição** | O rótulo que a pessoa lê |
| **Formato textual e diferenciável** | Amigável ao Git (decidido) |

Ponto aberto da especificação anterior: *"formato físico do Gameplay IR e como preservar referências estáveis"* (`LOCAL`, seção 29, item 8). Fica em `DR-07`.

## Padrões reaproveitáveis do Petunia3D

O `op_graph` mostra um bom desenho de dados para editores de nós (`LOCAL`):

| Ideia | Uso |
|---|---|
| Descritor estático por tipo de nó (portas tipadas, parâmetros, versão) | Gerado do schema |
| Interface gerada do descritor | Mesmo princípio |
| Nó desconhecido preservado como **fantasma** | Uma ação removida não apaga a regra; aparece com aviso |
| Limites duros (512 nós, 2048 arestas, grupos) | Arquivos são entrada hostil |
| Toda edição é **comando** com desfazer | Edição visual entra no mesmo histórico |

## Depuração

| Recurso | Fonte |
|---|---|
| Regra executando destacada | Mapa do IR para a execução |
| "Por que esta condição falhou?" | Proposta da especificação anterior (seção 19.3) |
| Pontos de parada e passo a passo em regras | Idem |
| Histórico de execução com tempo por regra | Idem |

Para script livre, o depurador é uma **lacuna** da Aipo (veja [aproveitamento da Aipo](../01-pesquisa/02-aproveitamento-aipo-lang.md)).

## Riscos

| Risco | Tratamento |
|---|---|
| IR vira "mais uma linguagem" | Poucos conceitos: regra, condição, ação, estado, variável. Tudo mais é **chamada da API** |
| Duas formas de fazer o mesmo (IR e script) | Aceito: o IR cobre o comum; o script, o resto. Mesma API |
| Layout dos nós desalinhado | Layout guardado **em arquivo ao lado**, com re-ancoragem; automático como alternativa |
| Quem prefere texto estranha | O texto é sempre uma opção equivalente |
| Programa visual enorme | Quadros recolhíveis, busca, vista em contorno |

## Decisões ligadas

`DR-07` (IR e telas) · `DR-06` (uma linguagem por projeto).
