# O que aproveitar do Petunia3D

**Revisão:** 2026-10-24 · **Fonte:** leitura direta de `~/Documentos/Projetos/petunia3D` (`LOCAL`)

## Resumo (TL;DR)

O Petunia3D tem cerca de **159 mil linhas de Rust** em 19 crates. Para a engine, o mais valioso **não é o modelador em si**. É a **infraestrutura de editor**: comandos com undo, splines, grafo de operações, plugins com permissões, MCP, pacotes seguros e a disciplina de qualidade.

O modelador (ferramentas de malha, pintura 2D, UV) deve continuar sendo o **Petunia3D**: um produto separado que **exporta assets** para a engine.

## Legenda das decisões

| Marca | Significa |
|---|---|
| **REUSAR** | Pegar o crate ou módulo quase como está. |
| **ADAPTAR** | Pegar a ideia e boa parte do código, com mudanças. |
| **INSPIRAR** | Copiar só o desenho, escrever de novo. |
| **FICA** | Permanece no Petunia3D. |

## Matriz de aproveitamento

### Núcleo de editor

| Módulo | O que faz (`LOCAL`) | Decisão | Uso na engine |
|---|---|---|---|
| `core/command.rs` (7,7 mil linhas), `commands` | `CommandDispatcher`: todo comando tem id, `can_execute`, execução transacional, undo/redo, `catch_unwind` | **ADAPTAR** | Espinha do editor. "Tudo é comando". |
| `core/schema_contracts.rs` | `CommandIntent` + JSON Schema (`schemars`); UI, CLI, Lua e MCP usam o mesmo contrato | **ADAPTAR** | Semente do schema único da API. |
| `core/module.rs` | Trait `Module` + `ModuleRegistry`; módulos não se conhecem | **REUSAR** | Isolar sistemas do editor. |
| `core/jobs.rs` | `rayon` + `flume`; workers nunca mudam estado; canais tipados | **REUSAR** | Modelo de jobs (bake, import). |

### Dados e formatos

| Módulo | O que faz (`LOCAL`) | Decisão | Uso na engine |
|---|---|---|---|
| `project/format.rs` | `.petunia` = ZIP com `manifest.json` + `document.json`; migra formato antigo | **ADAPTAR** | Formato de projeto e mundo. |
| `project/package.rs` | Pacote versionado; sem `..` no caminho; limite anti zip-bomb; escrita atômica | **REUSAR** | Pacote de jogo e de mods. |
| `project/io_atomic.rs` | Escrita atômica e limpeza de temporários | **REUSAR** | Autosave sem corromper. |
| `project/import_gltf.rs`, `export_model.rs` | glTF/GLB como formato principal; OBJ secundário | **REUSAR** | Pipeline de assets (modelos vindos do Petunia3D). |
| `project/prefab.rs` | Prefabs com tags, coleções, favoritos e `revision`; instâncias detectam "desatualizada" | **ADAPTAR** | Prefabs de cena e entidades. |
| `project/game_ready.rs` | Validador: severidade, código estável, alvo, ação recomendada | **INSPIRAR** | **Project Doctor** (validador do mapa e da acessibilidade). |

### Mundo natural (peças que já existem)

| Módulo | O que faz (`LOCAL`) | Decisão | Uso na engine |
|---|---|---|---|
| `project/spline.rs` | Spline Bézier cúbica; handles; quadros (frames) por ponto; tolerância; limite de pontos | **ADAPTAR** | Rios, estradas, cordas, cabos. |
| `project/path_generator.rs`, `mesh/sweep.rs` | Varredura de perfil ao longo de um caminho; orçamento de vértices | **ADAPTAR** | Malha de estrada, leito de rio, tubo de corda. |
| `project/surface_attachment.rs` | Ponto preso a um triângulo (barycentric + offset); detecta topologia mudada | **ADAPTAR** | Âncoras de corda, props presos ao terreno. |
| `mesh/triangulate.rs`, `half_edge.rs`, `uv_xatlas.rs` | Triangulação, topologia, atlas UV | **REUSAR** (parte) | Cruzamentos de estrada; bake de impostores. |

### Lógica visual

| Módulo | O que faz (`LOCAL`) | Decisão | Uso na engine |
|---|---|---|---|
| `project/op_graph.rs` (1,9 mil linhas) | Grafo de nós: portas tipadas, `NodeDescriptor` estático, UI gerada do descritor, nós desconhecidos preservados como "fantasma", limites duros (512 nós, 2048 arestas) | **INSPIRAR** | Modelo de dados do editor de nós. |
| `project/surface_recipe.rs`, `material_recipe.rs` | DAG com sockets tipados, detecção de ciclo, memoização, `RecipeParamSpec` (Float{min,max}, Choice…) | **INSPIRAR** | Grafo de materiais ("só no modo avançado"); desenho dos parâmetros. |

### Extensão e automação

| Módulo | O que faz (`LOCAL`) | Decisão | Uso na engine |
|---|---|---|---|
| `plugins` (2,2 mil linhas) | Lua 5.4 via `mlua`; manifesto com capabilities (nega por padrão); 1 milhão de instruções e 8 MiB por execução; scripts até 64 KiB | **ADAPTAR** | Trocar Lua 5.4 por **Luau**; unir com o modelo de permissões da Aipo. |
| `mcp` | Servidor MCP (`rmcp`): política nega por padrão, auditoria, lote, passa pelo mesmo dispatcher | **REUSAR** | Agentes de IA e automação (inclui assistentes de voz). |
| `ffi` | C-ABI com códigos de erro estáveis | **ADAPTAR** | Embutir a engine em outros programas. |

### Animação (fase posterior)

| Módulo | O que faz (`LOCAL`) | Decisão |
|---|---|---|
| `project/rig.rs`, `animation.rs`, `ik.rs` | Esqueleto, clipes, IK de dois ossos e FABRIK (determinístico) | **ADAPTAR** na fase de personagens. |

### Interface

| Módulo | O que faz (`LOCAL`) | Decisão |
|---|---|---|
| `config` | i18n com paridade en/pt-BR, keymaps por perfil, temas | **REUSAR** |
| `editor`, `ui-cxxqt` | Shell v2 em Qt 6: regiões fixas (cabeçalho, trilho de ferramentas, superfície de trabalho, estrutura, propriedades, gaveta); gate de acessibilidade; alvos de 24 px (WCAG 2.2) | **ADAPTAR** — Qt/CXX-Qt já decidido (`DR-04`) |
| `render` (GL 3.3, `glow`) | Fronteira neutra `RenderFrameInput`; sombreadores de grade, contorno, **SSAO** (`ao.frag`, `ao_blur.frag`) | **ADAPTAR** — OpenGL já decidido (`DR-03`) |

### Fica no Petunia3D

`module-model` (ferramentas de modelagem), `module-paint` (pintura 2D/3D, 10 mil linhas), `module-uv`, `mesh` (operações de edição como knife, bevel, boolean).

Regra: a engine **importa** o resultado (glTF e `.petunia`). Não carrega o modelador dentro do runtime.

## O que NÃO aproveitar

| Item | Motivo (`LOCAL`) |
|---|---|
| Slint, egui, WGPU do editor antigo | ADR-C11 removeu o Slint por completo e decidiu "não manter egui/WGPU em paridade". |
| `viewport_soft` (viewport por CPU) | Existe como fallback do modelador; a engine não precisa. |
| ECS especulativo | O próprio registro de decisões diz "ECS, scene graph universal: não adotados" **para o modelador**. A engine é outro caso (veja `DR-05`). |

## Duas lições de processo para herdar

1. **Mudou de toolkit três vezes** (egui → Slint → Qt, ADR-C11). Trocar de UI custa semanas. A decisão por Qt/CXX-Qt já foi tomada; o experimento S4 só **valida** (não reabre) e mede o custo de build.
2. **A máquina engasga ao compilar Qt/C++** (`CARGO_BUILD_JOBS=2`, swap cheia, em `AGENTS.md`). O custo de build do editor é um critério de decisão, não um detalhe.

## Estado do Petunia3D (importante)

O `CONTEXT.md` (2026-10-19) registra: branch de integração ainda não enviada, Ondas D–F de polimento da interface **planejadas**, e relato de interface "mal polida". Ou seja, o Shell v2 é bom como **base de desenho**, mas ainda não é um produto fechado.

> **Atualização:** a lista detalhada de cópia, incluindo a GUI arquivo por arquivo, está em [06-copia-do-petunia3d.md](06-copia-do-petunia3d.md).

## Decisões ligadas

`DR-01` licença · `DR-03` renderer · `DR-04` UI do editor · `DR-10` formatos · `DR-11` relação com o Petunia3D. Veja também a [especificação anterior](05-especificacao-anterior.md): ela já define `petunia-libs` como veículo de reuso.
