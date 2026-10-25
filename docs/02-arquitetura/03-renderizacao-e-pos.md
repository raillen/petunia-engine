# Renderização e pós-processamento

**Revisão:** 2026-10-24 · **Estado:** OpenGL decidido antes; validação pendente (S3)

## Resumo (TL;DR)

O renderer é **direto (forward) com pré-passe de profundidade**, simples e barato. Isso funciona bem em hardware modesto e facilita MSAA, água e decals.

O pós-processamento é uma cadeia **curta e fundida**: bloom leve → **um único passe** de tonemap + cor + vinheta + grão → suavização de bordas. Tem controles de conforto sensorial.

**Decisão herdada (`DR-03`):** **OpenGL leve**, já decidido na [especificação anterior](../01-pesquisa/05-especificacao-anterior.md). Esta documentação **mantém** a escolha e a valida no experimento S3. O `wgpu` fica como plano B.

## Fatos do ponto de partida

| Fato | Fonte |
|---|---|
| O Petunia3D usa OpenGL 3.3 Core via `glow`, com fronteira neutra `RenderFrameInput` | `LOCAL` |
| Ele já tem sombreadores de **SSAO** (`ao.frag`, `ao_blur.frag`) | `LOCAL` |
| O Petunia3D **abandonou** egui/WGPU no editor ("não manter em paridade") | `LOCAL`, ADR-C11 |
| O motivo foi o custo de manter **dois** renderers/frontends, não um defeito do wgpu | `LOCAL` (interpretação minha) |

**Lição:** nunca manter dois renderers em paridade. Escolher um e manter a fronteira neutra.

## API gráfica (`DR-03`): manter OpenGL

A especificação anterior decidiu **OpenGL leve** e disse que o spike com `wgpu` era "histórico; substituído" (`LOCAL`). Também pediu um "caminho gráfico básico para GPUs modestas, **evitando depender de compute shaders**" (`LOCAL`).

Tudo o que você pediu **cabe em OpenGL 3.3 sem compute** (`HIPÓTESE`, validar em S3):

| Recurso | Como cabe em OpenGL 3.3 |
|---|---|
| Vegetação instanciada | Desenho instanciado por chunk; vento no sombreador de vértice |
| Água com refração | Cópia da cor da cena + textura de profundidade |
| Decals projetados | Profundidade do pré-passe; volumes desenhados depois do opaco |
| Pós-processamento | Alvos de renderização (FBO) e passes de tela cheia |
| Sombras em cascata | Mapas de sombra com PCF |
| Culling | **Na CPU**, por chunk (sem culling na GPU) |

### Opções, em ordem de preferência

1. **Manter OpenGL 3.3 (`glow`)**, reaproveitando o `render` do Petunia3D. — **escolhida**
2. **`wgpu`** (Vulkan/Metal/DX12/WebGPU), só se S3 mostrar um bloqueio ou se o Web virar prioridade. O modo OpenGL do `wgpu` tem menos recursos (`HIPÓTESE`).
3. **Bevy render.** Descartada: impõe o modelo de mundo do Bevy e quebra API a cada versão (`WEB`).

### Fatos do ponto de partida

| Fato | Fonte |
|---|---|
| O Petunia3D usa OpenGL 3.3 Core via `glow`, com fronteira neutra `RenderFrameInput` | `LOCAL` |
| Já tem sombreadores de **SSAO** (`ao.frag`, `ao_blur.frag`) | `LOCAL` |
| O Petunia3D abandonou egui/WGPU no editor ("não manter em paridade", ADR-C11) | `LOCAL` |
| Motivo: custo de manter **dois** renderers, não defeito do `wgpu` | `LOCAL` (interpretação minha) |

**Regra mantida:** nunca manter dois renderers em paridade. A fronteira neutra de quadro (`RenderFrameInput`) deixa a troca futura possível sem reescrever o mundo.

## Passes de imagem

```text
 1. Sombras (cascatas)        → 2–3 cascatas + PCF; perfil Simples: 1 cascata
 2. Pré-passe de profundidade → ajuda decals, água e AO
 3. Opaco (PBR simples)       → metal/rugosidade do glTF
 4. Terreno                   → mistura de camadas por altura
 5. Vegetação                 → instanciada, vento compartilhado
 6. Estradas e decals         → projetados sobre o que já está desenhado
 7. Céu e atmosfera
 8. Água                      → depois do opaco; refração + profundidade
 9. Transparentes
10. Pós-processamento         → veja abaixo
11. UI do jogo                → por último, sem pós
```

## Pós-processamento leve

Referência de desenho conferida em projetos abertos (`WEB`): cena em HDR, AO opcional, bloom em meia resolução, um passe fundido de tonemap + gradação (LUT 3D 32³) + vinheta + grão, e suavização (SMAA) no fim. Um exemplo estima ~0,5 ms em GPU moderna e ~2,5 ms em um portátil (Steam Deck).

| Etapa | Escolha | Observação |
|---|---|---|
| Cena | HDR (`Rgba16Float`) | Padrão |
| **AO** | GTAO opcional | Só no perfil Alto; reaproveita o SSAO do Petunia3D como estudo |
| **Bloom** | Duplo filtro (Kawase), meia resolução | Barato |
| **Tonemap** | **AgX** como padrão; **Khronos PBR Neutral** como alternativa | AgX preserva melhor as cores e é boa base de gradação (`WEB`) |
| **Cor** | LUT 3D + lift/gamma/gain | Presets: "Natural", "Suave", "Contraste alto" |
| **Vinheta e grão** | Desligados por padrão em "Conforto" | |
| **Suavização** | SMAA (1x) depois do tonemap; FXAA no perfil Simples | |

Tudo que puder vai num **único passe de tela cheia** para economizar leitura de memória.

### Conforto sensorial (diferencial)

Estes controles são parte do pós-processamento, não complementos.

| Controle | O que faz | Por quê |
|---|---|---|
| **Limitador de flash** | Limita mudança brusca de luminância por segundo (relâmpago, explosão, bloom) | Fotossensibilidade (Game Accessibility Guidelines: "evitar imagens piscantes", `WEB`) |
| **Reduzir movimento** | Desliga tremor de câmera, balanço, desfoque de movimento | Enjoo e sobrecarga |
| **Estímulo geral** | Controla partículas, saturação e bloom em conjunto | Perfis "Calmo", "Padrão" |
| **Contraste e cor** | Paletas seguras para daltonismo; **simulação** de daltonismo só como ferramenta do criador | Não "corrigir" cores sozinho; avisar quando a informação depende só da cor |

O **Project Doctor** verifica se o jogo expõe esses controles (veja [acessibilidade dos jogos](../04-acessibilidade/02-jogos-criados.md)).

## Materiais

- Padrão: PBR metal/rugosidade, como o glTF.
- Grafo de materiais: **só no modo avançado**, seguindo a filosofia do Petunia3D ("o usuário vê presets; o grafo é Advanced-only", `LOCAL`, `surface_recipe.rs`).
- Presets de material para o mundo: terra, grama, pedra, asfalto, madeira, água, lama.

## Orçamentos

- Cada passe tem um **orçamento declarado** (triângulos, chamadas de desenho, memória de textura), visível no editor.
- **Escala dinâmica de resolução** opcional, para manter o ritmo em máquinas fracas.
- Nenhum número é prometido antes de medir (S3).

## Decisões ligadas

`DR-03` renderer · `DR-12` decals · `DR-13` pós-processamento.
