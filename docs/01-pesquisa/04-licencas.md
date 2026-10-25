# Licenças

**Revisão:** 2026-10-24 · **Estado:** análise técnica, **não é aconselhamento jurídico**

> **Atualização (2026-10-24):** você confirmou que o Petunia3D e a Aipo são **projetos seus**. O reuso de **código** entre eles está liberado (`DR-01` decidida). Continua aberta só a **licença pública do runtime**. Ativos de terceiros (Digimon) seguem outra regra: veja [prova de conceito](../07-prova-de-conceito/01-digi-tactics.md).

## Resumo (TL;DR)

**Problema:** o Petunia3D é **GPL-3.0-or-later**. A Aipo é **MIT**. Se o código GPL entrar no **runtime** da engine, os jogos distribuídos com esse runtime podem precisar ser GPL também.

**Recomendação:** separar em três partes, cada uma com sua licença (tabela abaixo). Se você é a única pessoa com os direitos autorais do Petunia3D, pode relicenciar o que quiser reaproveitar. **Preciso que você confirme isso.**

## Fatos conferidos

| Fato | Fonte |
|---|---|
| Petunia3D: `GPL-3.0-or-later` | `petunia3D/Cargo.toml`, `LICENSE` (`LOCAL`) |
| Aipo: `MIT` | `aipo-lang/README.md`, `LICENSE` (`LOCAL`) |
| Petunia3D usa Qt 6 via `cxx-qt` | `crates/ui-cxxqt/Cargo.toml` (`LOCAL`) |
| Dependências do Petunia3D: `mlua`, `rmcp`, `manifold-rust`, `xatlas-rs-v2`, `geo`, `zip`… | `Cargo.toml` raiz (`LOCAL`) |
| AccessKit: MIT/Apache-2.0 | accesskit.dev (`WEB`) |
| Luau é código aberto (MIT) | `GERAL` — confirmar |
| Rapier (`rapier3d` 0.36.1): Apache-2.0 | crates.io, 2026-10-24 (`WEB`) |
| `kira`, `renet`, `winit`, `cosmic-text`, `glam`, `mlua` (MIT) e demais: MIT e/ou Apache-2.0 | crates.io (veja [bibliotecas](../02-arquitetura/10-bibliotecas-auxiliares.md)) |
| `symphonia` (decodificação do Kira): **MPL-2.0**, fora da lista permitida do `deny.toml` | crates.io; abrir exceção registrada se entrar |
| Qt: LGPLv3 / GPLv3 / comercial | `GERAL` — confirmar |

## Por que a GPL importa aqui

Uma engine é **linkada** dentro de cada jogo. Se a parte linkada é GPL, o jogo inteiro tende a cair sob a GPL. Para quem cria jogos comerciais ou fechados, isso é um bloqueio.

O editor é diferente: ele é um programa separado, que **não** vai dentro do jogo. O editor pode ser GPL sem afetar o jogo criado.

## Proposta de separação

| Parte | Licença sugerida | Motivo |
|---|---|---|
| **Runtime** (núcleo, renderer, física, rede, host de scripts) | MIT ou Apache-2.0 (ou GPL + exceção de runtime) | Jogos criados ficam livres |
| **Linguagem Aipo** | MIT (como já é) | Sem mudança |
| **Editor** | GPL-3.0-or-later (como o Petunia3D) ou a mesma do runtime | Pode herdar código do Petunia3D sem relicenciar |
| **Assets de exemplo** | CC0 ou CC-BY, com `ATTRIBUTION.md` | Rastreável |

## Consequência para o reaproveitamento

| Módulo do Petunia3D | Vai para… | Precisa de relicenciamento? |
|---|---|---|
| `spline`, `path_generator`, `surface_attachment`, `mesh/sweep`, `triangulate` | Runtime (geração de estradas e rios) | **Sim**, se o runtime for MIT |
| `commands`, `op_graph`, `prefab`, `game_ready` (padrões) | Editor | Não, se o editor for GPL |
| `plugins`, `mcp` | Editor e/ou runtime | Depende de onde rodam |

Alternativa sem relicenciar: **reescrever** os poucos módulos do runtime a partir do desenho (clean-room interno). Mais lento, mas sem questão jurídica.

A especificação anterior já decidiu que o reuso passa por **`petunia-libs`**, um repositório de crates pequenas e versionadas (`LOCAL`). Isso ajuda: **a licença se decide por crate**. Geometria, spline e varredura podem ser MIT/Apache em `petunia-libs`; o editor continua GPL.

## O que verificar antes de decidir

1. Você é a única pessoa com direitos autorais do Petunia3D? O repositório remoto é `WASDst/petunia3d`. Há outras contribuições?
2. Algum código do Petunia3D vem de terceiros (com a GPL como condição)? O `ATTRIBUTION.md` só lista imagens de diagnóstico.
3. Rodar `cargo deny` (já há `deny.toml` nos dois projetos) para listar a licença de cada dependência.
4. Decidir como o Qt será distribuído (ligação dinâmica LGPL ou outra opção), caso o editor use Qt.

## Decisões ligadas

`DR-01` licença · `DR-04` UI do editor (Qt tem implicações de licença).
