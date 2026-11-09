# Primitivas e edição de malha para construir mapas

**Revisão:** 2026-10-27 · **Fonte:** leitura e medição de `petunia3D/crates/mesh` (22,9 mil linhas) e do código da engine (`LOCAL`). Decisão relacionada: [DR-22](../06-decisoes/registro-de-decisoes.md).

## Resumo (TL;DR)

Há **três níveis** de edição para construir mapas. **Os três já existem no motor.** O nível 3 (booleanos, inset, loop cut, bevel e o resto) chegou em 2026-10-25 e tem teste; a **interface** chegou em 2026-10-27 (barra **Malha**, vértices visíveis e atalhos). O **valor ao vivo** (segure `I`, `Ctrl+E` ou `Ctrl+B` e arraste) e a **faca clicando arestas** (`K`) chegaram em **2026-10-30**, junto com as contagens no inspetor: **nenhum gancho de interface
falta**. Veja [o que a interface liga](#o-que-a-interface-liga-e-o-que-ainda-falta).

| Nível | O que é | Estado | Custo |
|---|---|---|---|
| **1. Brushes paramétricos** | Formas com tamanho e detalhe, movidas com gizmo e snap | **Feito** | já pago |
| **2. Edição de brushes** | Empurrar/puxar, extrudar, cavar, tornar oco | **Feito** em parte (falta cortar com plano, arranjo, espelhar) | sem dependência nova |
| **3. Edição de malha** | Booleanos, inset, extrudar, loop cut, bevel, subdividir, dissolver, fundir, faca | **Feito no motor** (2026-10-25); interface pendente | 14,1 mil linhas copiadas (legado; 10,9 mil sem os testes) + ~3,3 mil novas; 2 dependências diretas |

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

### O que a interface liga (e o que ainda falta)

> **Atualização (2026-10-30):** a barra **Malha** liga as operações de faces, arestas e vértices, com os campos **Distância**, **Cortes** e **Segmentos** ao lado; o modo vértice mostra um ponto em cada vértice. Entre **2026-10-29 e 2026-10-30** entraram o menu **Malha ▸ Booleano…**, **Malha ▸ Informações da malha**, os atalhos de modelagem, o **valor ao vivo** e a **faca clicando arestas**.

| Gancho | Como ficou |
|---|---|
| **Valor ao vivo** | Segure `I` (inserir), `Ctrl+E` (extrudar), `Ctrl+B` (chanfro) ou `Ctrl+R` (corte em laço): o resultado aparece na hora; mover o mouse muda o valor (1 cm por pixel, de 0,01 a 10 m; no corte em laço, 40 px por corte, de 1 a 32); **soltar a tecla, clicar ou `Enter` aplica** como **um** passo de desfazer; `Esc` cancela. Uma batida rápida aplica o valor da barra, como antes |
| **Faca por clique** | `K` (ou **Malha ▸ Faca…**) arma a faca; cada clique escolhe o ponto na aresta mais próxima (o `mesh.pick` dá a aresta e a fração); o segundo clique corta. `Esc` cancela. **Malha ▸ Faca por números…** continua para quem digita as arestas |
| **Booleano e informações** | **Malha ▸ Booleano…** escolhe a ferramenta, a operação e "manter a ferramenta"; **Malha ▸ Informações da malha** mostra vértices, arestas, faces, triângulos e se o sólido é fechado |
| **Contagens no inspetor** | a seção `Mesh` mostra `8 vértices · 12 arestas · 6 faces` em vez de "malha editável": a conta é feita sobre o texto guardado, sem ida ao motor, e acompanha toda edição |

**Nenhum gancho de interface falta nesta versão.** O que continua fora é de outra ordem:
**atributos por face e coordenadas de textura** exigem guardar mais por face do que o texto atual
(`positions` + `faces`) e precisam de decisão própria.

## Riscos

| Risco | Efeito |
|---|---|
| Subconjunto B tem testes que dependem dos módulos excluídos | Parte dos testes não roda no destino; reescrever |
| Malha editável pede formato de arquivo novo | O formato de texto precisa guardar malhas (tamanho, diffs) |
| A interface de edição de componentes (vértice, aresta, face) não existe na engine | É trabalho novo, além da cópia do crate |

## Próximo passo sugerido

Os ganchos de interface estão fechados ([o que a interface liga](#o-que-a-interface-liga-e-o-que-ainda-falta)).
O próximo passo é de **formato**, não de janela: **material por face e coordenadas de textura**
exigem guardar mais por face do que o texto atual (`positions` + `faces`) e precisam de decisão
própria; depois disso, o arrasto de valor do `loop_cut` (hoje a tecla aplica na hora).
