# Índice da documentação

**Revisão:** 2026-10-24 · **Estado:** rascunho para discussão

## Resumo (TL;DR)

Esta pasta guarda a análise e o plano da Petunia Engine, a **retomada** da Petunia Game Engine 3D com escopo ampliado. Cada documento começa com um resumo curto. Ao final de cada um há "decisões ligadas", que apontam para o [registro de decisões](06-decisoes/registro-de-decisoes.md).

## Legenda de confiança

Todo fato importante recebe um rótulo. Assim você sabe o que foi conferido e o que é palpite.

| Rótulo | Significa |
|---|---|
| `LOCAL` | Li o código ou o documento nos repositórios, nesta sessão. |
| `WEB` | Li numa fonte da internet nesta sessão (link na seção de fontes). |
| `DECLARADO` | Está num README ou doc, mas **não** reproduzi nem testei. |
| `GERAL` | Conhecimento geral meu, não conferido agora. Confirmar antes de depender. |
| `HIPÓTESE` | Inferência minha. Precisa de experimento (spike). |

Regra: **`DECLARADO`, `GERAL` e `HIPÓTESE` nunca viram fato sem prova.**

## Documentos

### 00 — Visão
- [01 Resumo executivo](00-visao/01-resumo-executivo.md)
- [02 Princípios](00-visao/02-principios.md)
- [03 Proposta da engine](00-visao/03-proposta.md) — a visão em uma página
- [04 **Descrição completa da engine**](00-visao/04-descricao-completa.md) — a referência única, de ponta a ponta
- [05 Filosofia de engenharia (do Digi-Tactics)](00-visao/05-filosofia-de-engenharia.md)

### 01 — Pesquisa
- [01 O que aproveitar do Petunia3D](01-pesquisa/01-aproveitamento-petunia3d.md)
- [02 O que aproveitar da Aipo](01-pesquisa/02-aproveitamento-aipo-lang.md)
- [03 Referências externas (Godot, Unity, GameGuru MAX, FPS Creator…)](01-pesquisa/03-referencias-externas.md)
- [04 Licenças](01-pesquisa/04-licencas.md)
- [05 Especificação anterior (Petunia Game Engine 3D)](01-pesquisa/05-especificacao-anterior.md) — **leia cedo**: já decidiu várias coisas
- [06 O que copiar do Petunia3D (inclui a GUI)](01-pesquisa/06-copia-do-petunia3d.md)
- [07 **Pintura e animação: vale trazer?**](01-pesquisa/07-pintura-e-animacao.md)

### 02 — Arquitetura
- [01 Visão geral](02-arquitetura/01-visao-geral.md)
- [02 Mundo e dados](02-arquitetura/02-mundo-e-dados.md)
- [03 Renderização e pós-processamento](02-arquitetura/03-renderizacao-e-pos.md)
- [04 Terreno, rios e estradas](02-arquitetura/04-terreno-rios-estradas.md)
- [05 Vegetação, clima, decals e billboards 3D](02-arquitetura/05-vegetacao-clima-decals.md)
- [06 Física leve e cordas](02-arquitetura/06-fisica-e-cordas.md)
- [07 Online](02-arquitetura/07-online.md)
- [08 Editor e construção de mapas](02-arquitetura/08-editor-e-construcao-de-mapas.md)
- [11 **Editor (fase 1): sessão, operações e interface Qt**](02-arquitetura/11-editor.md) — o que existe hoje
- [12 **Paridade da interface com o Petunia3D**](02-arquitetura/12-paridade-ui-petunia3d.md) — o que veio e o que falta
- [13 **Primitivas e edição de malha**](02-arquitetura/13-primitivas-e-edicao-de-malha.md) — três níveis para construir mapas
- [14 **Workspaces e prioridades antes da API**](02-arquitetura/14-workspaces-e-prioridades.md) — Mapa, Pintura, Animação, Efeitos; cavernas e andares
- [09 Personagens, UI de jogo e grade](02-arquitetura/09-personagens-ui-e-grade.md)
- [10 Bibliotecas auxiliares (comprar × construir)](02-arquitetura/10-bibliotecas-auxiliares.md)

### 03 — Linguagens e API
- [01 Estratégia de linguagens (Aipo + Luau)](03-linguagens-e-api/01-estrategia-de-linguagens.md)
- [02 Princípios da API](03-linguagens-e-api/02-principios-da-api.md)
- [03 Esboço da API (ilustrativo)](03-linguagens-e-api/03-esboco-da-api.md)
- [05 **Script Host (Aipo) e editor de código**](03-linguagens-e-api/05-script-host.md): implementado, com o que falta
- [04 Programação visual e nós 1:1 com a API (Gameplay IR)](03-linguagens-e-api/04-programacao-em-nos.md)

### 04 — Acessibilidade
- [01 Editor acessível](04-acessibilidade/01-editor.md)
- [02 Jogos acessíveis](04-acessibilidade/02-jogos-criados.md)

### 05 — Roadmap
- [01 Fases e experimentos](05-roadmap/01-fases-e-experimentos.md)
- [02 Riscos e fora de escopo](05-roadmap/02-riscos-e-fora-de-escopo.md)
- [03 **Estado da implementação**](05-roadmap/03-estado-da-implementacao.md) — o que existe e o que foi verificado

### 07 — Prova de conceito
- [01 Remake do Digi-Tactics](07-prova-de-conceito/01-digi-tactics.md)

### 06 — Decisões
- [Registro de decisões](06-decisoes/registro-de-decisoes.md)
- [Filosofia própria × derivar do Godot](06-decisoes/filosofia-propria-vs-godot.md)
- [Linguagem do núcleo: comparativo de 7 linguagens](06-decisoes/linguagem-do-nucleo-comparativo.md)
- [Portar a matemática para Odin](06-decisoes/odin-vs-rust.md)

### Apoio
- [Glossário](glossario.md)
- [Registro de dependências](dependencies.md)
