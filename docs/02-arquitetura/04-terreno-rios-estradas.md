# Terreno, rios e estradas

**Revisão:** 2026-10-27 (DR-33) · **Estado:** decisão aceita; implementação começou pelo terreno

> **Decisão (DR-33):** o exterior é **heightmap em chunks**; cavernas e volumes orgânicos são **SDF 3D esparso em chunks** (`petunia_sculpt` já é a base). Dois estilos de mapa coexistem: **sólido/CSG estilo Doom/Quake** (brushes convexos + booleanos do `petunia_mesh`) e **orgânico** (terreno esculpido + SDF). Brushes 2D e 3D coexistem. Escavar cavernas sem entrar usa **raio X + prévia fantasma**. `surface_follow` é opt-in por objeto, com padrão configurável nas preferências.

## Resumo (TL;DR)

Terreno, rios e estradas são **a mesma família**: curvas (splines) + um perfil + regras que **modificam o terreno** ao redor. Cavernas entram como **volumes SDF** que cortam o heightmap onde abrem para a superfície.

O Petunia3D já tem as peças centrais (`LOCAL`): spline Bézier com quadros por ponto, varredura de perfil ao longo de um caminho e "pontos presos a superfície". A engine reaproveita essas peças e acrescenta **redes**: estradas se ligam em cruzamentos, rios se juntam em confluências.

## O que existe (2026-10-28)

| Parte | Onde | O que faz |
|---|---|---|
| Dados e edição | `crates/petunia_terrain` | Heightmap em chunks, brushes 3D (esfera, caixa, cápsula, cilindro), 4 falloffs, subir/baixar/suavizar/aplainar/ruído/terraço, erosão térmica, furos, geração fBm determinística |
| Armazenamento | `ValueKind::Terrain` + tabela `Terrain` | O terreno inteiro é um valor da cena, com alturas, materiais e furos; uma linha por cena |
| Comandos | `terrain.*` e `surface.follow` | `create`, `generate`, `brush` (com `mirror_x`/`mirror_z`), `paint`, `patch`, `holes`, `erode`, `surface.follow`; todos com desfazer |
| Consultas | `terrain.height`, `terrain.info` | Altura e normal numa posição; resumo de blocos e amostras |
| Render | `crates/petunia_render::terrain` | Malha por chunk, 8 cores de camada, LOD 1/2/4, saias e furos; o adaptador OpenGL usa cor por vértice |
| Física | `petunia_physics::scene` | Heightfield do Rapier por chunk; trimesh no chunk com furo (para o personagem entrar na caverna) |
| Editor | `crates/petunia_editor::terrain_tool` | Pincel com prévia ao vivo, anel, `begin/drag/end/cancel/set/preview`, um desfazer; `SurfaceFollow` re-projeta os objetos |
| Preferência | `preferences.surface_follow` | Novo objeto nasce com `SurfaceFollow` quando ligada |

## Modelo híbrido: heightfield + SDF (DR-33)

| Representação | Onde | Por quê |
|---|---|---|
| **Heightmap em chunks** | Exterior (colinas, vales, planícies) | Memória baixa, física barata (heightfield do Rapier), LOD simples |
| **SDF 3D esparso em chunks** | Cavernas, túneis, volumes orgânicos | Overhangs e túneis que o heightmap não representa; só os blocos tocados existem |
| **Brushes convexos + CSG** | Arquitetura estilo Doom/Quake | Edição em grade, rápida e previsível; booleanos já existem |

Regras de integração:

1. A caverna ancora no terreno ao ser criada (usa a altura do terreno como base).
2. Onde a caverna abre para cima, o terreno é **suprimido** (região de furo) e a malha da caverna preenche; *skirts* escondem a costura.
3. Em zona de caverna a colisão usa a malha da caverna; fora dela, o heightfield. O Rapier aceita vários colisores.
4. Chunks do terreno e blocos da caverna compartilham a mesma grade espacial para *culling* e streaming.
5. **SDF completo para o mapa inteiro está fora**: a 1 m de resolução, 1 km × 1 km × 200 m pede ~200 milhões de células (~800 MB) e malhagem/colisão proporcionais. O híbrido evita isso.

### Brushes 2D e 3D (coexistem)

| Brush | Onde atua | Operação |
|---|---|---|
| Pegada 2D (círculo/elipse) | Heightmap | Soma/subtrai altura com falloff |
| Esfera, cápsula, caixa, cilindro, cone | SDF e heightmap (projetado) | Soma/subtrai distância; no heightmap, empurra a superfície |
| Plano (*half-space*) | SDF | Corta em linha reta (falésia, muro) |
| *Stamp* de malha | SDF e heightmap | Imprime uma forma 3D |
| Caminho (spline) | SDF | Túnel por caminho (já existe em `petunia_sculpt`) |

Falloff: radial ou **anisotrópico**; perfis linear, *smoothstep*, gaussiano e curva configurável.

### Escavar sem entrar (raio X)

O brush vive no **espaço do mundo**, então não depende da câmera. No modo de edição:

| Recurso | Efeito |
|---|---|
| Prévia fantasma | Volume do brush translúcido + superfície resultante em cor de prévia; **nada é gravado até soltar/Enter** |
| Raio X | Terreno em volta a 30 % de opacidade ou arame na região do brush |
| Plano de corte | Secção opcional revela o interior sem alterar a superfície |
| Máscara de profundidade | Mostra por onde o pincel está passando |

### Objetos que acompanham a superfície (`surface_follow`)

| Coluna | Tipo | Efeito |
|---|---|---|
| `surface_follow` | Bool | Re-projeta o objeto ao editar terreno/caverna (opt-in no inspetor) |
| `surface_offset` | Float | Altura acima da superfície |
| `surface_align_normal` | Bool | Alinha a rotação ao normal da superfície |

O padrão é configurável no menu de configurações da engine. Corpos dinâmicos não seguem. A re-projeção percorre só os chunks do volume editado, em ordem fixa por handle.

## Ferramentas de terreno (DR-33)

| Ferramenta | O que faz | Prioridade |
|---|---|---|
| Subir/baixar | Altura com falloff | Alta |
| Suavizar | Média dos vizinhos | Alta |
| Aplainar | Plano do primeiro toque | Alta |
| Ruído | Detalhe procedural | Alta |
| Terraço | Degraus em encostas | Média |
| Erosão térmica | Desliza material de encostas | Feito (`terrain.erode`) |
| Erosão hidráulica | Água escava canais | Feito (`terrain.erode_hydraulic`) |
| *Stamp* | Imprime malha 3D ou heightmap | Feito (`terrain.stamp`, `terrain.stamp_mesh`) |
| Furo | Abre para caverna/rio | Alta |
| Rampa | Rampa entre dois pontos | Média |
| Altura exata | Valor numérico (sem mouse) | Acessibilidade |
| Importar/exportar heightmap | RAW/PNG | Média |
| Espelhar | Edição simétrica | Média |
| Camadas (splatmap) | Pintar grama/terra/pedra/lama | Alta |
| Textura automática | Por inclinação/altura | Média |

## Ferramentas de caverna (DR-33)

| Ferramenta | O que faz | Estado |
|---|---|---|
| Cavar/adicionar | Soma/subtrai rocha | Feito |
| Inflar/desinflar | Empurra a superfície | Feito |
| Suavizar | Borra o campo | Feito |
| Nivelar | Plano | Feito |
| Caminho/túnel | Spline | Feito |
| Gerar por semente | Caverna aleatória | Feito |
| Fundir traços | Douglas-Peucker | Feito |
| Facetada | Low poly | Feito |
| *Stamp* de malha | Imprime a superfície da malha no campo | Feito (`sculpt.stamp_mesh`): grava a superfície numa banda; não transforma a malha em sólido (para isso, `sculpt.boolean`) |
| Booleanos | Unir/subtrair/intersectar volumes | Feito (`sculpt.boolean`) |
| Espelhar | Simetria | Feito (`sculpt.mirror`) |
| Preencher | Fecha cavidades | Feito (`sculpt.fill`) |
| Oco | Casca | Feito (`sculpt.hollow`) |
| Deslocamento por ruído | Irregularidade orgânica | Feito (`sculpt.noise`) |

## Ferramentas gerais

Medição 3D, snap à superfície, grade configurável por andar, camadas de edição, máscaras de pintura, presets de brush, atalhos para tudo, prévia ao vivo e estatísticas (triângulos/área) para orçamento.

## Peça comum: curva + perfil + efeito no terreno

```text
 Curva (spline)  ──►  Perfil (seção transversal)  ──►  Malha (varredura)
       │                                                    
       └──────────►  Efeito no terreno (aplainar, escavar, margem)
```

| Peça | Existe no Petunia3D? | Fonte (`LOCAL`) |
|---|---|---|
| Spline Bézier cúbica, handles, tolerância, limite de pontos | Sim | `project/spline.rs` |
| Quadros por ponto (para orientar o perfil) | Sim | `SplineFrame` |
| Varredura de perfil com orçamento de vértices | Sim | `path_generator.rs`, `mesh/sweep.rs` |
| Ponto preso ao triângulo de uma superfície | Sim | `surface_attachment.rs` |
| **Rede** (nós de ligação, cruzamentos) | **Não** | novo |
| **Efeito no terreno** (aplainar, escavar) | **Não** | novo |

## Terreno

### O que o criador faz
Escolhe um **bioma** com poucos controles (como no GameGuru MAX, `WEB`), esculpe e pinta.

### Como funciona
| Tema | Proposta |
|---|---|
| Dados | Mapa de altura em **blocos (chunks)**; camadas de material com índice por célula |
| Nível de detalhe | Blocos em quadtree com costura (sem buracos entre níveis) |
| Pincéis | Subir, descer, suavizar, aplainar, ruído, erosão simples, **carimbos** |
| Geração | Ruído fractal + controles de bioma (relevo, umidade, altitude) |
| Colisão | Collider de **heightfield** para a física |
| Desfazer | Uma pincelada = um passo; guarda só a diferença dos blocos tocados |
| Teclado | Pincel "passo": subir/descer por valor numérico, sem arrastar o mouse |

## Rios

### O que o criador faz
Clica pontos da nascente à foz. O rio **se ajusta ao terreno** e a água aparece.

### Como funciona
| Etapa | Descrição |
|---|---|
| 1. Curva | Spline com **largura e profundidade por ponto** |
| 2. Validação | O rio deve **descer**. Se subir, o Project Doctor avisa e oferece **baixar o leito** (suavizado) |
| 3. Escavar | Remove terreno no canal com **margens suaves** (falloff configurável) |
| 4. Malha | Varredura de um perfil de água sobre a curva |
| 5. Fluxo | Direção da curva + velocidade por largura, gravados nos **vértices** (vetor de fluxo) |
| 6. Espuma de margem | Duas opções: por profundidade (precisa ler a profundidade da cena) ou **máscara de margem gravada nos vértices** (funciona sem ler profundidade, barata) |
| 7. Confluências | Rio afluente se junta a outro por um nó de ligação |
| 8. Lagos | Polígono de água com **nível fixo** |
| 9. Quedas | Onde a inclinação passa de um limite: malha vertical + partículas |

Técnica de água conferida (`WEB`):

- Para **rios**, usar **mapa de fluxo** (dois estados de textura que se alternam), não só textura rolando. Assim a água corre devagar na margem e rápido no centro.
- Para lagos e mar, **4 a 8 ondas de Gerstner** no sombreador de vértices são o ponto doce de custo.
- Espuma pode vir de profundidade, altura de onda, inclinação, ruído e mapa de fluxo.

### Ligações com o jogo
| Efeito de jogo | Fonte |
|---|---|
| Zona de nado, flutuação | Volume gerado do rio |
| **Correnteza** que empurra personagens e objetos | O próprio vetor de fluxo |
| Respingos e som de passos na água | Eventos de entrada em volume |

## Estradas

### O que o criador faz
Traça estradas. Ao cruzar outra, o **cruzamento nasce sozinho**. Ao cruzar um rio ou vale, a **ponte aparece**.

### Como funciona
A estrada é um **grafo**: nós (pontos de ligação) e arestas (trechos de spline com um perfil).

| Tema | Proposta |
|---|---|
| Perfil | Faixas, calçada, acostamento, largura por trecho |
| Ligação de 2 trechos | Nó simples (continuação) |
| **Cruzamento (3+ trechos)** | Ordenar por ângulo, calcular cantos pela meia largura, **preencher o polígono** (triangulação), arredondar cantos com raio |
| Detecção de cruzamento | Pesquisa acadêmica descreve o problema (sobreposição e cintilação de texturas) e métodos com Bézier e SAT (`WEB`, DiVA 2022). Alternativa mais simples: dividir as splines no ponto de cruzamento e criar um nó |
| Terreno | Aplainar ao longo do eixo; **aterro/corte** com falloff |
| Pontes e bueiros | Se a diferença de altura passa de um limite: segmento de ponte ou bueiro |
| Marcações | **Decals** em faixa ao longo da curva (não entram na malha da estrada) |
| Navegação | A rede vira um **grafo de navegação** para IA e veículos |
| "Interligadas" | O Project Doctor avisa: pontas soltas e **ilhas desconectadas** |

Pesquisa clássica (`WEB`, "Procedural Generation of Roads") usa curvas suaves e **escavações e aterros** com tratamento de túnel e ponte. É a referência para a lógica de terreno.

### Riscos conhecidos
| Risco | Tratamento |
|---|---|
| Cruzamentos feios em ângulos agudos | Limite de ângulo mínimo + aviso |
| Texturas piscando em sobreposição | Nunca sobrepor malhas: o cruzamento é uma malha própria |
| Custo de recalcular a rede | Recalcular só os nós e trechos tocados (revisão por trecho) |
| Orçamento de vértices | Reaproveitar `MAX_GENERATED_VERTICES` e orçamento por gerador |

## Tabelas de dados (esboço)

```text
 Terrain   : chunk, altura[], material_ids[]
 River     : spline, largura[], profundidade[], velocidade_base
 RiverNode : tipo (nascente, confluencia, foz, queda), posição
 Road      : spline, perfil, material
 RoadNode  : tipo (ponta, ligação, cruzamento), posição, raio_canto
 Bridge    : road, trecho, tipo
```

## Decisões ligadas

`DR-01` licença (varredura e spline vêm do Petunia3D) · `DR-14` ordem de entrega.
