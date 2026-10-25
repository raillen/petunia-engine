# Pintura e animação: vale trazer do Petunia3D?

**Revisão:** 2026-10-24 · **Fonte:** leitura e medição de `petunia3D` no commit `dec467127a7f` (`LOCAL`). Números de linhas são medidos. Custos de trabalho são **estimativas** (`HIPÓTESE`). Decisão relacionada: [DR-23](../06-decisoes/registro-de-decisoes.md).

## Resumo (TL;DR)

**Animação de reprodução: sim, e cedo.** A engine precisa tocar clipes e esqueletos para os personagens do Digi-Tactics.
**Pintura de texturas de modelos e autoria de esqueleto: não na engine.** O Petunia3D já é a ferramenta para isso; a engine importa o resultado em GLB.
**Pintura de mundo (terreno, cor de vértice, decals): sim, mas só o pincel**, não o motor inteiro.

| Capacidade | Recomendação | Quando |
|---|---|---|
| Reproduzir animação de esqueleto (clipes, mistura, skinning) | **Trazer** o núcleo (~3,8 mil linhas) | Fase de personagens |
| IK de pés e mira | **Trazer** (`ik.rs`, 682 linhas, funções puras) | Fase de personagens |
| Animação de propriedades (tweens, cinemáticas) | **Fazer novo**, pequeno, sobre a tabela de mundo | Fase 3 |
| Editor de esqueleto e poses (rig, auto-rig, retarget) | **Não trazer**; importar GLB | Revisar na fase 7 |
| Pintura de textura em modelos (UV, camadas, efeitos) | **Não trazer** | Revisar se houver demanda real |
| Pincel de terreno, cor de vértice, decals | **Trazer só o pincel** (falloff, traço) | Fase 4 (mundo natural) |

## Pintura

### O que o Petunia3D tem (medido)

| Parte | Linhas | O que faz |
|---|---:|---|
| `module-paint` | 10.019 | Pincéis (pixel, suave, borracha, balde, conta-gotas, linha, retângulo), pintura 3D por projeção UV, máscaras, caminhos |
| ↳ `engine.rs` | 964 | Motor de pinceladas: restrição por face, pincel esférico 3D, buffer de traço |
| ↳ `path_paint.rs` | 1.550 | Amostragem de traço, linha sem serrilhado |
| ↳ `projection.rs` | 1.822 | Projeção e estêncil |
| ↳ `paint_stack_commands.rs` | 1.804 | Camadas, efeitos, decals |
| UV (`mesh/uv*` 3.665 + `module-uv` 345) | ~4 mil | Desdobrar a malha para textura |

### Por que **não** trazer o conjunto

- **Acoplamento.** `module-paint` depende de `petunia_core` (`AppState`, `CommandContext`), de `petunia_mesh` (UV) e de `petunia_project` (camadas e `Canvas`). Para pintar é preciso trazer **malha, UV e o formato de projeto** junto.
- **Custo de dependência.** O crate `project` puxa `image`, `resvg`, `gltf`, `zip`, `tobj`, `postcard`. Seria o maior salto de dependências da engine.
- **Duplicação de ferramenta.** A pessoa que pinta modelos já usa o Petunia3D. A engine recebe o GLB com a textura pronta.
- **Acessibilidade.** Pintura é difícil de tornar acessível (precisão de mouse, cor). Faria sentido resolver uma vez, no Petunia3D.

### O que a engine precisa pintar de fato

| Uso | O que é | Precisa de UV? |
|---|---|---|
| **Terreno** (grama, terra, rocha) | Mistura de camadas por ponto do terreno | não (mapa de mistura) |
| **Cor de vértice** | Tinta por vértice em malhas | não |
| **Decals** | Marcas projetadas (sujeira, tiros) | não |
| Textura de modelo | Pintar o albedo de um objeto | **sim** |

Os três primeiros não precisam de UV nem de camadas de pixel. Só os **pincéis**.

### Opções

| Opção | O que é | Custo (estimativa) |
|---|---|---|
| **A. Nada** | Pintura fica no Petunia3D | zero |
| **B. Só o pincel** | Copiar o cálculo de pincel (`dab_falloff`, amostragem de traço, `pixel_perfect`) para o terreno e a cor de vértice | baixo-médio (~1 a 2,5 mil linhas) |
| **C. Pintura completa** | `module-paint` + UV + camadas | muito alto (traz `mesh`, `project`) |

**Recomendação: A agora, B na fase 4.** C só se houver pedido real e depois de resolver a [edição de malha](../02-arquitetura/13-primitivas-e-edicao-de-malha.md).

## Animação

### O que o Petunia3D tem (medido)

| Parte | Linhas | O que faz | Pura? |
|---|---:|---|---|
| `project/animation.rs` | 1.529 | Clipes, trilhas, keyframes (degrau, linear, cúbico), camadas de movimento, **presets de esqueleto** (humanoide, quadrúpede, serpente, peixe, ave, multi-pernas), auto-rig | em parte (auto-rig usa malha) |
| `project/rig.rs` | 1.581 | Esqueleto, ossos, pose de repouso, pesos de skinning | sim |
| `project/ik.rs` | 682 | IK de dois ossos, FABRIK, olhar para (funções **puras e determinísticas**) | sim |
| `project/rig_roles.rs` | 752 | Papéis (perna, braço) | sim |
| `project/gltf_rig.rs` | 962 | **Exporta** skin e animações para glTF | lado de exportação |
| `project/animatable.rs` | 509 | O que pode ser animado | sim |
| `editor/animate.rs` | 2.144 | Ponte do workspace Animate ("um clique e já se move") | interface |

O workspace **Animate** do Petunia3D está atrás de uma feature (`animation-workspace`) e **não tem painéis em Qt** ainda (verifiquei a pasta de C++).

### Os quatro usos na engine

| Uso | Necessário? | Observação |
|---|---|---|
| **Tocar clipes de modelos GLB** | **Sim** | Os personagens do Digi-Tactics são modelos animados |
| **IK** (pés no chão, mira) | Sim, na fase de personagens | `ik.rs` já está pronto e é puro |
| **Animar propriedades** (porta abrindo, luz piscando, cinemática) | Sim, simples | Trilhas sobre colunas do mundo; vale fazer **novo**, pequeno |
| **Criar esqueletos e poses** | Não na engine | É o trabalho do Petunia3D |

### O que falta para reproduzir

O Petunia3D tem a **exportação** de skin e animação. A engine precisa do lado oposto: **ler** o GLB (skin, juntas, animações) e calcular as matrizes de skinning. Isso é trabalho novo, mas pequeno em volta do núcleo copiado. A importação GLB já está planejada na fase 2.

### Opções

| Opção | O que é | Custo (estimativa) |
|---|---|---|
| **A. Só reprodução** | Núcleo (clipes, esqueleto, IK) + importar GLB + skinning | médio (~3,8 mil linhas copiadas + importação nova) |
| **B. Reprodução + propriedades** | A, mais trilhas de propriedades da cena | médio |
| **C. Autoria completa** | B, mais editor de poses, auto-rig, retarget, linha do tempo no editor | muito alto (e a interface de animação nem existe em Qt no Petunia3D) |

**Recomendação: B, nesta ordem:** primeiro A (personagens), depois as trilhas de propriedade. C só se a engine virar ferramenta de animação, o que não é o objetivo.

### Cuidados ao copiar

- O núcleo usa `glam` e `uuid`. Na engine, `glam` fica **dentro** do crate, sem aparecer na API, no schema ou nos arquivos (regra de bibliotecas).
- A cópia é **legado**: o Petunia3D usa nomes como `ctx`, `idx`, `cfg`. Migrar ao tocar, com teste de regressão.
- O determinismo importa: `ik.rs` já é determinístico; confira o mesmo para a amostragem de clipes antes de usar em simulação.

## Comparação final

| | Valor para a engine | Custo | Risco | Veredito |
|---|---|---|---|---|
| Reprodução + IK | **alto** (personagens) | médio | baixo | **Fazer** |
| Trilhas de propriedade | médio | baixo | baixo | **Fazer** (novo) |
| Pincel de mundo | médio | baixo-médio | baixo | **Fazer na fase 4** |
| Pintura de textura | baixo (já existe no Petunia3D) | muito alto | alto | **Não** |
| Autoria de esqueleto e poses | baixo (idem) | muito alto | alto | **Não** |

## O que mudaria minha recomendação

- Se os usuários da engine **não tiverem** o Petunia3D: pintura de textura e autoria de esqueleto sobem de prioridade.
- Se o Petunia3D **parar** de evoluir a pintura: reavaliar copiar o conjunto.
- Se a medição de build (experimento S0) mostrar que o crate `project` fica barato: a pintura completa fica menos cara.

## Próximo passo sugerido

Fazer a **importação de GLB com esqueleto** (fase 2) e só então copiar o núcleo de animação. Sem o GLB, não há como validar a reprodução.
