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
| Terreno (editor) | Pincel `terrain.begin/drag/end/cancel/set/preview`, anel, prévia ao vivo, um desfazer; menu **Criar > Terreno**, ferramenta **T**, barra de opções, **Gerar terreno**, **Raio X** em arame, **Corte** com **altura digitável** e teste de fumaça repetível | Sugestão automática da altura (caverna selecionada); verificação visual do LOD | 94% |
| Cavernas (SDF) | `petunia_sculpt` (chunks 16³, surface nets, 5 modos, caminho, gerador, fundir, facetada) + espelhamento X, preencher, oco, ruído, **booleanos** (`sculpt.boolean`) e **carimbo de malha** (`sculpt.stamp_mesh`) | Malha sólida (o carimbo imprime a superfície) | 98% |
| Caverna ↔ terreno | Furos explícitos (`terrain.holes`), furos automáticos dos túneis (`terrain.holes_from_caves`), render sem as células, física trimesh | Costura visual, ajuste fino do raio do furo | 85% |
| `surface_follow` | Tabela `SurfaceFollow`, comando `surface.follow`, `surface.set` inverso, preferência, re-projeção, alinhamento à normal e **um desfazer** junto de `generate/brush/paint/erode/hydraulic/stamp` e do arrasto | UI dedicada (o inspetor genérico já edita a tabela) | 95% |
| Erosão | Térmica determinística e hidráulica por gotas determinísticas, com desfazer | Ajuste visual fino | 90% |
| Rios | — (outro agent) | Spline, escavação, malha de água, fluxo, espuma, confluências, lagos, quedas | 0% |
| Estradas | — (outro agent) | Grafo, perfis, cruzamentos, aterro/corte, pontes, decals, navegação | 0% |
| Spline/sweep | — (outro agent) | Portar Bézier + varredura do Petunia3D | 0% |
| Água (render) | — (outro agent) | Flow map, Gerstner 4–8, refração, espuma | 0% |

## Implementado

### 2026-10-29 — altura do corte na barra e a caverna vista por dentro

| Item | Evidência |
|---|---|
| **Campo de altura do plano de corte** na barra do terreno (`y … m`), ao lado da caixa **Corte**: digitar a altura, confirmar com Enter, aplica `terrain.section` com `height` | compila no Qt 6.4.2; **visto em captura**: com 5,50 m o teto da caverna desaparece |
| Altura digitada passa a mandar no plano; **sem digitar nada**, a caixa **Corte** mantém o comportamento antigo (altura do cursor do pincel) | código da janela; captura com 5,50 m e comparação com o corte no cursor |
| O campo mostra a altura em vigor depois de cada comando (`section_height` no estado do terreno) | captura: o campo exibe `y 5,50 m` depois de digitar |

Capturas no display virtual (janela 1880×1120; `Caverna aleatória` com 46 traços, terreno gerado, câmera na maior sala, em (2,6; 3,8; −17,0) olhando para o centro da caverna):

| Cena | O que se vê |
|---|---|
| Interior **sem** os sinalizadores (`final/1-chamber.png`) | a rocha da caverna em volta, a sala e o gizmo do objeto; nada do mundo externo |
| **Raio X + Corte em 5,50 m** (`final/2-xray-cut-550.png`) | o **teto some**, o céu entra pela abertura e as paredes de rocha continuam abaixo do plano: é a leitura que faltava |
| **Raio X + Corte** na altura do cursor (0 m) | o mundo inteiro acima de 0 é descartado: sobram a grade e o arame do terreno, sem caverna |

Arquivos: `/tmp/petunia-cave/final/`, mesma cena nos dois casos.

**Achado — o que o raio X não faz por dentro:** com a câmera **dentro** da caverna o Raio X do terreno **não muda a imagem**. A superfície que tapa a vista é a **malha da caverna**, opaca e fora do raio X (que só age nos blocos `Shape::Terrain`); o terreno, visto de dentro do próprio sólido, não desenha as faces viradas para fora. A identificação foi feita **mudando a cor da caverna para magenta** na cena: as paredes em volta mudam de cor e o terreno não aparece. Para o raio X acrescentar algo é preciso olhar a caverna **de fora**, com o corte abrindo o terreno — que é o caso da captura do dia anterior.

**Limite conhecido:** com o foco ainda na caixa **Corte**, a captura mostra o contorno de foco sem o ✓, embora o corte esteja ligado (o estado volta a aparecer quando o foco sai). É pintura do tema, não estado: o desenho da cena e o campo de altura batem com o estado do motor.

### 2026-10-28 — caverna vista por dentro: o que a captura mostra

Capturas no display virtual (janela 1880×1120, `Caverna aleatória`, terreno gerado, câmera em y = 4,0, z = -12,2):

| Cena | O que se vê |
|---|---|
| Câmera **dentro** da caverna, sem os sinalizadores | as paredes de rocha em volta e a passagem adiante; o interior renderiza e é legível |
| **Raio X + Corte** ligados, corte na altura do cursor do terreno (≈0,5 m) | o terreno vira arame e **a caverna some inteira**: o plano descarta tudo acima da altura dele e a caverna vive de 0 a 8 m |
| **Raio X + Corte**, corte alto (monte com força 8) | o arame do terreno aparece; a caverna continua sem leitura, porque a altura do monte não cai dentro dela |

Arquivos: `/tmp/petunia-shot-smoke/cave-inside.png` (interior) e `/tmp/petunia-shot-smoke/cave-inside-xray-cut.png` (os dois sinalizadores ligados, caverna fora do quadro).

**Achado:** o plano de corte usa **sempre a altura do cursor do pincel de terreno** (`terrain.section` sem `height` cai no `terrain.cursor`, e a caixa **Corte** não manda altura). Não existe hoje, na janela, como escolher a altura do plano. Por isso o par “arame + corte” ainda **não** serve para olhar dentro de uma caverna: ou corta abaixo do piso dela (e ela desaparece) ou corta na altura de um monte (que não é onde a caverna está).

Consequência para a fila: antes de prometer “ver o interior da caverna pelo corte”, a janela precisa de uma **altura de corte escolhível** (campo numérico ou arrastar o plano), com a caverna selecionada como sugestão de altura. **Resolvido em 2026-10-29** com o campo `y … m` na barra (seção acima).

### 2026-10-28 — raio X em arame e teste de fumaça repetível

| Item | Evidência |
|---|---|
| **Arame no raio X**: o terreno translúcido (35 %) ganha as arestas desenhadas dos dois lados (`DrawItem::wireframe`), então a superfície continua legível e o interior aparece | teste do editor: os blocos de terreno ficam em arame com alfa < 1 e voltam a sólidos ao desligar; **visto em captura** no display virtual |
| **Arestas em duas passadas** no adaptador OpenGL: preenchimento translúcido, depois linhas de 90 % de opacidade, restaurando `FILL` e `CULL_FACE` | compila no Qt 6.4.2; visto na mesma captura |
| **Teste de fumaça repetível**: `12/12` em **sete** execuções seguidas | evidência abaixo |
| Menu do `≡` com o primeiro item ativo ao abrir por atalho: as setas e o Enter passam a funcionar sem mouse | teste de fumaça: **Operações > Gerar terreno** por `Alt+O` + `Return` |
| `Down` no campo de busca da hierarquia seleciona o primeiro objeto visível e leva o foco para a árvore | teste de fumaça: escolha por nome antes do modo de edição |

**Teste de fumaça, sete execuções seguidas:**

| Rodada | Resultado |
|---|---|
| 1 a 7 | **12 de 12**, código de saída 0 |

As três causas da instabilidade eram do harness e da janela, não de tempo de espera: display fixo `:97` colidindo com sobra de outra sessão, clique cego no `+` antes do workspace assentar (agora espera o diálogo pelo nome, ancorado) e `xdotool search` casando com o *tooltip* do botão. Agora o display é escolhido (primeiro livre), a janela é esperada pelo título e o menu abre com item ativo.

### 2026-10-28 — ferramentas avançadas e automação

| Item | Evidência |
|---|---|
| `terrain.stamp`: carimba um retângulo de alturas (replace/add/subtract/min/max) | 4 testes de crate + 1 de comando |
| `terrain.erode_hydraulic`: gotas determinísticas, com desfazer | 3 testes de crate + 1 de comando |
| `terrain.holes_from_caves`: abre furos onde os túneis chegam à superfície | 1 teste de comando; ação **Operações > Furos das cavernas** na janela |
| `surface.set` + `surface.follow` automático: terreno e objetos em **um** desfazer | teste de comando e de editor |
| **Plano de corte** no renderizador (`section_height` no frame e no shader) e na barra do terreno | teste de sessão; compila |
| **Espelhamento X** no pincel de caverna (`sculpt.mirror`) | teste de editor: traço e espelho em um desfazer |
| Checkboxes **Espelhar X/Z**, **Raio X** e **Corte** na barra do terreno | **vistos em captura** no display virtual (a barra só os mostra por inteiro com a janela larga; em 1440 px eles caem no menu de estouro) |
| **Preencher**, **Oco** e **Ruído** em cavernas (`sculpt.fill/hollow/noise`), com botões na barra | teste de editor: cada um é um desfazer |
| **Carimbo de malha 3D** (`terrain.stamp_mesh`): a superfície de uma malha vira altura do terreno, com transformação | 1 teste de comando |
| **Booleano de cavernas** (`sculpt.boolean`): assa as malhas das cavernas, roda `petunia_mesh::boolean` (união, subtração, interseção) e remove/mantém originais em um desfazer | 1 teste de editor; ação **Operações > Booleano de cavernas…** na janela com diálogo de seleção |
| **Carimbo de malha na caverna** (`sculpt.stamp_mesh`): imprime a superfície de um objeto com malha no campo SDF (cavar ou pôr rocha atrás de cada face), com a transformação do objeto assada no espaço da caverna; um desfazer e nova tabela `SculptStamp` | 3 testes de distância ponto-triângulo + 1 de forma no crate (`a_stamp_carves_the_side_its_face_looks_away_from`), 1 teste de editor (forma + desfazer); ação **Operações > Carimbar malha na caverna…** |

### 2026-10-28 — janela Qt do terreno

| Item | Evidência |
|---|---|
| **Criar > Terreno** cria um heightmap vazio e seleciona a ferramenta | compila; a ação aparece no menu e na paleta |
| Ferramenta **Esculpir terreno** (`T`) com arrasto ao vivo, anel e cancelamento | teste de protocolo (`terrain_tool`) e compilação da janela |
| Barra de opções: operação, forma, falloff, raio, força, material, pintar, espelhar X/Z e raio X | compila; **vista em display virtual** numa execução do teste de fumaça |
| **Gerar terreno** (Operações e paleta): cria o terreno se preciso e gera 3×3 blocos | verificação do teste de fumaça: **9 blocos** numa execução |
| **Raio X**: o terreno fica translúcido e em arame enquanto a ferramenta está ativa | `terrain.xray` no estado; **visto em captura** |
| `terrain.hover` atualiza o anel sem arrastar | teste do editor |
| Teste de fumaça do Qt | **12/12 em sete execuções seguidas** |

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
| 1 | Altura do plano de corte escolhível na janela (hoje só a do cursor do terreno) | — | o corte atravessar uma caverna sem subir um monte |
| 2 | Rios e estradas | outro agent | — |

Itens concluídos nesta rodada: **teste de fumaça estável** (12/12 em quatro execuções) e **arame no raio X** (captura no display virtual).

## Não verificado (honesto)

| Item | Estado |
|---|---|
| Terreno visível na janela Qt | **visto em display virtual** numa execução do teste de fumaça (9 blocos gerados); a aparência e o LOD não foram julgados |
| Ferramenta de terreno pela janela | **parcial**: o teste de fumaça gerou o terreno; o arrasto não foi confirmado por captura |
| LOD sem costura visível | **não visto**; as saias existem, mas não foram olhadas em imagem |
| Cores das 8 camadas | **não vistas**; os testes conferem contagem e furos, não a aparência |
| Personagem andando no heightfield pela janela | **não rodou**; o teste de física prova a colisão, não o jogo |
| Raio X | **visto em captura**: superfície a 35 % com arestas |
| Caverna por dentro com Raio X **e** Corte | **não obtido**: com o corte na altura do cursor do terreno a caverna some inteira; a captura boa do interior é sem os dois sinalizadores (ver a seção acima) |
| Teste de fumaça | **estável** em sete execuções seguidas (12/12); uma execução isolada em máquina carregada ainda pode falhar por tempo |
| Erosão em terreno grande | **não medida**; o teste usa 8×8 |
| Carimbo de malha na caverna | **sem lugar sólido**: imprime a superfície numa banda (2 células); uma malha fechada vira casca, não volume; limite de 4096 triângulos e 8 carimbos por caverna; **não olhado em imagem** |
| Terreno com `Transform` deslocado | **não suportado**; os dados são do mundo e o collider assume origem |

## Fora desta fase (declarado)

LOD de SDF, malhagem na GPU, SDF para o mapa inteiro, edição de caverna em primeira pessoa, *streaming* de mundo infinito e navmesh automática.
