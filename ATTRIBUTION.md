# Atribuições e proveniência

Regra do projeto: todo código ou arquivo copiado de outro lugar entra aqui, com origem, autoria e licença. Código copiado é **legado**: migra para as regras do `AGENTS.md` quando for tocado.

## Copiado do Petunia3D (mesma autoria, reuso livre: DR-01)

**Origem:** `~/Documentos/Projetos/petunia3D`, commit `dec467127a7f` (2026-10-19), "Petunia3D Team". Cópia fotografada em 2026-10-24; a extração para uma biblioteca compartilhada (`ui-kit`) vem depois ([DR-18](docs/06-decisoes/registro-de-decisoes.md)).

| Destino na engine | Origem no Petunia3D | Mudanças |
|---|---|---|
| `editor/src/ui_kit/petunia_theme.{h,cpp}` | `crates/ui-cxxqt/cpp/petunia_theme.*` | 2026-10-25: estilos novos (barra principal, títulos de painel, seções do inspetor, opções da ferramenta); os existentes ficaram iguais |
| `editor/src/ui_kit/petunia_icons.h` | `crates/ui-cxxqt/cpp/petunia_icons.h` | nenhuma |
| `editor/src/ui_kit/petunia_expression.h` | `crates/ui-cxxqt/cpp/petunia_expression.h` | nenhuma |
| `editor/src/ui_kit/petunia_scrub.{h,cpp}` | `crates/ui-cxxqt/cpp/petunia_scrub.*` | nenhuma |
| `editor/src/ui_kit/petunia_number_field.{h,cpp}` | `crates/ui-cxxqt/cpp/petunia_number_field.*` | nenhuma |
| `editor/src/ui_kit/petunia_vector3_field.{h,cpp}` | `crates/ui-cxxqt/cpp/petunia_vector3_field.*` | nenhuma |
| `editor/src/ui_kit/petunia_nav_cube.{h,cpp}` | `crates/ui-cxxqt/cpp/petunia_nav_cube.*` | nenhuma |
| `editor/src/ui_kit/shell/petunia_pie_menu.{h,cpp}` | `crates/ui-cxxqt/cpp/shell/petunia_pie_menu.*` | nenhuma |
| `editor/src/ui_kit/shell/petunia_i18n.h` | `crates/ui-cxxqt/cpp/shell/petunia_i18n.h` | nenhuma |
| `editor/assets/themes/*.json` | `assets/themes/<tema>/theme.toml` (`petunia-dark`, `petunia-light`, `petunia-high-contrast`) | TOML convertido para JSON; mesmas chaves. 2026-10-25: no `petunia-light`, cinzas de fundo e bordas ajustados (o destaque e os textos ficaram iguais) |
| `editor/assets/icons/*.svg` (287 ícones) | `assets/icons/petunia-outline/svg/**` | pastas achatadas em uma só (sem colisão de nomes) |

**Licença dos ícones:** pacote `petunia-outline` 1.0.0, autor "Petunia3D Team", licença MIT (conforme o `manifest.toml` do pacote).

**Não copiado:** os três arquivos `.zip` de ícones de origem não documentada (`apple-like`, `windows11-like`, `technical-cad`) ficam de fora até a procedência ser verificada.

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
