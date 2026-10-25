# Proposta da Petunia Engine

**Revisão:** 2026-10-24 · **Estado:** proposta para sua aprovação. Reúne tudo o que foi analisado.

## Resumo (TL;DR)

A Petunia Engine é a **engine de comandos**: tudo o que existe nela (criar um rio, comprar uma carta, cortar uma corda, abrir uma porta) é **um comando com schema**. O mesmo comando serve ao editor, aos scripts Aipo e Luau, às telas visuais, aos testes, aos agentes de IA e à rede.

Ela é provada por **dois projetos**: o **Digi-Tactics** (jogabilidade, animação, UI, rede) e a **Ilha** (rios, estradas, clima, cordas).

## A tese em uma frase

> **Menos conceitos, um caminho só.** Quem cria o jogo descreve **dados e regras**. A engine cuida do resto, com acessibilidade ligada por padrão.

## As 6 apostas

| # | Aposta | Por que |
|---|---|---|
| 1 | **API como dado** (um schema → Aipo, Luau, telas visuais, docs, MCP, rede) | O Digi-Tactics, o Petunia3D e a Aipo já fazem isso em pequeno |
| 2 | **Dados em vez de classes** (tabelas + regras + funções) | Facilita ler, testar e replicar; combina com a Aipo |
| 3 | **Aipo primeiro**, Luau como alternativa por projeto | Você quer a Aipo de primeira classe; a decisão anterior já limita a uma por projeto |
| 4 | **Programação visual 1:1 com a API** (Event Sheet + Gameplay IR) | Evita o erro do VisualScript do Godot |
| 5 | **Acessibilidade verificada** no editor e nos jogos (Project Doctor) | Diferencial real; hoje os engines dão o mínimo |
| 6 | **Mundo natural fácil** (terreno, rios, estradas ligadas, clima, vegetação) | O "primeiro minuto" gera algo bonito |

## O que muda em relação à especificação anterior

| Mantido (decidido antes) | Acrescentado |
|---|---|
| Rust, Qt/CXX-Qt, OpenGL leve, GLB, `petunia-libs`, RuntimeWorld, Gameplay IR | Mundo natural, cordas, online pequeno, pós leve |
| Luau **ou** Aipo, uma por projeto | Aipo como padrão |
| Level Builder com brushes | Ferramentas de rio, estrada, vegetação |
| Project Doctor / Issue Browser | Verificações de acessibilidade |
| | **Animação esquelética (reprodução), toon, UI de jogo, grade** (vindos do Digi-Tactics) |

## Como as duas provas se dividem

| Parte da engine | Digi-Tactics | Ilha |
|---|:--:|:--:|
| Núcleo, comandos, schema | ✔ | ✔ |
| Aipo em tempo fixo, determinismo | ✔ | |
| Animação esquelética + toon | ✔ | |
| UI de jogo, textos en/pt-BR | ✔ | |
| Módulo de grade, A\* | ✔ | |
| Terreno, rios, estradas | | ✔ |
| Vegetação, clima, decals | | ✔ |
| Física leve, cordas | | ✔ |
| Online | ✔ (8 jogadores) | ✔ (coop) |
| Project Doctor, acessibilidade | ✔ | ✔ |

## Ordem proposta (9 fases)

```text
 F1 Esqueleto     F2 Editor+brushes    F3 Regras+Script Host  ◄── porta o núcleo do Digi-Tactics (sem gráficos)
 F4 Personagens, UI, grade ──► PROVA 1: Digi-Tactics offline
 F5 Mundo natural ──────────► PROVA 2: a Ilha
 F6 Física, cordas, IA   F7 Online   F8 Intercâmbio, MCP, export   F9 Acabamento
```

Por que esta ordem:

- **F3 já testa a Aipo com um jogo real**, antes de qualquer gráfico. Se a Aipo falhar, descobrimos cedo.
- **F4 vem antes do mundo natural** porque personagens e UI são necessários em quase todo jogo.
- **F5** entrega o diferencial visual. Pode ser antecipada em parte (só o terreno) se você quiser (`DR-14`).

## Regras de proteção do escopo

1. Cada fase termina com uma **demo** que roda.
2. Cada pedido novo passa pela pergunta: **"é um comando do schema?"** Se não for, reescrevemos o pedido.
3. **Fatia mínima** para cada prova (já definida para o Digi-Tactics).
4. **Experimentos antes de decisões**: S1 a S9.
5. **Sem número sem medição** (FPS, memória, orçamentos).

## O que eu **não** proponho

- Um renderer sofisticado, ray tracing, VR, consoles.
- Reescrever o Petunia3D dentro da engine. Ele continua o modelador.
- Copiar o shader ou os modelos do jogo Digimon para o repositório público.
- Prometer *rollback netcode* ou determinismo entre plataformas na v1.
- Marketplace, contas e nuvem.

## Meu veredito honesto

| Pergunta | Resposta |
|---|---|
| É viável? | **Sim**, se o escopo for protegido. O Digi-Tactics e o Petunia3D mostram que o estilo funciona. |
| É rápido? | **Não.** É um projeto de longo prazo. Por isso as fases têm demo e os experimentos vêm antes. |
| Maior risco técnico? | A **Aipo em tempo real** (e o custo do Qt na sua máquina). Ambos têm experimento (S1, S4). |
| Maior risco de projeto? | **Escopo.** Rios, estradas, cordas, clima, online e UI de jogo juntos são muita coisa. |
| Maior oportunidade? | O **Project Doctor de acessibilidade** e a **API única**: poucas engines têm. |

## Decisões ligadas

`DR-14` ordem e escopo · `DR-16` prova de conceito · `DR-02` objetivo.
