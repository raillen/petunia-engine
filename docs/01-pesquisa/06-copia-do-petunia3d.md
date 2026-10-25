# O que copiar do Petunia3D (inclui a GUI)

**Revisão:** 2026-10-24 · **Fonte:** leitura e medição direta de `~/Documentos/Projetos/petunia3D` (`LOCAL`). Direitos: código seu, sem restrição (`DR-01`).

## Resumo (TL;DR)

A GUI do Petunia3D tem cerca de **27,5 mil linhas de C++ (Qt Widgets)**, 2,3 mil de QML e 7,5 mil de Rust na ponte. Medi o acoplamento de cada arquivo a conceitos de modelagem:

| Decisão | Linhas | Módulos | O que é |
|---|---:|---:|---|
| **COPIAR** | ~10,1 mil | 34 | Widgets e peças de shell **genéricos** (campo numérico, roda de cor, paleta de comandos, tema, layout…) |
| **ADAPTAR** | ~13,2 mil | 9 | Vale a base, mas há trabalho (viewport GL, estrutura, canvas de nós, janela do shell) |
| **NÃO COPIAR** | ~4,2 mil | 6 | Específico de modelagem e pintura |

Mais: o crate de **configuração** (i18n, atalhos, temas: 5,1 mil linhas), módulos **neutros** do crate `editor` e os **assets** (temas, atalhos, idiomas, ícones próprios).

**Como copiar:** primeiro como **cópia fotografada** (o Petunia3D ainda está em integração), depois **extrair** para `petunia-libs/ui-kit`, para as duas aplicações usarem a mesma fonte.

## Como a GUI é construída (por que dá para reaproveitar)

- **Qt Widgets em C++**, ligada ao Rust por **CXX-Qt**. QML existe, mas é uma prévia separada (`README` do crate).
- **A lógica fica em Rust puro**, no crate `editor`, testável sem tela. A GUI "só apresenta e emite intenções".
- **Tokens visuais numa fonte só.** `petunia_theme` gera a folha de estilo **uma vez por troca de tema**. **Cores em hexadecimal dentro de widgets são proibidas**, e um teste (`ui_theme_tokens`) garante isso.
- **Testes de contrato:** `component_contracts`, `ui_contract_ids`, `ui_i18n_catalog`, `ui_thread_guard`, `shell_v2`, `editor_parity`, galeria de componentes.

## Cuidados antes de copiar (honestidade)

| Cuidado | Evidência |
|---|---|
| A acessibilidade **não está certificada** | `README` do crate: "build/test não certifica … a11y nem performance" |
| A interface foi descrita como **"mal polida"** | `CONTEXT.md`; ondas D–F de acabamento ainda planejadas |
| O branch de integração **ainda não foi enviado** | `CONTEXT.md` (2026-10-19) |
| O build de Qt/C++ **pesa na sua máquina** | `AGENTS.md`: `CARGO_BUILD_JOBS=2`, swap cheia |
| O `README` do crate cita "entrada padrão Slint" | Está **desatualizado**: o ADR-C11 removeu o Slint |

Consequência: copia-se **componentes e desenho**, não "polimento pronto". A auditoria de acessibilidade (S8) vem antes de declarar a GUI acessível.

## GUI: COPIAR (34 módulos, ~10,1 mil linhas)

Densidade de termos de modelagem baixa (≤ 1,5 por 100 linhas) e pouca ponte com o Rust.

| Grupo | Módulos | Notas |
|---|---|---|
| **Entradas numéricas** | `number_field` (747), `expression`, `vector3_field`, `angle_dial`, `scrub` | Campo tipado: foco → editar → arrastar/setas = prévia → **Enter confirma uma vez** → **Esc restaura**. Aceita expressões (`2+3*4`, `1,5/2`, `pi`). Expressão inválida mantém o rascunho e marca o campo |
| **Cor** | `color_picker` (1027) | Anel de matiz + quadrado saturação/valor |
| **Formulários** | `bound_form`, `property_group`, `property_section`, `segmented` | Ligação de campos a dados |
| **Navegação e comandos** | `command_palette`, `pie_menu`, `region_navigator` (F6) | Base da navegação por teclado |
| **Diálogos** | `dialog_chrome`, `home_dialog`, `recovery_dialog`, `placeholder_message` | Início de projeto e **recuperação de autosave** |
| **Shell** | `app_bar`, `tool_rail`, `status_bar`, `drawer`, `operation_card`, `align_popover`, `context_actions`, `node_page`, `shell_controller` | Regiões fixas: cabeçalho, trilho, barra de status, gaveta |
| **Layout e tema** | `theme` (tokens), `layout_store` (layout versionado), `i18n.h`, `icons.h` | Fonte única de tokens |
| **Ferramentas de teste** | `component_gallery`, `selftest`, `ui_trace`, `app_host` | Galeria para ver todos os componentes |

## GUI: ADAPTAR (9 módulos, ~13,2 mil linhas)

| Módulo | Linhas | O que adaptar |
|---|---:|---|
| `gl_widget` | 1578 | Mantém o **padrão** de viewport OpenGL compartilhado com o Qt (FBO importado sem *readback*). Troca o que desenha pelo renderer da engine |
| `nav_cube` | 443 | Cubo de navegação; presets de vista |
| `node_canvas` | 2101 | `QGraphicsView` de nós. Serve de base para a **vista de nós** do Gameplay IR. O **Event Sheet** é outro widget (lista) |
| `structure_panel` | 1965 | Árvore de estrutura: vira a árvore de **entidades e tabelas** |
| `shell_window` | 1204 | Composição do shell; remover as ligações aos modos de modelagem |
| `operation_hud` | 1191 | HUD com parâmetros **vivos** da operação em curso. Reutilizar para pincel de terreno, desenho de rio e estrada |
| `viewport_header` | 490 | Cabeçalho da viewport |
| `properties_panel` | 212 | Inspetor genérico |
| `shell_harness` | 2975 | Mantém como **padrão de teste** do shell |

## GUI: NÃO COPIAR (6 módulos, ~4,2 mil linhas)

`editor_panel` (1385, 32 ligações à ponte de modelagem), `model_sections`, `workspace_sections`, `brush_cursor`, `canvas_input_contracts`, `editor_api.h`. Além do **QML** (2,3 mil linhas, prévia experimental); só `DesignTokens.qml` serve de referência.

## Rust de apoio

| Módulo | Linhas | Decisão | Nota |
|---|---:|---|---|
| `config/i18n.rs` | 2115 | **COPIAR** | Idiomas en/pt-BR com **paridade** testada |
| `config/keybinds.rs` | 1296 | **COPIAR** | Perfis de atalho |
| `config/theme.rs` | 857 | **COPIAR** | Registro de temas e tokens |
| `config/preferences.rs` | 764 | **COPIAR** | Preferências |
| `editor/numeric.rs`, `keymap_edit.rs`, `section_layout.rs`, `view_layout.rs`, `split_view.rs`, `tool_catalog.rs`, `context_actions.rs`, `tr.rs`, `theme.rs`, `perf.rs`, `thumbnail.rs`, `color_wheel.rs` | ~3,9 mil | **COPIAR** | Estado puro, sem tela. O `tool_catalog` é "o trilho projeta o catálogo, nunca mantém lista própria" |
| `editor/gizmo.rs`, `picking.rs`, `projection.rs`, `view_model.rs`, `bridge/{menus,preferences,outliner,library,commands,application}` | ~9,7 mil | **ADAPTAR** | Gizmos e seleção servem; tirar o que é de malha |
| `editor/{animate,brush_panel,paint_*,decal_tool,draw_*,shape_builder_tool}`, `bridge/{loop_cut,slice_knife,uv,paint_*,profile*,poly_pen}`, `viewport_soft` | ~12,6 mil | **NÃO** | Modelagem, pintura e viewport por CPU |

## Assets de interface

| Item | Conteúdo | Decisão |
|---|---|---|
| `themes/` | `petunia-dark`, `petunia-light`, **`petunia-high-contrast`** | **COPIAR** (alto contraste já existe) |
| `keymaps/` | 8 perfis: `petunia-default`, `petunia-simple`, `petunia-notebook`, Blender, 3ds Max, Maya, Cinema 4D | **COPIAR** (ajuda quem vem de outras ferramentas) |
| `locales/` | `en.toml`, `pt-BR.toml` | **COPIAR** |
| `icons/petunia-outline`, `petunia-filled`, `petunia-dual` | ~289 ícones por estilo, próprios | **COPIAR** |
| `icons/` lucide, tabler, phosphor, iconoir | Só **manifestos** (1 arquivo cada) | Usar como mapeamento |
| 3 arquivos `.zip` de ícones ("apple-like", "windows11-like", "technical-cad", ~5 MB) | Origem **não documentada** | **NÃO copiar** até verificar a procedência |
| Documento `PETUNIA_UI_001_DESIGN_SYSTEM.md` (39 KB) e docs de UI | Sistema de design e linguagem visual | **COPIAR** (é a especificação da GUI) |

> `ATTRIBUTION.md` do Petunia3D lista só imagens de diagnóstico. Antes de copiar ícones, **registrar autor, fonte e licença** de cada pacote (regra de proveniência da engine).

## Cópia fora da GUI, por fase

| Fase | O que copiar | Origem |
|---|---|---|
| 1 | `commands` (undo), `io_atomic`, `jobs`, `module` (registro), `schema_contracts` | `commands`, `core` |
| 1–2 | Formato de pacote e anti zip-bomb | `project/package.rs`, `format.rs` |
| 2 | Importação glTF/OBJ, triangulação, validação de malha | `project/import_*`, `mesh` (subconjunto) |
| 3 | `prefab`, padrão do `game_ready` (Project Doctor), `op_graph` (ideias do canvas), capabilities dos plugins (Lua → Luau) | `project`, `plugins` |
| 5 | `spline`, `path_generator`, `mesh/sweep`, `surface_attachment` | `project`, `mesh` |
| 6 | IK (dois ossos, FABRIK) para personagens | `project/ik.rs` |
| 8 | `mcp`, `ffi` | `mcp`, `ffi` |
| 1–4 | Sombreadores GL: grade, contorno, AO | `render/gl/shaders` |

## Como copiar (6 passos)

1. **Fotografar:** copiar os módulos para `petunia-libs/ui-kit/` registrando o **commit de origem** (e `ATTRIBUTION.md`).
2. **Desacoplar:** `number_field` tem 18 ligações à ponte Rust e `color_picker` 4. Trocar por interfaces pequenas.
3. **Tema:** manter a regra "sem hexadecimal em widget" e o teste que a garante.
4. **Galeria e testes:** levar `component_gallery`, `selftest`, `component_contracts` e o gate de paridade de idiomas.
5. **Acessibilidade:** rodar o experimento **S8** (teclado, leitor de tela, contraste) **antes** de declarar a GUI acessível.
6. **Extrair:** quando estável, `ui-kit` vira biblioteca compartilhada e o **Petunia3D passa a consumi-la** (uma fonte só, sem divergência).

## O que a engine precisa de GUI nova (não existe no Petunia3D)

| Widget | Para quê |
|---|---|
| **Event Sheet** (lista QUANDO / SE / FAÇA) | Lógica sem programar |
| **Editor de código** com diagnósticos, formatação e modo calmo | Aipo e Luau |
| **Painel do Project Doctor** | Validação com "Corrigir" |
| **Ferramentas de mundo** (terreno, rio, estrada, vegetação) | Pincéis e desenho de curvas |
| **Painel de clima e pós** | Hora do dia, presets, conforto sensorial |
| **Árvore de UI do jogo** | Editor da UI do jogo |
| **Narrador da cena** | Acessibilidade da viewport 3D |
| **Navegador de assets do jogo** | Biblioteca de prefabs e assets |
| **Painel "Testar online"** | N clientes + latência simulada |

## Decisões ligadas

`DR-04` UI do editor · `DR-18` como reusar a GUI · `DR-15` gates.
