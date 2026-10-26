# Atribuições e proveniência

Regra do projeto: todo código ou arquivo copiado de outro lugar entra aqui, com origem, autoria e licença. Código copiado é **legado**: migra para as regras do `AGENTS.md` quando for tocado.

## Copiado do Petunia3D (mesma autoria, reuso livre: DR-01)

**Origem:** `~/Documentos/Projetos/petunia3D`, commit `dec467127a7f` (2026-10-19), "Petunia3D Team". Cópia fotografada em 2026-10-24; a extração para uma biblioteca compartilhada (`ui-kit`) vem depois ([DR-18](docs/06-decisoes/registro-de-decisoes.md)).

| Destino na engine | Origem no Petunia3D | Mudanças |
|---|---|---|
| `editor/src/ui_kit/petunia_theme.{h,cpp}` | `crates/ui-cxxqt/cpp/petunia_theme.*` | 2026-10-25: medidas (24 px, 12 px, 16 px) e a folha de estilos **reescritas** ([design](docs/02-arquitetura/15-design-da-interface.md)); tokens e API iguais |
| `editor/src/ui_kit/petunia_icons.h` | `crates/ui-cxxqt/cpp/petunia_icons.h` | nenhuma |
| `editor/src/ui_kit/petunia_expression.h` | `crates/ui-cxxqt/cpp/petunia_expression.h` | nenhuma |
| `editor/src/ui_kit/petunia_scrub.{h,cpp}` | `crates/ui-cxxqt/cpp/petunia_scrub.*` | nenhuma |
| `editor/src/ui_kit/petunia_number_field.{h,cpp}` | `crates/ui-cxxqt/cpp/petunia_number_field.*` | nenhuma |
| `editor/src/ui_kit/petunia_vector3_field.{h,cpp}` | `crates/ui-cxxqt/cpp/petunia_vector3_field.*` | nenhuma |
| `editor/src/ui_kit/petunia_nav_cube.{h,cpp}` | `crates/ui-cxxqt/cpp/petunia_nav_cube.*` | 2026-10-25: suporte a menu de projeção via seta e rótulo, presets isométrica e game 2:1, sincronização de projeção com a câmera, arrasto para orbitar e acesso por teclado |
| `editor/src/ui_kit/shell/petunia_pie_menu.{h,cpp}` | `crates/ui-cxxqt/cpp/shell/petunia_pie_menu.*` | nenhuma |
| `editor/src/ui_kit/shell/petunia_segmented.{h,cpp}` | `crates/ui-cxxqt/cpp/shell/petunia_segmented.*` | nenhuma (copiado em 2026-10-25) |
| `editor/src/command_palette.{h,cpp}` | ideia e comportamento de `crates/ui-cxxqt/cpp/petunia_command_palette.*` | **reescrito**: busca nas ações dos menus (não na ponte Rust), busca tolerante, destaque, recentes |
| `editor/src/ui_kit/shell/petunia_i18n.h` | `crates/ui-cxxqt/cpp/shell/petunia_i18n.h` | nenhuma |
| `editor/assets/themes/*.json` | `assets/themes/<tema>/theme.toml` (`petunia-dark`, `petunia-light`, `petunia-high-contrast`) | TOML convertido para JSON; mesmas chaves. 2026-10-25: cores dos temas claro e escuro trocadas (neutros quentes, acento violeta); alto contraste igual |
| `editor/assets/icons/*.svg` (287 ícones) | `assets/icons/petunia-outline/svg/**` | pastas achatadas em uma só (sem colisão de nomes); `search.svg` corrigido (caminho sem `M`/`L` não desenhava o cabo da lupa) |

**Licença dos ícones:** pacote `petunia-outline` 1.0.0, autor "Petunia3D Team", licença MIT (conforme o `manifest.toml` do pacote).

**Não copiado:** os três arquivos `.zip` de ícones de origem não documentada (`apple-like`, `windows11-like`, `technical-cad`) ficam de fora até a procedência ser verificada.

### Edição de malha (copiado em 2026-10-25, mesmo commit `dec467127a7f`)

Destino: `crates/petunia_mesh/src/legacy/`. Origem: `crates/mesh/src/` do Petunia3D. Os arquivos são **legado** (regra do `AGENTS.md`): nomes curtos e índices diretos migram quando forem tocados. A API da engine (`crates/petunia_mesh/src/lib.rs`) é nova.

| Destino | Origem | Mudanças |
|---|---|---|
| `legacy/mod.rs` | `lib.rs` | lista de módulos reduzida ao subconjunto; tiradas `from_sweep` e `to_triangles*` (dependiam de `sweep` e `shading`); `mark_sharp` trazido de `shading.rs`; `crate::` virou `crate::legacy::` em todos os arquivos |
| `legacy/{half_edge,topology,triangulate,validation,cleanup,inset,bevel,loop_cut,connect,loops,knife,boolean,boolean_partition,obj}.rs` | mesmos nomes | só o caminho `crate::legacy::` |
| `legacy/ops.rs` | `ops.rs` | tirada `sweep` (dependia de `sweep.rs`) |
| `legacy/boolean_cleanup.rs` | `boolean_cleanup.rs` | sem o novo layout de UV (módulo `uv_layout` não copiado); `annotate_mesh_provenance` marcada como não usada; 2 testes de UV tirados |
| `legacy/primitives.rs` | `primitives.rs` | chamadas a `layout_uv_charts` tiradas; 4 `let mut` viraram `let` |

**Não copiado:** `uv*` (7 arquivos), `uv_xatlas`, `shading`, `sweep`, `curve`, `poly_pen`, `shape_ops`, `profile_geo`, `imprint`, `arrangement`, `solid_builder`, `morph`, `path_frames` e os testes de integração de `crates/mesh/tests/`. Testes tirados dos módulos copiados: `sweep_polyline_creates_mesh`, `equalize_texel_density_scales_islands`, `inherits_uv_when_every_face_comes_from_a`, `mixed_origin_results_get_a_fresh_non_overlapping_layout` (dependiam do que não foi copiado). Os outros **164 testes** do Petunia3D passam aqui.

## Portado do Petunia3D (lógica reescrita em Rust da engine, mesmo desenho)

| Destino na engine | Origem no Petunia3D | O que foi portado |
|---|---|---|
| `crates/petunia_editor/src/gizmo.rs` | `crates/editor/src/projection.rs` (`compute_gizmo`, constantes `GIZMO_*`) e `crates/editor/src/bridge/gizmo.rs` (regras de acerto) | Gizmo em espaço de tela: haste de 72 px, seta, quadrado, planos, anéis com arco da frente, anel da vista, centro, modo universal com zona morta |
| `crates/petunia_render/src/grid.rs` | `crates/render/src/scene.rs` (`grid_plane_lines`, `grid_axis_lines`) | Grade com linha maior a cada 10 e eixos X/Z tonalizados |
| Política de pivô em `crates/petunia_editor/src/tools.rs` | `crates/core/src/state.rs` (`PivotPoint`, `TransformOrientation`) | Centro da caixa e base; orientação global e local (o cursor 3D foi retirado; o pivô passou a ser por objeto) |
| Alvos de snap em `crates/petunia_editor/src/tools.rs` | `crates/core/src/state.rs` (`SnapTarget`) | Inteligente, Grade, Incremento, Ponto, Superfície |
| Modo Livre (`free_*` em `tools.rs`, duplo toque em `editor/src/main_window.cpp`) | `docs/manual/modeling.md` (duplo toque em G/R/S, 350 ms) | Comportamento: mouse muda o valor, X/Y/Z restringem, número digitado fixa |

## Algoritmos públicos implementados do zero (nenhum código copiado)

| Onde | Algoritmo | Referência |
|---|---|---|
| `crates/petunia_sculpt/src/field.rs` | *Surface nets* (um vértice por célula cortada, um quadrilátero por aresta cortada) | Gibson, 1998, *Constrained Elastic SurfaceNets* |
| `crates/petunia_sculpt/src/lib.rs` | Mínimo e máximo suaves polinomiais; distância com sinal de caixa e de cápsula | Técnicas amplamente documentadas (Inigo Quilez) |
| `crates/petunia_sculpt/src/path.rs` | Spline de Catmull-Rom | Catmull e Rom, 1974 |
| `crates/petunia_sculpt/src/simplify.rs` | Douglas-Peucker | Douglas e Peucker, 1973 |
| `crates/petunia_sculpt/src/generate.rs` | Ruído de valor, *splitmix64*, salas ligadas por túneis dobrados por ruído | Mark et al., 2015, *Procedural Generation of 3D Caves for Games on the GPU* (a ideia; código nosso) |

## Usado do projeto Aipo (mesma autoria, MIT; sem cópia)

O Script Host (`crates/petunia_script`) usa as crates `aipo-*` do projeto **Aipo** por **dependência de caminho** (`../aipo-lang`), sem copiar código para este repositório. Lista e recursos em [dependências](docs/dependencies.md); decisão em `DR-25`.
