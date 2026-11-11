# Primitivas e edição de malha para construir mapas

**Revisão:** 2026-10-27 · **Fonte:** leitura e medição de `petunia3D/crates/mesh` (22,9 mil linhas) e do código da engine (`LOCAL`). Decisão relacionada: [DR-22](../06-decisoes/registro-de-decisoes.md).

## Resumo (TL;DR)

Há **dois caminhos** para construir mapas: brushes (níveis 1 e 2) e **booleano**. A edição por vértice, aresta, faca, inset, bevel e modo Tab foi **apagada** em 2026-11-09 ([DR-51](../06-decisoes/registro-de-decisoes.md)). O [DR-53](../06-decisoes/registro-de-decisoes.md) reabre só seleção de componente, loop cut na malha gravada e gizmo no componente, sem modo novo. O booleano continua: menu **Malha ▸ Booleano…**.

| Nível | O que é | Estado | Custo |
|---|---|---|---|
| **1. Brushes paramétricos** | Formas com tamanho e detalhe, movidas com gizmo e snap | **Feito** | já pago |
| **2. Edição de brushes** | Empurrar/puxar, extrudar, cavar, tornar oco | **Feito** em parte (falta cortar com plano, arranjo, espelhar) | sem dependência nova |
| **3. Edição de malha** | Inset, faca, bevel, loop cut, modo de componentes | **Removido** (DR-51). Fica o booleano | — |

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

## Nível 2: edição de brushes (feito em 2026-11-08)

**Feito em 2026-10-24:** empurrar/puxar (`P`), extrudar (`E`), **cavar** (caixa menos caixa gera até 6 caixas) e **tornar oco** (6 paredes). **Feito em 2026-11-08:** **cortar com plano**, **arranjo em linha**, **arranjo em círculo** e **espelhar**. **Feito em 2026-11-09:** **redimensionar pelos cantos e pelas arestas**. Cada operação é **um** desfazer.

É o que ferramentas de mapa (Hammer, TrenchBroom) fazem, sem precisar de malha editável.

| Ferramenta | Como funciona | Dependência |
|---|---|---|
| **Alças de face** (**feito**: `P` e `E`) | Arrastar a face de uma caixa muda `size` e `position` juntos, com snap | nenhuma |
| **Redimensionar pela caixa** (**feito**) | 8 cantos e 12 arestas âmbar na caixa selecionada. Arrastar segue o plano da vista; o canto ou a aresta oposta fica. `Esc` cancela. Cilindro, escada e malha não têm essas alças | nenhuma |
| **Cortar com plano** (**feito**) | Divide a caixa em duas com um plano perpendicular a um eixo dela | nenhuma |
| **Brush subtrativo** (**feito para caixas**: cavar e tornar oco) | Cavar portas e janelas | caixas: nenhuma; formas livres: CSG (`manifold-rust`), no nível 3 |
| **Arranjo** (**feito**) | Cópias em linha ou círculo (colunas, degraus, cercas) | nenhuma |
| **Espelhar** (**feito**) | Simetria de uma construção | nenhuma |

### Nível 2: o que foi feito em 2026-11-08

Todas são operações de dados: a janela só coleta os campos, o motor valida e grava, e cada uma vira **um** desfazer.

| Operação | Comando | Campos | Regra |
|---|---|---|---|
| **Cortar com plano** | `brush.cut_plane` | `axis` (`x`/`y`/`z` ou 0–2), `offset` em metros a partir do centro | Só **caixas**. O plano é perpendicular a um eixo **da própria caixa**, então uma caixa girada é cortada certo. As duas metades continuam caixas, com a mesma rotação, escala e cor. O plano precisa sobrar uma parte de cada lado (`EDITOR_CUT_PLANE_OUTSIDE`); a soma dos volumes é igual à da caixa original |
| **Arranjo em linha** | `brush.arrange_line` | `count` (2–64, contando o original), `offset` `{x,y,z}` em metros | Só **caixas**. O original fica; as cópias são criadas no mesmo `world.batch` e o passo se multiplica por 1, 2, 3… |
| **Arranjo em círculo** | `brush.arrange_circle` | `count` (2–64), `center` `{x,y,z}`, `axis` | Só **caixas**. O original fica na posição e é o item do ângulo zero; cada cópia gira em torno do eixo junto com o círculo. A janela calcula o centro a um raio de distância do objeto, no primeiro eixo perpendicular |
| **Espelhar** | `brush.mirror` | `axis`, `plane` (coordenada do plano no eixo), `duplicate` | Só **caixas**. Reflete a **posição** e a **rotação** (`M·R·M`), então a caixa continua encaixada. Com `duplicate` (padrão) cria uma cópia; sem ele, move o próprio objeto. A cópia duplicada leva traços, colisores e scripts |

**Interface.** As quatro entradas ficam em **Operações**, ligadas só quando há seleção:

| Entrada | Atalho |
|---|---|
| **Cortar com plano…** | `Ctrl+Alt+P` |
| **Arranjo em linha…** | `Ctrl+Alt+L` |
| **Arranjo em círculo…** | `Ctrl+Alt+O` |
| **Espelhar…** | `Ctrl+Alt+M` |

**Limites conhecidos:** cilindros, escadas e malhas ficam para depois nessas quatro operações (a mensagem de recusa diz qual comando usar); o arranjo **não** copia colisores, scripts nem traços — só o espelho duplicado copia; e não há prévia ao vivo, o resultado aparece ao confirmar a caixa de diálogo.

**Verificação (2026-11-08, display virtual):** na sala de exemplo, cortar uma caixa de 1 m no eixo local X gerou duas caixas de 0,5 m com o mesmo giro de 25° e a mesma soma de volume; a linha criou cópias a 1 m; o círculo de 8 itens fechou o anel com passos de 45° em torno do eixo Y; o espelho em X=0 criou a cópia em `x = -2,773` com giro `-25°`; **um** `Ctrl+Z` removeu cada operação inteira (as 7 cópias do círculo caíram juntas).

**Por que nível 2 primeiro:** entrega o que mais pesa em construção de mapa (redimensionar rápido, alinhar na grade) **sem** trazer 11 mil linhas. Todas essas ferramentas viram **comandos** (desfazem e viajam por rede).

## Nível 3: edição de malha (análise de 2026-10-24, antes da decisão)

> **Histórico.** Esta seção é a análise que levou à decisão; o que foi feito está em [Nível 3: o que foi feito](#nível-3-o-que-foi-feito-2026-10-25). Diferença em relação ao plano B abaixo: os **booleanos entraram** (com `manifold-rust`), porque você pediu.

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

## Nível 3: o que foi feito (2026-10-25)

**Decisão (sua, 2026-10-25):** trazer as operações e guardar a **malha pronta** no mundo (opção "A" da pergunta: a malha, não a lista de operações). Registro em [DR-22](../06-decisoes/registro-de-decisoes.md).

### Como a malha fica guardada

| Peça | Onde | O quê |
|---|---|---|
| Tabela `Mesh` | `petunia_project/src/standard.rs` | `geometry` (a malha) e `color` |
| Tipo de valor `mesh` | `petunia_world` e `petunia_math::PolygonMesh` | Vértices e faces (n-gons), validados como dado hostil: números finitos, índices que existem, limites |
| No arquivo da cena | texto | `"positions": ["0 0 0", …], "faces": ["0 1 2 3", …]`: **uma linha por vértice e por face**, então mover um vértice muda uma linha |
| Desfazer | histórico comum | Cada operação é **um** `row.set` (ou um `world.batch`); `Ctrl+Z` traz a malha anterior |

**Limites de uma malha:** 65.536 vértices, 65.536 faces, 256 cantos por face, 262.144 cantos ao todo.

**Duas proteções novas, por causa das malhas:**

- **A cena pode ter até 64 MB** (eram 4 MB), e **salvar recusa** uma cena maior que esse limite, deixando o arquivo antigo intacto. Antes, dava para salvar uma cena que depois **não abria** (o limite só valia ao abrir). Isso era um risco de perder trabalho, agora com teste.
- **O histórico de desfazer tem teto de memória** (256 MB, ajustável), além dos 500 passos. Cada passo de malha guarda a malha antiga; sem o teto, centenas de edições numa malha grande ocupariam gigabytes. O passo mais novo sempre fica.

### O que veio do Petunia3D

Crate novo `petunia_mesh`. O módulo `legacy/` é a cópia do Petunia3D (commit `dec467127a7f`), **quase sem mudanças**, registrada no [ATTRIBUTION.md](../../ATTRIBUTION.md). Por cima dele, uma API pequena (`lib.rs`) que:

1. converte a malha da engine para a do Petunia3D e de volta (os tipos do Petunia3D e o `glam` **nunca saem do crate**);
2. **valida antes** (malha, índices, distâncias finitas até 1000 m, contagens);
3. **valida depois** (o resultado precisa ser uma malha válida dentro dos limites);
4. **recusa com motivo** quando nada mudou. As funções do Petunia3D simplesmente não fazem nada quando a entrada não serve; aqui você recebe um código e uma sugestão.

| Operação | Função | Teste principal (o que confere) |
|---|---|---|
| Cortar, fundir, interseção | `boolean` (com `manifold-rust`) | volumes exatos: parede menos porta = 5 m³; A∪B e A∩B batem com a inclusão-exclusão; resultado fechado |
| Inset | `inset` | 10 faces, 12 vértices, sólido fechado, face de dentro a 0,75 m do centro |
| Extrudar | `extrude` | volume 12 m³ para fora, 6 m³ para dentro |
| Loop cut | `loop_cut` | 1 corte: 10 faces; 2 cortes: 14; termina limpo em triângulos e n-gons |
| Bevel | `bevel` | chanfro tira exatamente 0,04 m³; arredondado com 4 segmentos |
| Subdividir | `subdivide` | grade 2×2; **vizinhos sem rachadura** (correção nossa, ver abaixo) |
| Dissolver, fundir, apagar, virar | `dissolve_edges`, `merge_vertices`, `merge_by_distance`, `delete_faces`, `flip_faces` | contagens e volume |
| Faca | `knife` | uma face vira duas; sólido continua fechado |
| Brush → malha | `from_triangles` | a caixa vira 8 vértices e 6 quadriláteros (solda e junta triângulos coplanares) |

**Testes:** 17 da API nova, mais **164 testes do próprio Petunia3D**, que continuam passando dentro do crate. Isso mostra que a cópia se comporta igual à origem. Também há testes de **entrada hostil** (NaN, índices fora, listas vazias, distâncias absurdas: tudo recusado, sem pânico) e de **determinismo** (a mesma entrada, várias vezes, dá a mesma malha bit a bit).

**Desempenho (medido em 2026-10-25, build do editor):** numa malha de 6,5 mil vértices, o booleano leva 74 ms, o inset em todas as faces 29 ms e a checagem de sólido 3 ms. Dois ganhos vieram de medir: a junção dos vizinhos da subdivisão (código nosso) caiu de 1,6 s para 8 ms com uma grade espacial; a checagem de sólido, de 200 ms para 7 ms, com um teste direto das arestas, que é **conferido contra** a estrutura de meias-arestas do Petunia3D em 6 malhas. `opt-level = 3` em `petunia_mesh` e `manifold-rust`, também no build de desenvolvimento, levou o booleano de 164 ms para 30 ms.

**Correção nossa:** a subdivisão do Petunia3D deixa os **vizinhos** da face com a aresta antiga. A malha fica com "rachaduras" invisíveis (junções em T) e deixa de ser um sólido fechado, então um booleano depois disso seria recusado. A API agora insere os pontos novos nas arestas dos vizinhos.

### No editor

Módulo `petunia_editor/src/mesh_tool.rs`, operações `mesh.*` (lista no [editor](11-editor.md#operações--op----)). Pontos principais:

- **Tornar editável** mantém a entidade (nome, transform, scripts); só troca `Brush` por `Mesh`.
- **Booleanos** aceitam brushes e malhas, em qualquer rotação e escala: a ferramenta é levada para o espaço do alvo. Teste: um pilar girado 45° e esticado 2× dá exatamente o volume dele.
- **Partes da malha** (faces, arestas, vértices): `mesh.pick` diz o que está sob o mouse; `mesh.select` escolhe. A escolha **some** quando a malha muda (desfazer, outra operação), para nunca apontar para a face errada.
- **Desenho:** malhas entram no mesmo caminho dos brushes (`DrawItem` com `Shape::Mesh`), então seleção, gizmo, snap a superfície e andares já funcionam com elas. Os triângulos são guardados por "impressão digital" da malha, e o OpenGL apaga os buffers de malhas que saíram da cena.
- A malha selecionada mostra o **aramado**; faces, arestas e vértices escolhidos aparecem destacados.
- **Valor ao vivo:** `mesh.preview` mostra inset, extrudar, bevel e loop cut enquanto o mouse arrasta, sem gravar; soltar (`mesh.preview_end`) vira **um** desfazer. Cada passo parte da malha de antes do arrasto, então ir e voltar dá exatamente o mesmo resultado (teste).

### Limites conhecidos (nível 3)

| Limite | Efeito |
|---|---|
| **Interface parcial** | Inset, extrudar, chanfro, loop cut, subdividir, dissolver, fundir, apagar e virar já estão na barra **Malha**; booleanos, faca, valor ao vivo arrastando e inspetor de `Mesh` ainda não |
| Bevel recusa arestas que se encontram num canto | Limite do Petunia3D; chanfre uma aresta de cada vez |
| Sem UV nem material por face | Cor única por objeto. O módulo de UV (7 arquivos) ficou de fora |
| Booleanos só entre **sólidos fechados** | Malha aberta (depois de apagar faces) é recusada com motivo |
| Booleano com faces exatamente no mesmo plano | O `manifold-rust` resolve na maioria dos casos; quando falha, a mensagem pede para mover um pouco a ferramenta |
| Código copiado ainda usa nomes curtos (`n`, `f`, `idx`) e índices diretos | É legado (regra do `AGENTS.md`): migra quando for tocado. A API nova segue as regras |

### Ganchos da interface

> **Atualização (2026-11-04):** todos os ganchos abaixo estão ligados na janela. A tabela diz onde cada um vive e qual operação ele envia.

| Gancho | Onde está | Operação |
|---|---|---|
| Menu **Malha** | barra de menus da janela; a paleta `Ctrl+K` lê os menus, então as entradas aparecem nela sem trabalho a mais | todas as `mesh.*` |
| Ferramenta **Editar malha** | coluna de ferramentas do viewport, abaixo das ferramentas de escultura | `mesh.pick`, `mesh.select` |
| **Tornar editável** | menu **Malha** e inspetor de Brush | `mesh.convert` |
| **Booleano…** | menu **Malha**: caixa de diálogo com operação (fundir, cortar, interseção), objeto-ferramenta e "manter a ferramenta" | `mesh.boolean` |
| **Modo de edição de malha** (`Tab`) e modos face, aresta, vértice (`1`, `2`, `3`) | clique escolhe, `Shift` soma, `Ctrl` alterna | `mesh.pick` → `mesh.select` |
| Atalhos de modelagem | `I` inset, `Ctrl+E` extrudar, `Ctrl+R` loop cut, `Ctrl+B` bevel, `K` faca, `M` fundir, `X` apagar faces | `mesh.*` |
| Valor ao vivo | combo **Ao arrastar** na barra **Malha**: arrastar muda a distância (ou o deslize do loop cut); soltar aplica, `Esc` cancela, `Shift` deixa fino; o motivo de um valor inválido aparece na barra de status | `mesh.preview`, `mesh.preview_end` |
| Painel com números | barra **Malha**, segunda linha (os ícones ficam na primeira, que não caberia junto na largura do viewport): distância, cortes, segmentos e deslize do corte em laço | os campos de cada operação |
| Inspetor da tabela `Mesh` | mostra vértices, arestas, faces e se a malha é sólida ou aberta | `mesh.info` |

**Faca.** `K` arma a faca; o primeiro clique marca um ponto sobre uma aresta e o segundo corta a face entre os dois pontos. Uma linha tracejada acompanha o ponteiro entre os dois cliques. Os dois pontos precisam estar na mesma face, como no Petunia3D. `Esc` desarma.

**Partes escolhidas.** O valor ao vivo usa as faces (inserir, extrudar) ou arestas (chanfro, corte em laço) que já estão escolhidas. Arrastar sem nenhuma escolhida mostra *selecione uma face antes de arrastar* em vez de aplicar uma operação vazia; para escolher, ponha **Ao arrastar** em *Nada (clique escolhe)*.

**Nível 2 feito** (cortar, arranjar, espelhar, alças de canto e aresta). **Nível 3 de modelagem removido**; o corte de portas e janelas é o booleano.

## Riscos

| Risco | Efeito |
|---|---|
| Subconjunto B tem testes que dependem dos módulos excluídos | Parte dos testes não roda no destino; reescrever |
| Malha editável pede formato de arquivo novo | O formato de texto precisa guardar malhas (tamanho, diffs) |
| A interface de edição de componentes (vértice, aresta, face) não existe na engine | É trabalho novo, além da cópia do crate |

## Próximo passo sugerido

A blocagem (nível 2) e o booleano estão na interface. A edição de malha completa não volta sem uma decisão nova. A lista do que vem depois está em [estado da implementação](../05-roadmap/03-estado-da-implementacao.md).
