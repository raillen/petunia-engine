# Workspaces e o que vem antes da API

**Revisão:** 2026-10-24 · Estado: **proposta** ([DR-24](../06-decisoes/registro-de-decisoes.md)). Nada aqui está implementado, exceto o que a tabela de prioridades marca como feito.

## Resumo (TL;DR)

Proponho **cinco workspaces** que trocam ferramentas e painéis sobre a **mesma cena**: **Mapa** (com **Jogar**), **Pintura**, **Animação** e **Efeitos**. Antes da API, da física e do editor de código, recomendo terminar o que torna a **montagem de mapas** boa de verdade: andares completos, **cavernas**, **terreno**, **prefabs** e o **modo Jogar**.

## Os workspaces

O padrão vem do Petunia3D (`DRAW · POLY · PAINT · UV`): a mesma cena, outra caixa de ferramentas. Nada é copiado entre arquivos, e o trilho e os painéis mudam.

| Workspace | Para quê | Base que já existe | Primeiro passo |
|---|---|---|---|
| **Mapa** | Construir: brushes, gizmo, snap, andares, cavar | **feito** (esta fase) | — |
| **Jogar** (dentro de Mapa) | Andar pela cena como o jogador, com colisão, e voltar | `petunia run` (janela separada) | Botão ▶ no editor: câmera do jogador, `Esc` volta ao estado salvo |
| **Pintura** | Colorir **sem UV**: cor por face, cor por vértice, mistura de terreno | pincel do Petunia3D (análise: [pintura](../01-pesquisa/07-pintura-e-animacao.md)) | Cor por face nos brushes (balde e pincel) |
| **Animação** | Tocar clipes de modelos, trilhas de propriedades (portas, luzes) | núcleo do Petunia3D (~3,8 mil linhas) | Importar GLB com esqueleto |
| **Efeitos** (VFX) | Partículas, shaders e efeitos por **nós fáceis** | canvas de nós do Petunia3D (`node_canvas`, 2,1 mil linhas) | Emissor de partículas com 6 nós |

### Pintura sem UV (o caso low poly)

| Técnica | Como funciona | Bom para |
|---|---|---|
| **Cor por face** | Cada face guarda uma cor; nada de textura | Low poly "flat", blocagem, arte limpa |
| **Cor por vértice** | Cada vértice guarda uma cor; a GPU mistura | Degradês, sujeira nos cantos, oclusão pintada |
| **Paleta compartilhada** | As cores vêm de uma paleta do projeto | Coerência visual, mudar o tema do jogo inteiro |
| Projeção triplanar (depois) | Textura aplicada pela direção da face | Rocha, tijolo sem desdobrar UV |

**Recomendação:** cor por face primeiro, porque cabe no formato atual (uma coluna por face do brush) e não precisa de malha editável.

### Efeitos por nós

O Gameplay IR já prevê nós para lógica. Para efeitos, a ideia é um grafo **pequeno e tipado**: `Emissor → Forma → Velocidade → Cor no tempo → Tamanho no tempo → Saída`. Cada nó é um comando do schema, então o mesmo grafo vira texto, nós e API, sem segundo caminho.

## O que vem antes da API, da física e do editor de código

Critério: **o que a pessoa precisa para montar um mapa jogável** e que muda o formato dos dados (mais caro depois).

| # | Ferramenta | Estado | Por quê agora |
|---|---|---|---|
| 1 | **Andares** (grade sobe e desce, criação no andar) | **feito: básico** | Prédios e masmorras de vários níveis |
| 1b | Andares completos: fantasma ou ocultar acima do andar, lista de andares com nome e altura **salvos na cena** | **feito** (2026-10-25) | Sem ocultar, os andares de cima atrapalham |
| 2 | **Cavernas**: volume esculpido, 5 modos de pincel, caminho, gerador, fundir, facetada | **feito** (2026-10-25) | Pedido seu; não dá com caixas |
| 2b | Salas e túneis com caixas: **tornar oco** e **cavar** | **feito** | Já resolve masmorras retas |
| 3 | **Terreno**: mapa de altura com pincéis (subir, baixar, suavizar, nivelar) | planejado | Quase todo mapa externo começa nele |
| 4 | **Prefabs** (instâncias) e **arranjo** (cópias em linha, círculo, grade) | planejado | Colunas, cercas, árvores, sem copiar e colar |
| 5 | **Modo Jogar** dentro do editor | planejado | Testar a escala e a altura dos degraus |
| 6 | **Importar GLB** (modelos e esqueletos) | planejado | Destrava animação e objetos prontos |
| 7 | **Cor por face** (Pintura sem UV) | planejado | Mapa bonito sem textura |
| 8 | Luz básica (sol, ponto) e sombras | planejado | Ler o volume do mapa |
| — | API de scripts, física, editor de código | depois | Precisam de dados estáveis para não refazer |

### Sobre cavernas (o item mais pedido e mais caro)

| Opção | Como | Custo | Recomendação |
|---|---|---|---|
| A. Caixas ocas e cavadas | Já existe | zero | Para túneis retos |
| **B. Volume esculpido (SDF)** | Pincéis somam e subtraem esferas, cápsulas e ruído; *marching cubes* gera a malha | médio-alto (estimativa: 2 a 3 mil linhas) | **Sim**: cavernas orgânicas, também serve para terreno com saliências |
| C. Voxels editáveis | Cubos pequenos | alto, visual "minecraft" | Não, salvo se for o estilo do jogo |

## Cavernas: como funcionam (feito)

**Exemplo:** `Criar > Caverna` põe um bloco de rocha com um túnel. `B` (Esculpir) e arraste sobre a rocha: cava. A barra de cima escolhe o **modo**; `Ctrl` faz o oposto. `Shift+B` abre o **túnel por caminho**: clique pontos na rocha e `Enter`.

| Peça | O que é |
|---|---|
| Tabela `Sculpt` | O volume: `base` (`solid` = bloco, `empty` = nada), `size`, `cell_size`, `color`, `faceted` (low poly) |
| Tabela `SculptStroke` | Um traço: `mode`, `start`, `end`, `radius`, `smooth`, `strength` |
| Crate `petunia_sculpt` | Campo de distâncias **em memória**, blocos de 16³, malha por *surface nets*; sem dependências, determinístico |
| Um arrasto | Pré-visualizado ao vivo e salvo como **um** `world.batch`: um `Ctrl+Z` |

**Modos do pincel** (todos viram traços, então salvam, desfazem e viajam como o resto):

| Modo | O que faz | Com `Ctrl` |
|---|---|---|
| Cavar | Tira rocha com uma cápsula | Adiciona rocha |
| Adicionar rocha | Soma rocha | Cava |
| Inflar | Empurra a superfície para fora (em metros) | Esvazia |
| Nivelar | Puxa a superfície para o plano do primeiro toque | — |
| Suavizar | Borra o campo com os vizinhos | — |

**Outras ferramentas:** *Gerar túneis* (salas ligadas por túneis sinuosos a partir de uma **semente**; a mesma semente dá a mesma caverna), *Criar > Caverna aleatória*, *Fundir traços* (junta as cápsulas curtas de arrastos longos, sem mudar a forma além de ¼ de célula) e *Facetada* (triângulos chapados, calculados na placa de vídeo, sem mexer na malha).

### Como ficou rápido (medido, `--release`)

| Caverna | Traços | 1 pincelada **antes** | 1 pincelada **agora** |
|---|---:|---:|---:|
| 16×6×16 m | 300 | 54 ms | **0,7 ms** |
| 64×16×64 m | 1000 | 1063 ms | **0,9 ms** |
| 128×24×128 m | 300 | 290 ms | **0,5 ms** |

- O campo de distâncias fica guardado numa grade ancorada na origem do volume. Uma pincelada mexe só nas amostras ao redor e refaz só os **blocos** vizinhos (8 de 128 no caso grande).
- Cada bloco lê 18³ amostras e emite só as faces cujo ponto de baixo ele possui: **cada face é emitida uma vez** e os vértices da borda saem iguais dos dois lados (teste: o incremental é igual a construir do zero).
- Blocos são malhados em até **4 threads** (sem dependência: `std::thread::scope`).
- O mesmo bloco serve para **desenhar** e para **clicar** (antes a malha era calculada duas vezes); o clique rejeita blocos inteiros por caixa antes de testar triângulos.
- Na placa de vídeo cada bloco tem o seu buffer, e os antigos saem a cada pincelada.

**O preço (honesto):** a **primeira construção**, e desfazer, recalculam o campo todo: 95 ms com 300 traços e **459 ms com 1000** num volume de 64 m. Abrir um projeto grande ou dar `Ctrl+Z` numa caverna enorme tem essa pausa. Uma pincelada normal não.

- Por que *surface nets* e não *marching cubes* (GERAL): dispensa a tabela de 256 casos e gera menos triângulos, mais regulares, bom para low poly. Referência: Gibson, 1998 (algoritmo conhecido; código nosso). A pesquisa que embasou as escolhas está na conversa de 2026-10-25; fontes: Voxel Tools, Joris Rijsdijk, Nick Gildea, Mark et al. 2015, Alex Evans (Dreams, SIGGRAPH 2015).
- Não adotados, por decisão técnica: Dual Contouring (precisa de gradientes guardados, gera malhas não-manifold), voxels cúbicos editáveis, malhagem na GPU (um teste publicado ficou de 3 a 30 vezes mais lento que a CPU), LOD com costuras.

**Limites desta versão:**

| Limite | Efeito |
|---|---|
| Recalcular tudo ao abrir e ao desfazer | Pausa em cavernas com muitos traços (acima) |
| Até **4.096 traços** por caverna; *Fundir traços* só junta cavar e adicionar em cadeia | Inflar, nivelar e suavizar não se fundem |
| Traços não aparecem na Hierarquia nem se editam um a um | A caverna inteira se move, copia e apaga |
| A caverna não fica "fantasma" acima do andar | Só os brushes ficam |
| `Suavizar` não aparece em `Volume::distance` (precisa da grade) | Só afeta consultas exatas por traço |
| Colisão de caverna no modo de jogo | Ainda não existe (depende da física) |

## O que preciso de você

Uma resposta curta basta: **a ordem dos itens 3 a 8 está boa?**
