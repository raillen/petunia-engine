# Sistema de mundos — checklist de implementação

**Revisão:** 2026-10-27 · **Decisão:** [DR-33](../06-decisoes/registro-de-decisoes.md) · **Branch:** `feat/sistemas-de-mundo`

Este documento é a lista viva do que foi implementado e do que falta no sistema de mundos. Ele é atualizado a cada implementação, como pedido. O desenho técnico fica em [terreno, rios e estradas](../02-arquitetura/04-terreno-rios-estradas.md).

## Estado por sistema

| Sistema | Implementado | Falta | % |
|---|---|---:|---:|
| Terreno (dados) | `petunia_terrain`: chunks, amostras, materiais, furos, consultas, limites | — | 90% |
| Terreno (geração) | fBm determinístico próprio; parâmetros validados; **heightmap em arquivo** (`terrain.heights`, importar/exportar `.pthm`); **5 biomas** que escolhem relevo e camadas pela semente | — | 96% |
| Terreno (edição) | Brushes 3D, 4 falloffs, 6 operações, erosão térmica e hidráulica, espelhamento x/z, *stamp* de heightmap e de malha 3D | Brushes plano/caminho, máscaras | 92% |
| Terreno (render) | Malha por chunk, cores por camada (8), LOD 1/2/4, saias, furos, plano de corte, **culling por chunk** pelo volume de visão do viewport | Materiais por textura, verificação visual do LOD | 92% |
| Terreno (física) | Heightfield no Rapier; trimesh nos chunks com furo | Terreno com transformação não-identidade | 80% |
| Terreno (editor) | Pincel `terrain.begin/drag/end/cancel/set/preview`, anel, prévia ao vivo, um desfazer; menu **Criar > Terreno**, ferramenta **T**, barra de opções, **Gerar terreno**, **Raio X** em arame, **Corte** com **altura digitável** (sugerida pela caverna selecionada), **Exportar/Importar heightmap** e teste de fumaça repetível | Verificação visual do LOD | 97% |
| Cavernas (SDF) | `petunia_sculpt` (chunks 16³, surface nets, 5 modos, caminho, gerador, fundir, facetada) + espelhamento X, preencher, oco, ruído, **booleanos** (`sculpt.boolean`) e **carimbo de malha** (`sculpt.stamp_mesh`) | Malha sólida (o carimbo imprime a superfície) | 98% |
| Caverna ↔ terreno | Furos explícitos (`terrain.holes`), furos automáticos dos túneis (`terrain.holes_from_caves`), render sem as células, física trimesh | Costura visual, ajuste fino do raio do furo | 85% |
| `surface_follow` | Tabela `SurfaceFollow`, comando `surface.follow`, `surface.set` inverso, preferência, re-projeção, alinhamento à normal e **um desfazer** junto de `generate/brush/paint/erode/hydraulic/stamp` e do arrasto | UI dedicada (o inspetor genérico já edita a tabela) | 95% |
| Erosão | Térmica determinística e hidráulica por gotas determinísticas, com desfazer | Ajuste visual fino | 90% |
| Rios | — (outro agent) | Spline, escavação, malha de água, fluxo, espuma, confluências, lagos, quedas | 0% |
| Estradas | — (outro agent) | Grafo, perfis, cruzamentos, aterro/corte, pontes, decals, navegação | 0% |
| Spline/sweep | `petunia_curve`: Bézier cúbica com handles absolutos, quadros por transporte paralelo, perfis (faixa, calha, tubo), varredura em malha e comando `spline.sweep` | Verificação visual pela janela (desenho do caminho) | 70% |
| Água (render) | — (outro agent) | Flow map, Gerstner 4–8, refração, espuma | 0% |

## Implementado

### 2026-10-29 — spline e varredura

| Item | Evidência |
|---|---|
| **Crate `petunia_curve`** (domínio puro, sem GPU): `Spline` de Bézier cúbica com **handles absolutos**, `Spline::through` (handles derivados dos vizinhos, forma Catmull-Rom), aberta ou fechada, com limite de 256 pontos | **22 testes**: pontas iguais ao primeiro/último ponto e entrada fora do intervalo presa no intervalo, anel fechado, comprimento de 20 m numa reta, handles absolutos e canto sem handle, recusa de 0/1 ponto, de 257 pontos e de coordenada não finita |
| **Quadros por transporte paralelo**: `normal` projetada do quadro anterior sobre o plano do novo tangente, com eixo de reserva quando a curva sobe na vertical; `frames(n)` distribui os anéis por **distância igual** (tabela de comprimento de arco) | 3 testes: ortonormalidade e espaçamento de 5 m em 5 anéis de 20 m, produto escalar entre `binormal` vizinhos > 0,8 numa curva em S (não capota), curva vertical ainda tem quadro; curva sem direção volta erro |
| **Perfis**: `Profile` de 2 a 64 vértices em metros (`x` para a direita, `y` para cima); `ribbon` (com ou sem espessura), `trough` (calha com margens), `tube` (anel de `sides` vértices); perfil fechado é **normalizado para anti-horário**, senão o tubo sairia do avesso | 3 testes: tamanhos, recusa de largura/raio/profundidade zero e de lado de tubo abaixo de 3, inversão do sentido horário |
| **Varredura**: anéis por `SweepSettings { steps, cap_ends }`, quadriláteros entre anéis, tampas só em perfil fechado, `PolygonMesh` validada pelas regras do renderizador | 6 testes: faixa de 4 m × 20 m plana (5 anéis, 4 faces), **faixa virada para cima** e **tubo virado para fora**, tampas (3 × 8 + 2 faces), calha afundando 1,5 m, determinismo byte a byte, 1 anel sem faces e recusa de 0/513 passos |
| **Comando `spline.sweep`**: recebe `points`, `closed`, `profile`, `width`, `depth`, `thickness`, `sides`, `steps`, `cap_ends`, grava a malha na coluna `geometry` do objeto e devolve `{vertices, faces, steps}`; **um** desfazer (`row.set` da geometria anterior) | **4 testes de comando**: registrado no schema, varredura grava 10 vértices/4 faces e o desfazer volta a malha vazia, caminho/perfil/passos inválidos recusados sem tocar no objeto, objeto sem tabela `Mesh` recusado com `WORLD_ROW_MISSING` |

**O que ainda não existe:** o desenho do caminho na janela (clicar pontos no viewport), a UI de perfil (largura/profundidade na barra) e o efeito no terreno (escorvar leito, aplainar aterro). Rios e estradas se apoiam nesta peça, mas nenhum dos dois começou.

### 2026-10-29 — biomas

| Item | Evidência |
|---|---|
| **`Biome`** é dado puro: relevo (tamanho da feição, amplitude, oitavas, persistência, altura base) e as faixas de camada que a altura pinta | **6 testes de crate**: nomes que vão e voltam (incluindo `planície`), a mesma semente escolhe o mesmo bioma, 80 sementes alcançam os 5, o relevo da neve é mais alto e mais estreito que o da planície, as faixas de camada (incluindo amplitude zero e altura não finita) |
| **A semente escolhe o bioma** (`Biome::from_seed`), com mistura do valor de ruído, então sementes vizinhas não caem no mesmo bioma | teste acima |
| **`terrain.generate` recebe `biome`** (`plains`, `forest`, `desert`, `snow`, `volcano` ou `none`); sem o campo, a semente escolhe; nome desconhecido é recusado **sem tocar no terreno**; a resposta traz o bioma usado | **2 testes de comando**: gera deserto (só areia e rocha), a mesma semente devolve o mesmo bioma nas duas chamadas, `none` deixa tudo na primeira camada; `tundra` volta erro nomeando o valor, e o terreno fica igual |
| **Relevo do bioma, com o payload por cima**: os parâmetros de ruído passaram a opcionais, então a amplitude que um script manda continua vencendo a do bioma | testes acima (`amplitude: 0` + `biome: none` = chão plano) |
| **Na janela**: Operações > Gerar terreno abre a lista de biomas (**Pela semente**, Planície, Floresta, Deserto, Neve, Vulcão) e a barra de estado diz qual foi usado | **teste de fumaça 12/12** com o diálogo e capturas no display virtual: deserto em areia e neve com dunas altas, a mesma cena do projeto de exemplo |
| Nada de terceiros: o ruído continua sendo o nosso, e a escolha do bioma também | código |

**O que a captura mostra:** no deserto o chão é areia (0,78; 0,70; 0,50) e praticamente plano, como o relevo de 3,5 m de amplitude pede; na neve o mesmo comando gera dunas de 22 m de amplitude e superfície quase branca. As duas capturas foram feitas com a mesma câmera e o mesmo projeto de exemplo — muda o bioma escolhido na lista.

**Limites:** o bioma é escolhido **na geração**; trocar o bioma de um terreno já esculpido é gerar de novo (que desfaz escultura, com um passo de desfazer). Não há transição entre biomas num mesmo mundo — o relevo e as camadas são função da altura, sem variação horizontal.

### 2026-10-29 — culling por chunk

| Item | Evidência |
|---|---|
| **Volume de visão** (`Frustum`, 6 planos extraídos da matriz visão-projeção) com teste de esfera e de caixa | **7 testes**: esfera na frente visível, atrás/ao lado/além do plano distante escondida, esfera enorme em volta da câmera visível, caixa seguindo a esfera e matriz degenerada que não recusa nada |
| **Limites por bloco** incluindo as amostras da borda compartilhada (a malha lê o vizinho) e a saia do LOD mais grosso | **1 teste**: a esfera contém todos os vértices do LOD 4 (a saia mais funda) em dois blocos vizinhos com relevo diferente |
| **O quadro passa a receber o viewport**: `extract_frame_in(viewport)` monta o volume com a proporção real e **não malha** o bloco fora dele; `session.frame_in(viewport)` e o FFI usam a largura/altura do viewport | teste do editor: bloco a 320 m na diagonal **não é construído** quando a câmera olha para o lado oposto (0 itens) e é construído quando olha para ele (1 item); `frame()` sem viewport (16:9) também recorta |
| **Medida**: terreno de 25 blocos (5 × 5), câmera do projeto de exemplo no horizonte | **11 blocos construídos** de 25 (14 descartados); olhando para baixo, 25, porque a esfera de cada bloco toca a câmera |
| Sem buraco na imagem | capturas no display virtual com terreno gerado: superfície contínua, sem bloco faltando; teste de fumaça **12/12** |

O recorte é **conservador**: a esfera de cada bloco inclui a saia mais funda e a borda do vizinho, então um bloco visível nunca é descartado (pode sobrar bloco que não aparece). Ele não reduz o custo ao olhar para baixo, quando os blocos realmente estão em volta da câmera.

### 2026-10-29 — heightmap em arquivo e o corte que mira a caverna

| Item | Evidência |
|---|---|
| **Formato `.pthm`** no crate: 28 bytes de cabeçalho (`PTHM`, versão, largura, profundidade, tamanho da célula, origem) e um `f32` por amostra, em metros | **8 testes de crate**: ida e volta exata, origem e célula, arquivo de outro programa, versão errada, arquivo cortado ou com bytes sobrando, tamanhos absurdos, altura não finita e limites de construção |
| **Exportação** da área do terreno (ou a largura/profundidade pedidas) para o arquivo | teste de comando (`terrain.heights`: 17 × 17 amostras no terreno inteiro, recorte 2 × 3, pedido de 9000 limitado a 2048); **visto na janela**: barra de estado "Heightmap exportado: 49 × 49 amostras" |
| **Importação** com **um desfazer**, usando `terrain.stamp` por baixo | teste do editor: exporta, aplana, importa e as 16 amostras voltam iguais; desfazer volta ao chão plano |
| **Consulta `terrain.heights`** (`entity`, `x`, `z`, `width`, `depth`) para tirar uma grade sem gravar arquivo | teste de comando (acima) |
| Operações > **Exportar heightmap…** e **Importar heightmap…** com diálogo de arquivo `.pthm` | **visto em captura** no display virtual: o menu mostra os dois itens, e cada um escreve na barra de estado o resultado |
| **Altura do corte sugere a caverna**: sem altura digitada, o plano vai para o meio vertical da caverna selecionada (casco e traços, com a transformação aplicada), e só depois para o cursor do pincel | teste de render: limites do volume e centro com deslocamento e com escala; teste do editor: corte em 99 m desce para o meio da caverna criada, e o quadro usa a mesma altura |

**Ida e volta pela janela:** exportei `a.pthm` (9632 bytes: 49 × 49 amostras de 1 m, origem −16,−16, alturas de −6,46 a 1,15 m), importei o mesmo arquivo e exportei de novo para `b.pthm`: **os dois arquivos são byte a byte iguais** (`cmp`). O caminho janela → FFI → protocolo → arquivo está fechado; a diferença visual entre as duas capturas daquela rodada foi a câmera, movida por teclas minhas que caíram no viewport em vez do diálogo — não o terreno.

**Limites do formato:** 2048 amostras por lado e 4 milhões no total (16 MB); altura fora de ±10000 m é recusada; amostras sem bloco valem 0 m; a importação só escreve onde já existe bloco. O caminho tem de terminar em `.pthm`, para um caminho errado não sobrescrever outro arquivo.

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
| 1 | Biomas na geração | heightmap em arquivo (feito) | uma semente escolher um bioma e ele aparecer na cor e na forma |
| 2 | Materiais por textura no terreno | atlas / assets de textura | a superfície trocar de textura por camada, não só de cor |
| 3 | Rios e estradas | spline/varredura | — |

Itens concluídos nesta rodada: **altura do corte sugerida pela caverna**, **heightmap em arquivo** (`terrain.heights`, exportar e importar com um desfazer), **culling por chunk** (11 de 25 blocos num mundo 5 × 5) e **teste de fumaça 12/12**.

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
| Heightmap grande | **não medido**: o teste usa 4 × 4 amostras e a janela, 49 × 49; o limite de 2048 × 2048 não foi exportado nem importado de verdade |
| Importar heightmap por cima de cavernas | **não testado**: o carimbo muda alturas e deixa os furos como estão |

## Fora desta fase (declarado)

LOD de SDF, malhagem na GPU, SDF para o mapa inteiro, edição de caverna em primeira pessoa, *streaming* de mundo infinito e navmesh automática.
