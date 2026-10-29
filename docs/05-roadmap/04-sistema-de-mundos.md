# Sistema de mundos — checklist de implementação

**Revisão:** 2026-10-27 · **Decisão:** [DR-33](../06-decisoes/registro-de-decisoes.md) · **Branch:** `feat/sistemas-de-mundo`

Este documento é a lista viva do que foi implementado e do que falta no sistema de mundos. Ele é atualizado a cada implementação, como pedido. O desenho técnico fica em [terreno, rios e estradas](../02-arquitetura/04-terreno-rios-estradas.md).

## Estado por sistema

| Sistema | Implementado | Falta | % |
|---|---|---:|---:|
| Terreno (dados) | `petunia_terrain`: chunks, amostras, materiais, furos, consultas, limites | — | 90% |
| Terreno (geração) | fBm determinístico próprio; parâmetros validados | Biomas, importar/exportar heightmap | 70% |
| Terreno (edição) | Brushes 3D, 4 falloffs, 6 operações, erosão térmica e hidráulica, espelhamento x/z, *stamp* de heightmap e de malha 3D | Brushes plano/caminho, máscaras | 92% |
| Terreno (render) | Malha por chunk, cores por camada (8), LOD 1/2/4, saias, furos, plano de corte | Materiais por textura, culling por chunk, verificação visual | 80% |
| Terreno (física) | Heightfield no Rapier; trimesh nos chunks com furo | Terreno com transformação não-identidade | 80% |
| Terreno (editor) | Pincel `terrain.begin/drag/end/cancel/set/preview`, anel, prévia ao vivo, um desfazer; menu **Criar > Terreno**, ferramenta **T**, barra de opções, **Gerar terreno** e **Raio X** | Plano de corte; ajuste fino de raio X; teste de fumaça estável | 85% |
| Cavernas (SDF) | `petunia_sculpt` (chunks 16³, surface nets, 5 modos, caminho, gerador, fundir, facetada) + espelhamento X, preencher, oco, ruído e **booleanos** (`sculpt.boolean`) | *Stamp* de malha na caverna | 95% |
| Caverna ↔ terreno | Furos explícitos (`terrain.holes`), furos automáticos dos túneis (`terrain.holes_from_caves`), render sem as células, física trimesh | Costura visual, ajuste fino do raio do furo | 85% |
| `surface_follow` | Tabela `SurfaceFollow`, comando `surface.follow`, `surface.set` inverso, preferência, re-projeção, alinhamento à normal e **um desfazer** junto de `generate/brush/paint/erode/hydraulic/stamp` e do arrasto | UI dedicada (o inspetor genérico já edita a tabela) | 95% |
| Erosão | Térmica determinística e hidráulica por gotas determinísticas, com desfazer | Ajuste visual fino | 90% |
| Rios | — (outro agent) | Spline, escavação, malha de água, fluxo, espuma, confluências, lagos, quedas | 0% |
| Estradas | — (outro agent) | Grafo, perfis, cruzamentos, aterro/corte, pontes, decals, navegação | 0% |
| Spline/sweep | — (outro agent) | Portar Bézier + varredura do Petunia3D | 0% |
| Água (render) | — (outro agent) | Flow map, Gerstner 4–8, refração, espuma | 0% |

## Implementado

### 2026-10-28 — ferramentas avançadas e automação

| Item | Evidência |
|---|---|
| `terrain.stamp`: carimba um retângulo de alturas (replace/add/subtract/min/max) | 4 testes de crate + 1 de comando |
| `terrain.erode_hydraulic`: gotas determinísticas, com desfazer | 3 testes de crate + 1 de comando |
| `terrain.holes_from_caves`: abre furos onde os túneis chegam à superfície | 1 teste de comando; ação **Operações > Furos das cavernas** na janela |
| `surface.set` + `surface.follow` automático: terreno e objetos em **um** desfazer | teste de comando e de editor |
| **Plano de corte** no renderizador (`section_height` no frame e no shader) e na barra do terreno | teste de sessão; compila |
| **Espelhamento X** no pincel de caverna (`sculpt.mirror`) | teste de editor: traço e espelho em um desfazer |
| Checkboxes **Espelhar X/Z**, **Raio X** e **Corte** na barra do terreno | compila; **não vistos em imagem** |
| **Preencher**, **Oco** e **Ruído** em cavernas (`sculpt.fill/hollow/noise`), com botões na barra | teste de editor: cada um é um desfazer |
| **Carimbo de malha 3D** (`terrain.stamp_mesh`): a superfície de uma malha vira altura do terreno, com transformação | 1 teste de comando |
| **Booleano de cavernas** (`sculpt.boolean`): assa as malhas das cavernas, roda `petunia_mesh::boolean` (união, subtração, interseção) e remove/mantém originais em um desfazer | 1 teste de editor; ação **Operações > Booleano de cavernas…** na janela com diálogo de seleção |

### 2026-10-28 — janela Qt do terreno

| Item | Evidência |
|---|---|
| **Criar > Terreno** cria um heightmap vazio e seleciona a ferramenta | compila; a ação aparece no menu e na paleta |
| Ferramenta **Esculpir terreno** (`T`) com arrasto ao vivo, anel e cancelamento | teste de protocolo (`terrain_tool`) e compilação da janela |
| Barra de opções: operação, forma, falloff, raio, força, material, pintar, espelhar X/Z e raio X | compila; **vista em display virtual** numa execução do teste de fumaça |
| **Gerar terreno** (Operações e paleta): cria o terreno se preciso e gera 3×3 blocos | verificação do teste de fumaça: **9 blocos** numa execução |
| **Raio X**: o terreno fica translúcido enquanto a ferramenta está ativa | estado exposto por `terrain.xray`; **não olhado em imagem** |
| `terrain.hover` atualiza o anel sem arrastar | teste do editor |
| Teste de fumaça do Qt | **intermitente**: 9/12 na última execução (gizmo, geração por paleta e Tab falharam); as etapas de caverna, script, grafo, canvas e Jogar passaram |

### 2026-10-27 — segundo slice: integração completa do terreno

| Item | Evidência |
|---|---|
| Crate `petunia_terrain` com heightmap em chunks, brushes 3D, falloffs, geração, erosão e furos | **33 testes** do crate |
| `ValueKind::Terrain` + `Value::Terrain` no mundo, com JSON compacto (alturas curtas, materiais, furos) | testes de ida e volta e de JSON hostil |
| Tabelas `Terrain` e `SurfaceFollow` nas tabelas padrão do projeto | teste de cena: salvar, abrir e comparar o terreno |
| Comandos `terrain.create`, `terrain.generate`, `terrain.brush`, `terrain.paint`, `terrain.patch`, `terrain.holes`, `terrain.erode`, `surface.follow` | **17 testes** de comandos, incluindo desfazer/refazer de cada um |
| Consultas `terrain.height` e `terrain.info` | teste de altura/normal e de contagem de blocos |
| Render: malha por chunk, 8 cores de camada, LOD 1/2/4 por distância, saias, furos | **22 testes** de render (malha, LOD, furo, impressão digital) |
| Adaptador OpenGL: cores por vértice no shader, buffer de cor por malha, sem *culling* no terreno | compila; **não visto em janela** |
| Física: heightfield por chunk; trimesh quando há furo | **20 testes** de física, incluindo bola que repousa no heightfield e furo que remove triângulos |
| Editor: `terrain.begin/drag/end/cancel/set/preview`, anel do pincel, prévia ao vivo e um passo de desfazer; cancelar não deixa rastro | teste de arrasto completo no `petunia_editor` |
| `surface.follow`: re-projeta objetos com `SurfaceFollow`, com `offset` e `align_normal`; desfazer | teste de comando e teste do editor |
| Preferência `preferences.surface_follow` (padrão para objetos novos) | teste do editor: o objeto criado ganha a linha |
| Erosão térmica determinística e espelhamento x/z no brush | testes de comando e de crate |

**Gates:** `cargo fmt --check` ok; `cargo clippy --workspace --all-targets -D warnings` ok; **462 testes, 0 falhas** no workspace (2 threads).

## Fila de implementação (ordem)

| # | Item | Depende de | Critério de pronto |
|---|---|---|---|
| 1 | Estabilizar o teste de fumaça (coordenadas fixas, foco do Tab, paleta) | editor | 12/12 repetível |
| 2 | Arame no raio X (hoje translucidez + plano de corte) | 1 | ver o interior sem alterar a superfície |
| 3 | *Stamp* de malha 3D diretamente sobre o volume da caverna | `petunia_sculpt` | teste de forma |
| 4 | Rios e estradas | outro agent | — |

## Não verificado (honesto)

| Item | Estado |
|---|---|
| Terreno visível na janela Qt | **visto em display virtual** numa execução do teste de fumaça (9 blocos gerados); a aparência e o LOD não foram julgados |
| Ferramenta de terreno pela janela | **parcial**: o teste de fumaça gerou o terreno; o arrasto não foi confirmado por captura |
| LOD sem costura visível | **não visto**; as saias existem, mas não foram olhadas em imagem |
| Cores das 8 camadas | **não vistas**; os testes conferem contagem e furos, não a aparência |
| Personagem andando no heightfield pela janela | **não rodou**; o teste de física prova a colisão, não o jogo |
| Raio X | **translucidez implementada**; plano de corte e arame **não** |
| Teste de fumaça | **intermitente**: 9/12 na última execução; gizmo, geração por paleta e Tab falharam |
| Erosão em terreno grande | **não medida**; o teste usa 8×8 |
| Terreno com `Transform` deslocado | **não suportado**; os dados são do mundo e o collider assume origem |

## Fora desta fase (declarado)

LOD de SDF, malhagem na GPU, SDF para o mapa inteiro, edição de caverna em primeira pessoa, *streaming* de mundo infinito e navmesh automática.
