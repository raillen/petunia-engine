# Primitivas e edição de malha para construir mapas

**Revisão:** 2026-10-24 · **Fonte:** leitura e medição de `petunia3D/crates/mesh` (22,9 mil linhas) e do código da engine (`LOCAL`). Decisão relacionada: [DR-22](../06-decisoes/registro-de-decisoes.md).

## Resumo (TL;DR)

Há **três níveis** de edição para construir mapas. O nível 1 **já funciona**. Recomendo o nível 2 agora e o nível 3 por etapas, com aprovação sua, porque ele quase **dobra** o tamanho do código da engine.

| Nível | O que é | Estado | Custo |
|---|---|---|---|
| **1. Brushes paramétricos** | Formas com tamanho e detalhe, movidas com gizmo e snap | **Feito** | já pago |
| **2. Edição de brushes** | Arrastar uma face para redimensionar, cortar, subtrair (portas e janelas) | Planejado, **recomendo agora** | ~1,5 mil linhas (**estimativa**), sem dependência nova |
| **3. Edição de malha** | Vértices, arestas e faces: extrudar, inset, bevel, loop cut | Planejado, **precisa do seu OK** | ~11,6 mil linhas copiadas |

## O nível 1: o que já existe

Seis formas: **caixa, rampa, cilindro, cone, esfera e escada**. Cada uma tem `size` e uma coluna `detail` (segmentos nas formas redondas, degraus na escada).

| Ferramenta | Como |
|---|---|
| Criar | Menu **Criar**, trilho de primitivas, `Ctrl+1/2/3`; nasce na frente da câmera, no chão, alinhado a 0,5 m |
| Mover, girar, escalar | Gizmo no viewport, teclas `W`, `E`, `R` |
| Snap | Mover alinha a **coordenada absoluta** (o objeto cai na grade); girar e escalar alinham o valor final. `Ctrl` inverte |
| Editar números | Inspetor: campos com expressões e arrasto |
| Desfazer | Um `Ctrl+Z` por arrasto |
| Copiar e apagar | `Ctrl+D`, `Delete` |

Cada forma tem **teste de orientação das faces**: nenhum triângulo fica virado para dentro (não dá para ver malhas num teste automático, então o teste confere a geometria).

## O que o Petunia3D tem

Medido em `crates/mesh`:

| Grupo | Arquivos (linhas) | O que faz |
|---|---|---|
| **Núcleo** | `lib` (1365), `half_edge` (884), `topology` (330), `triangulate` (234), `validation` (407), `cleanup` (516) | Malha, arestas ligadas, reparo e checagem |
| **Primitivas** | `primitives` (1389) | cubo, plano, cilindro, cone, cunha, círculo, toro, icosfera, esfera, cápsula, revolução, polígono extrudado |
| **Operações** | `ops` (3399), `inset` (488), `bevel` (759), `loop_cut` (1028), `connect`, `loops` | extrudar, empurrar e puxar, inset, bevel, corte em laço |
| **Lógica booleana** | `boolean` (781), `boolean_cleanup` (1322), `boolean_partition`, `imprint` (1057) | somar e subtrair sólidos |
| **UV** | `uv*` (7 arquivos), `uv_xatlas` | desdobrar para texturas |
| **Desenho** | `poly_pen`, `knife`, `curve`, `sweep`, `shape_ops` | ferramentas de modelagem manual |

### Dependências de terceiros (verificadas arquivo a arquivo)

| Crate | Quem usa | No subconjunto proposto? |
|---|---|---|
| `glam` | quase tudo | **sim** (matemática de vetores) |
| `serde`, `thiserror` | núcleo | **sim** |
| `manifold-rust` | só `boolean.rs` | não |
| `xatlas-rs-v2`, `bytemuck` | só `uv_xatlas.rs` | não |
| `geo` | `shape_ops`, `profile_geo`, `imprint` | não |

**Subconjunto proposto:** `lib`, `half_edge`, `primitives`, `ops`, `inset`, `bevel`, `loop_cut`, `triangulate`, `topology`, `validation`, `cleanup`, `connect`, `loops`, `obj` somam **11.612 linhas** e usam só `glam`, `serde` e `thiserror`. Dois pontos soltos precisam de ajuste: um teste em `loop_cut.rs` (usa `boolean`) e uma chamada a `sweep` em `ops.rs`.

## Nível 2: edição de brushes (em andamento)

**Feito em 2026-10-24:** empurrar/puxar (`P`), extrudar (`E`), **cavar** (caixa menos caixa gera até 6 caixas) e **tornar oco** (6 paredes). Cada operação é **um** `world.batch`: um `Ctrl+Z` desfaz tudo. Falta: cortar com plano, arranjo, espelhar.

É o que ferramentas de mapa (Hammer, TrenchBroom) fazem, sem precisar de malha editável.

| Ferramenta | Como funciona | Dependência |
|---|---|---|
| **Alças de face** (**feito**: `P` e `E`) | Arrastar a face de uma caixa muda `size` e `position` juntos, com snap | nenhuma |
| **Redimensionar pela caixa** | Alças nos cantos e nas arestas | nenhuma |
| **Cortar com plano** | Divide um brush em dois | nenhuma |
| **Brush subtrativo** (**feito para caixas**: cavar e tornar oco) | Cavar portas e janelas | caixas: nenhuma; formas livres: CSG (`manifold-rust`), no nível 3 |
| **Arranjo** | Cópias em linha ou círculo (colunas, degraus, cercas) | nenhuma |
| **Espelhar** | Simetria de uma construção | nenhuma |

**Por que nível 2 primeiro:** entrega o que mais pesa em construção de mapa (redimensionar rápido, alinhar na grade) **sem** trazer 11 mil linhas. Todas essas ferramentas viram **comandos** (desfazem e viajam por rede).

## Nível 3: edição de malha

| Opção | O que é | Custo | Quando |
|---|---|---|---|
| **A. Não trazer** | Malhas customizadas vêm de fora (GLB, feito no Petunia3D) | zero | Se o foco for só blocagem e cenários |
| **B. Subconjunto** | Crate `petunia_mesh` com as 11,6 mil linhas, **sem** UV, booleano e desenho | médio-alto | Se a engine precisar editar malhas **dentro** do editor |
| **C. Tudo** | O crate inteiro, com UV e booleano | muito alto (traz `xatlas` em C++, `manifold`, `geo`) | Só se a engine virar também um modelador |

**Recomendação: A agora, B quando a fase 2 mostrar a necessidade.** Razões:

- O Petunia3D **já é** o modelador. Importar GLB resolve o caso comum.
- O subconjunto B tem **11,6 mil linhas**, e a engine inteira tem 8,6 mil. É uma decisão de manutenção, não só de cópia.
- As cópias viram **legado** (regra do `AGENTS.md`): o Petunia3D usa `ctx`, `idx`, `cfg`; a engine proíbe. Migrar custa.

### Se escolhermos B (plano)

1. Copiar para `crates/petunia_mesh/`, **sem alterar**, registrando a origem em `ATTRIBUTION.md`.
2. Resolver os dois acoplamentos e compilar com `glam` **só dentro** do crate. **Nada de `glam` na API pública, no schema ou nos arquivos** (regra de bibliotecas): a fronteira converte para `Vec3` da engine.
3. Tabela nova `MeshRef` no mundo (aponta para uma malha) e comandos `mesh.extrude`, `mesh.inset`, `mesh.bevel`, com desfazer.
4. Gizmo de componente (vértice, aresta, face) usando o mesmo mecanismo de **gesto** do gizmo atual.
5. Migrar nomes (`ctx`, `idx`, `cfg`) **ao tocar** cada módulo, com teste de regressão antes.

## Riscos

| Risco | Efeito |
|---|---|
| Subconjunto B tem testes que dependem dos módulos excluídos | Parte dos testes não roda no destino; reescrever |
| Malha editável pede formato de arquivo novo | O formato de texto precisa guardar malhas (tamanho, diffs) |
| A interface de edição de componentes (vértice, aresta, face) não existe na engine | É trabalho novo, além da cópia do crate |

## Próximo passo sugerido

Fazer o **nível 2** (alças de face e snap). Se você quiser o nível 3, me diga **A, B ou C** e eu abro o plano de cópia.
