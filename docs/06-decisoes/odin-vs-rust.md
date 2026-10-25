# Portar a matemática do Petunia3D para Odin

> **Veja também o [comparativo completo](linguagem-do-nucleo-comparativo.md)** (Rust, Odin, Go, C#, TypeScript, C++ e V, com a integração de Luau e Aipo). Este documento é o detalhe do **porte para Odin**.

**Revisão:** 2026-10-24 · **Pergunta:** qual a dificuldade de portar a lógica matemática e as dependências do Petunia3D para uma linguagem mais leve e fácil de compilar, como o Odin? · Fontes: medições nos seus repositórios (`LOCAL`) e pesquisa (`WEB`)

## Resumo (TL;DR)

**Dificuldade: alta em volume, média em técnica.** A matemática do Petunia3D é código Rust simples e portável. Mas são **cerca de 41 mil linhas**, mais os testes, e as dependências que complicam são poucas.

**Dois achados mudam a conversa:**
1. **A matemática não é o que pesa na compilação.** O workspace tem **379 pacotes** (`Cargo.lock`). O peso vem de Qt/C++, `mlua`, `rmcp`/`tokio`, `resvg` e (no Aipo) `wasmtime`. A matemática usa `glam` e estruturas simples.
2. **Você já percorreu esse caminho.** A baseline da Aipo era **Odin** e passou a **Rust-first em 2026-09-15** (`LOCAL`, canon da Aipo: "as seções históricas em Odin … não definem mais a baseline").

**Recomendação:** **não portar agora.** Reduzir o custo de compilação em Rust e, se ainda quiser, rodar um **experimento curto (S10)** com uma fatia pequena antes de decidir.

## O que seria portado (medido)

| Bloco | Linhas | Observação |
|---|---:|---|
| `mesh` (half-edge, extrusão, bevel, loops, varredura, triangulação, UV) | ~24,0 mil | Código simples |
| `project`: spline, IK, rig, animação, caminhos, perfis, SVG, alinhamento | ~12,5 mil | |
| `core`: câmera, seleção, BVH, inferência, caneta poly, região | ~4,6 mil | |
| **Total de matemática/geometria** | **~41 mil** | |
| Testes nesses crates | **1.370** `#[test]` (mesh 323, project 445, core 602) | Nem todos são de matemática |

### Quão "portável" é esse Rust?

No crate `mesh` (`LOCAL`): **0 traits**, **0 genéricos `impl<…>`**, **0 lifetimes**, 35 enums, 82 `Result`, 90 usos de `serde`, **1.352 cadeias de iteradores**. É o estilo "estruturas e funções": estruturalmente fácil de levar. O trabalho mecânico é trocar **iteradores por laços** e `Result`/`?` por `or_return`.

## As dependências: onde dói

Só **três arquivos de produção** do código matemático carregam dependências pesadas (`LOCAL`):

| Dependência | Onde | Dificuldade em Odin |
|---|---|---|
| `manifold-rust` (booleanas 3D) | `mesh/boolean.rs` (781 linhas) | **Difícil.** Ligar ao Manifold em C++ ou portar. **O runtime da engine não precisa** (CSG fica no modelador) |
| `xatlas` (atlas de UV) | `mesh/uv_xatlas.rs` (314) | **Média a difícil.** Biblioteca C++; precisa de um adaptador em C. Só o modelador usa |
| `geo` | `mesh/profile_geo.rs` (112) | **Fácil.** Uso mínimo |
| `glam` | 167 de 425 arquivos `.rs` | **Fácil**, mas confira convenções (matrizes, quaternions, eixos) |
| `gltf-json` | 5 arquivos de `project` | **Fácil**: `vendor:cgltf` lê e escreve glTF |
| `tobj` (OBJ) | 1 arquivo | **Fácil** (escrever um leitor pequeno) |
| `resvg` (SVG) | 1 arquivo | **Difícil.** Sem equivalente em Odin; só o modelador usa |
| `zip` | 2 arquivos | **Média.** Escrever o contêiner (`HIPÓTESE`: a `core` do Odin tem `compress/zlib`) |
| `serde` / `schemars` | 90+ usos | **Média.** O JSON do Odin usa reflexão; o schema da engine já é dado próprio |
| `mlua`, `rmcp`/`tokio`, Aipo, Rapier | outros crates | **Ecossistema.** Ver abaixo |

**Leitura:** para o **runtime**, quase tudo difícil (booleanas, atlas, SVG) **não é necessário**. O que a engine precisa é matemática simples, glTF, imagens e zip.

## O que o Odin oferece (`WEB`)

| Ponto | Fato |
|---|---|
| Matemática | Vetores, matrizes e quaternions **embutidos**; pacote `core:math/linalg` |
| Bibliotecas `vendor:` | **raylib 6.0, SDL3, GLFW, OpenGL (até 4.6), wgpu, stb, `cgltf` (glTF leitura e escrita), miniaudio, ENet, box2d e box3d** |
| Física 3D | **Jolt** por binding da comunidade (`joltc-odin`, exige biblioteca compartilhada `libjoltc`) |
| Compilação | Um projeto de 14 mil linhas compilou em cerca de **230 ms** (benchmark de terceiros) e há trabalho contínuo de velocidade |
| Linguagem | Estável; quebras raras (reescrita do `core:os`, **gerador aleatório padrão mudou** e alterou sequências por semente) |
| Versões | `dev-YYYY-MM` mensais; sem 1.0; **sem gerenciador de pacotes oficial** nos resultados |

## O que o Rust nos dá que o Odin não dá (hoje)

| Rust | Em Odin |
|---|---|
| Rapier, Kira, renet, cosmic-text, `accesskit_winit`, `mlua`, `rmcp` | Sem equivalentes diretos. Seria ligar bibliotecas em C/C++ por *bindings* |
| `clippy -D warnings`, `cargo deny`, `cargo nextest`, `proptest`, `insta`, `criterion`, `cargo-fuzz` | Ferramentas de teste bem menores (`core:testing`, rastreador de alocações) |
| "Nunca travar" ajudado pelo compilador (`Result`, sem `null`) | Verificação de limites por padrão, mas estouro de inteiro dá volta em silêncio; disciplina manual |
| Aipo, Luau e MCP já em Rust | Aipo tem **ABI em C** (`aipo-c-abi`, ~4,1 mil linhas), então um host Odin é possível; o Aipo continua compilado por Cargo |
| Tarefas e testes por agentes de código guiados pelo compilador | Menos verificação estática para agentes |

Mas o Odin combina **melhor** com a sua filosofia de "dados e procedimentos, sem classes" (regras 8 e 9 do Digi-Tactics): não existem traits nem herança.

## Tabela de dificuldade

Tamanhos relativos: **P** pequeno · **M** médio · **G** grande · **GG** muito grande.

| Componente | Dificuldade | Tamanho |
|---|---|:--:|
| `glam` → `core:math/linalg` | Fácil (conferir convenções) | M |
| `mesh` (24 mil linhas, sem traits/genéricos) | Média (mecânica) | GG |
| spline, IK, rig, animação, caminhos (12,5 mil) | Média | G |
| câmera, seleção, BVH, inferência (4,6 mil) | Média | M |
| glTF por `vendor:cgltf`, OBJ, imagens por `stb` | Fácil | M |
| JSON, schema, `serde` | Média | M |
| zip | Média | P |
| Booleanas (`manifold`), atlas (`xatlas`), SVG (`resvg`) | **Difícil** | G |
| Luau/Aipo/MCP como host Odin | Média a difícil (ABI em C; Aipo ainda em Cargo) | G |
| Física: Rapier → Jolt por binding (`libjoltc` em C++) | Média; **traz C++ de volta ao build** | M |
| Testes: 1.370 `#[test]` → dados de referência | Média | G |
| **Editor** (`core` 63 mil, `editor` 40 mil, `project` 40 mil linhas) | **Reescrita** | GG |

## O que muda se portar só a matemática

- Fica um sistema **misto** (Odin + Rust + C++ do Qt). Fronteiras por **ABI em C**. O Petunia3D já tem `ffi` em C (698 linhas, códigos de erro estáveis), então a ponte com o Qt é plausível e talvez **mais simples** que o CXX-Qt.
- Mas a lógica que usa a matemática (comandos, projeto, `editor`) **continua em Rust**. O ganho de compilação seria **parcial**.
- Se portar também o resto, deixa de ser porte: é **reescrever a engine** (e várias peças perdem as bibliotecas Rust).

## O que realmente pesa na compilação (e como reduzir sem trocar de linguagem)

Não medi tempos de build (a máquina engasga e o `AGENTS.md` pede cuidado). O que se sabe (`LOCAL`): **379 pacotes** no Petunia3D, Qt/C++ de ~27,5 mil linhas, `mlua` com código C embutido, `wasmtime` ligado por padrão no Aipo.

| Alavanca | Efeito esperado |
|---|---|
| **Runtime sem Qt** (já decidido) | O ciclo diário de domínio e runtime **não compila Qt** |
| **Desligar `wasmtime`** do Aipo por padrão | Remove uma dependência muito pesada |
| **Dividir o workspace** em camadas (`world`, `schema`, `ir`, `commands` sem GPU, Qt, `mlua`, `rmcp`) | `cargo check` rápido no dia a dia |
| `cargo check` e `cargo nextest` no ciclo; build completo só nos *gates* | Menos espera |
| Linker rápido (`lld`/`mold`), cache de compilação (`sccache`), `opt-level` baixo para dependências em `dev` | Reduz tempo em builds incrementais |
| `CARGO_BUILD_JOBS=2` | Evita estourar a swap |

Todas são `HIPÓTESE` de ganho até serem medidas no seu computador (experimento S0 abaixo).

## Recomendação

1. **Não portar agora.** A matemática não é o gargalo; o custo é alto; você perderia Rapier, Kira, renet, AccessKit-winit, Luau e MCP em Rust.
2. **Medir primeiro (S0):** tempo de build frio e incremental de `world + schema + commands` **sem** Qt, mlua e rmcp, no seu computador. Se for aceitável, o problema era a configuração.
3. **Experimento S10 (opcional, curto):** portar **uma fatia pequena** para Odin e comparar com o Rust.
   - Fatia: `spline` + `sweep` + `triangulate` → malha de uma estrada.
   - Medir: **tempo de porte**, **tempo de compilação**, desempenho e **saída idêntica** (teste diferencial contra dados de referência).
   - Decidir só depois, com números.
4. **Seguro barato desde já:** escrever os "núcleos" de geometria como **funções puras de dados** (estruturas simples, índices, arrays de `f32`), com **dados de referência em formato neutro** (JSON). Qualquer porte futuro pode ser validado por eles.

### Quando reabrir
- S0 mostra que mesmo sem Qt o build é inaceitável.
- S10 mostra ganho grande **e** as bibliotecas que faltam podem ser ligadas por C sem trazer C++ pesado.
- O objetivo mudar para "engine mínima e portátil", sem o editor Qt.

## Fontes
- Odin `vendor:`: https://github.com/odin-lang/Odin/tree/master/vendor · https://pkg.odin-lang.org/vendor/
- `core:math/linalg`: https://pkg.odin-lang.org/core/math/linalg/
- Jolt para Odin: https://github.com/jrdurandt/joltc-odin
- Releases e mudanças: https://github.com/odin-lang/Odin/releases/tag/dev-2026-10 · https://odin-lang.org/news/newsletter-2026-q1/
- Aipo, baseline Odin histórica e pivô para Rust: `aipo-lang/docs/canon/Construindo a Aipo — Livro de Implementação…` e `…Rust Poppy Pivot…` (`LOCAL`)

## Decisões ligadas
`DR-20` linguagem do núcleo · `DR-02` base técnica.
