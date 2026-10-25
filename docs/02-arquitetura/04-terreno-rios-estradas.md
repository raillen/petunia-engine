# Terreno, rios e estradas

**Revisão:** 2026-10-24 · **Estado:** proposta; precisa de protótipos

## Resumo (TL;DR)

Terreno, rios e estradas são **a mesma família**: curvas (splines) + um perfil + regras que **modificam o terreno** ao redor.

O Petunia3D já tem as peças centrais (`LOCAL`): spline Bézier com quadros por ponto, varredura de perfil ao longo de um caminho e "pontos presos a superfície". A engine reaproveita essas peças e acrescenta **redes**: estradas se ligam em cruzamentos, rios se juntam em confluências.

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
