# Personagens, UI de jogo e grade

**Revisão:** 2026-10-24 · **Estado:** proposta; vem do [Digi-Tactics](../07-prova-de-conceito/01-digi-tactics.md); precisa do experimento S9

## Resumo (TL;DR)

Cinco sistemas que a primeira versão dos documentos **não cobria** e que qualquer jogo com personagens precisa: **animação esquelética (reprodução)**, **material toon**, **pano de fundo 2.5D**, **UI de jogo** e **módulo de grade**.

Todos seguem a regra de sempre: a **API é de alto nível** (`animation.play(unit, "attack")`, não matrizes de ossos).

## 1. Animação esquelética (reprodução)

**Dentro do escopo:** carregar glTF com *skin*, tocar clipes, misturar, controlar estados.
**Fora do escopo:** criar ou editar esqueletos e clipes (fica no Petunia3D).

### Papéis de animação (herdados do Digi-Tactics, `LOCAL`)

Em vez de nomes soltos de clipes, o jogo fala em **papéis**:

| Papel | Comportamento |
|---|---|
| `idle` | Repete sempre |
| `walk` | Repete; locomoção |
| `attack`, `attack_alt`, `skill`, `hit` | Toca uma vez e volta para `idle` |
| `death`, `victory` | Toca uma vez e **fica no último quadro** (estado terminal) |

Regras úteis, já provadas no projeto:

- Mistura entre clipes (padrão 0,15 s).
- Se faltar o clipe de `death`, a unidade ainda vira **terminal** e não age mais.
- A tabela de papéis é **dado** do personagem (um mapa papel → clipe), não código.

```aipo
# Ilustrativo
animation.play(unit, role = "attack")
animation.on_finished(unit, role = "attack", do_this = fn(e) { ... })
```

### Técnica
GPU skinning com matrizes de ossos em textura ou array de uniformes (cabe em OpenGL 3.3, `HIPÓTESE`). Limite de juntas e de personagens por quadro vira **orçamento** visível no editor.

## 2. Material toon (cel-shading) e contorno

- Implementação **original** da engine (não copiar o GLSL recriado do jogo; veja [direitos](../07-prova-de-conceito/01-digi-tactics.md)).
- Parâmetros como **dados do material**: cores de degrau, limiar, rim light, espessura do contorno.
- Contorno por passe de casco invertido ou por borda na profundidade/normais (a escolher em S3).
- Presets no editor: "Anime", "Pintura", "Plano".
- O criador não escreve shader. Escolhe preset e ajusta sliders.

## 3. Pano de fundo 2.5D

O Digi-Tactics usa **arte pintada** como arena e calibra uma câmera e um tabuleiro em perspectiva sobre ela (`LOCAL`, `arena-editor.md`).

| Parte | Proposta |
|---|---|
| **Placa de fundo** | Imagem (ou camadas) atrás da cena 3D |
| **Câmera calibrada** | Âncoras de pixel do tabuleiro → câmera de perspectiva |
| **Camadas de profundidade** | Planos opcionais na frente dos personagens |
| **Perfil exportável** | JSON versionado e **limitado a 64 KiB**, com validação estrita (padrão do projeto) |
| **Pós-processamento** | O mesmo da [renderização](03-renderizacao-e-pos.md): bloom, vinheta, cor |

Editor: um painel especializado no modo **Atmosfera**, com comparação A/B (o projeto original já tem isso).

## 4. UI de jogo

O HUD do Digi-Tactics (loja, banco, ouro, nível, dicas, arrastar unidades) é **React**. Na engine precisa existir uma UI própria, e este é um trabalho **grande**.

### Requisitos mínimos

| Requisito | Detalhe |
|---|---|
| Elementos | Texto, imagem, botão, barra, lista, grade, caixa de diálogo |
| **Arrastar e soltar** | Com alternativa **por teclado** (selecionar, mover, confirmar) |
| **Dicas (tooltips)** | Por foco e por ponteiro |
| **Layout** | Âncoras, alinhamento, tamanhos relativos, escala de interface |
| **Textos** | Chaves de tradução, **en e pt-BR** em paridade |
| **Acessibilidade** | Árvore de acessibilidade (AccessKit), foco visível, contraste |
| **Ligação a dados** | Elementos ligados a tabelas do mundo |
| **Eventos** | Viram ações/eventos da API |

### Opções para construir (`DR-17`, a decidir após S9)

1. **UI própria retida, descrita como dados**, com árvore de acessibilidade. — **recomendada**
2. **Reaproveitar** `packages/aipo-ui` / `aipo-egui` / Zoe (`LOCAL`: existem; adequação a verificar).
3. **Modo imediato** (estilo egui) para o jogo. Rápido de fazer; pior para acessibilidade e layout complexo.

## 5. Módulo `grid`

Os jogos táticos reaproveitam a mesma matemática. Em vez de cada jogo reescrever, a engine oferece um **bloco de alto nível** (lição do Godot: nós e APIs valem quando embrulham operações maiores).

| Função (ilustrativa) | Faz |
|---|---|
| `grid.hex(size = ...)` | Cria grade hexagonal (coordenadas cúbicas `q + r + s = 0`) |
| `grid.square(size = ...)` | Grade quadrada |
| `grid.distance(a, b)` | Distância exata |
| `grid.neighbors(cell)` | Vizinhas |
| `grid.path(from, to, blocked = ...)` | Caminho A\* |
| `grid.range(cell, radius)` | Células ao alcance |
| `grid.to_world(cell)` / `grid.from_world(point)` | Conversões |

Fórmula já usada no projeto: `distância = (|Δq| + |Δr| + |Δs|) / 2` (`LOCAL`, GDD §3). A grade também serve a **waypoints e A\*** herdados da especificação anterior.

## Tabelas de dados (esboço)

```text
 Skeleton     : juntas, hierarquia, pose_de_repouso
 AnimationSet : papel → clipe, mistura, repete?, terminal?
 ToonMaterial : degraus[], limiar, rim, contorno{espessura, cor}
 Backdrop     : imagem, âncoras_de_pixel, camada_de_profundidade[]
 UiTree       : nós, âncoras, texto_chave, ação
 Grid         : tipo, tamanho, células_bloqueadas
```

## Riscos

| Risco | Tratamento |
|---|---|
| Skinning em OpenGL 3.3 acima do orçamento | Limite de juntas; S3 mede |
| UI de jogo consome meses | S9 antes da Fase 4; começar pelo mínimo do Digi-Tactics |
| Contorno toon custa em GPU fraca | Duas técnicas; perfil Simples sem contorno |
| Placa 2.5D e câmera livre se confundem | Modo "câmera fixa" explícito no projeto |

## Decisões ligadas

`DR-16` prova de conceito · `DR-17` UI de jogo · `DR-03` renderer.
