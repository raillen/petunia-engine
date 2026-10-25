# Petunia Engine

> **Nome provisório.** Engine 3D para quem constrói mundos, não só para quem programa.
> **Estado:** fase 1 (esqueleto) em implementação. Veja [o que existe e o que foi verificado](docs/05-roadmap/03-estado-da-implementacao.md).
> **Última revisão:** 2026-10-24

## Resumo (TL;DR)

Petunia Engine é uma engine 3D em Rust, pensada para:

- **Acessibilidade de verdade**, no editor e nos jogos criados.
- **Criar mapas rápido:** terreno, rios, estradas ligadas, vegetação, clima, decals e billboards 3D.
- **Programar com dados, não com classes.** Linguagem padrão: **Aipo**. Alternativa: **Luau** (uma ativa por projeto).
- **Uma API só**, descrita como dado. Dela saem: Aipo, Luau, Event Sheet e nós (1:1 com a API), autocomplete e documentação.
- **Online fácil:** "criar sala, passar código, jogar".

## Experimente (fase 1)

Para abrir o **editor** (precisa de Qt 6, CMake e Ninja; veja [dependências](docs/dependencies.md)):

```bash
scripts/editor.sh
```

Dentro dele: **Arquivo > Novo projeto de exemplo** cria uma sala para explorar. Controles em [editor](docs/02-arquitetura/11-editor.md).

Sem o editor, pela linha de comando:

```bash
export CARGO_TARGET_DIR=/tmp/petunia-engine-target CARGO_BUILD_JOBS=2
cargo run -p petunia_cli -- new /tmp/minha_sala      # cria um projeto com uma sala de exemplo
cargo run -p petunia_cli -- check /tmp/minha_sala    # valida o projeto, sem janela
cargo run -p petunia_cli -- run /tmp/minha_sala      # abre a janela com câmera livre
cargo run -p petunia_cli -- schema                   # lista a API da engine
```

## Como ler esta documentação

Comece por aqui, nesta ordem:

1. [Descrição completa da engine](docs/00-visao/04-descricao-completa.md) — a referência única, de ponta a ponta.
2. [Proposta](docs/00-visao/03-proposta.md) — a visão em uma página.
3. [Resumo executivo](docs/00-visao/01-resumo-executivo.md) — o que é e por quê.
4. [Especificação anterior](docs/01-pesquisa/05-especificacao-anterior.md) — o que já estava decidido e o que mudou.
5. [Filosofia de engenharia](docs/00-visao/05-filosofia-de-engenharia.md) — o que adotamos do `AGENTS.md` do Digi-Tactics.
6. [Bibliotecas auxiliares](docs/02-arquitetura/10-bibliotecas-auxiliares.md) — o que comprar e o que construir.
7. [Filosofia própria × Godot](docs/06-decisoes/filosofia-propria-vs-godot.md), [comparativo de linguagens](docs/06-decisoes/linguagem-do-nucleo-comparativo.md) e [Odin × Rust](docs/06-decisoes/odin-vs-rust.md) — análises de decisão.
8. [Prova de conceito: Digi-Tactics](docs/07-prova-de-conceito/01-digi-tactics.md) — o remake que prova a engine.
9. [Princípios](docs/00-visao/02-principios.md) — as regras que decidem os empates.
10. [Registro de decisões](docs/06-decisoes/registro-de-decisoes.md) — o que **você** precisa decidir.

O índice completo está em [docs/README.md](docs/README.md).

## Mapa das pastas

| Pasta | O que tem |
|---|---|
| `docs/00-visao` | Visão, público, princípios |
| `docs/01-pesquisa` | O que aproveitar do Petunia3D e da Aipo; referências externas; licenças |
| `docs/02-arquitetura` | Como a engine é organizada, por sistema |
| `docs/03-linguagens-e-api` | Aipo, Luau, desenho da API, programação em nós |
| `docs/04-acessibilidade` | Editor acessível e jogos acessíveis |
| `docs/05-roadmap` | Fases, experimentos (spikes), riscos, fora de escopo |
| `docs/06-decisoes` | Decisões abertas, com opções e recomendação |
| `docs/07-prova-de-conceito` | Remake do Digi-Tactics como prova de uso |

## Projetos de origem (somente leitura)

- `antigo-projeto[apenas-para-consulta-e-analise].md` (nesta pasta) — especificação da **Petunia Game Engine 3D** (`WASDst/petunia-game-engine`). Resumida e reconciliada em `docs/01-pesquisa/05-especificacao-anterior.md`.
- `~/Documentos/Projetos/petunia3D` — modelador low-poly em Rust (GPL-3.0).
- `~/Documentos/Projetos/aipo-lang` — a linguagem Aipo, em Rust (MIT).
- `~/Documentos/digimon-stranger-story-animated-models` — o **Digi-Tactics** (auto-battler em TypeScript), alvo do remake de prova.
