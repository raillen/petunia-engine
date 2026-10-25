# Especificação anterior (Petunia Game Engine 3D)

**Revisão:** 2026-10-24 · **Fonte:** `antigo-projeto[apenas-para-consulta-e-analise].md`, na raiz do projeto (`LOCAL`). Arquivo tratado como **somente leitura**.

## Resumo (TL;DR)

Esse arquivo é a especificação consolidada da **Petunia Game Engine 3D** (repositório `WASDst/petunia-game-engine`), datada de 2026-10-24, com decisões de 10/10/2026. **Esta pasta (`petunia-engine`) é a retomada desse projeto**, agora com escopo maior.

Resultado da comparação:

- **Muita coisa confirma** o que esta documentação propõe (schema único, handles, Project Doctor, GLB, runtime sem Qt).
- **Quatro pontos divergiam** das minhas primeiras recomendações. Eu **ajustei esta documentação** para respeitar as decisões anteriores.
- **O escopo cresceu.** A lista "não perseguir" da época contém itens que você pediu agora (online, física com cordas, clima rico). Isso é uma decisão de escopo, não um detalhe.

## O que a especificação anterior já decidiu (`[Decidido]`)

| Área | Decisão anterior | Efeito nesta documentação |
|---|---|---|
| Linguagem do núcleo | **Rust** | Mantida |
| Editor | **Qt 6 / CXX-Qt**, desacoplado do core | **Mantida.** `DR-04` deixa de ser escolha e vira validação (S4) |
| Renderer | **OpenGL leve** (o spike com `wgpu` foi "histórico; substituído") | **Mantida.** Minha recomendação por `wgpu` foi **retirada**; `DR-03` agora é "manter OpenGL" |
| Scripting | **Luau ou Aipo; uma linguagem ativa por projeto na V1** | **Adotada** (`DR-06`) |
| Host API | **Schema único**; dele derivam bindings, docs e metadados | Idêntica à ideia central desta documentação |
| Autoria visual | **Gameplay IR** + Event Sheets + grafos especializados; **não** espelhar a AST | **Adotada** (`DR-07`). Substitui minha proposta de "projeção da AST" |
| Simulação | **RuntimeWorld** separado da autoria; ECS "ou equivalente enxuto" | Compatível com `DR-05` |
| Geometria | **Brushes convexos** e CSG por etapas | **Entrou** no editor (modo Construir) |
| Intercâmbio | **GLB/glTF** + metadados opcionais; **Live Link** com Petunia3D depois | Mantido |
| Bibliotecas | Repositório próprio de crates pequenas (**`petunia-libs`**) | Adotado como o veículo de reuso (`DR-01`, `DR-11`) |
| Formatos de projeto | **Textuais, difáveis, amigáveis ao Git** | Adotado (`DR-10`) |
| Desktop | **Windows e Linux** primeiro; Web só depois de validar | Mantido |
| Acoplamento | Petunia3D e engine **não** se acoplam em runtime; o runtime **não** depende de Qt | Mantido |

## Pontos que divergiam, e como foram resolvidos

| # | Eu havia proposto | A decisão anterior diz | Resolução |
|---|---|---|---|
| 1 | `wgpu` como renderer | OpenGL leve | **Vale a anterior.** Os recursos pedidos (água, vegetação instanciada, decals, pós) cabem em OpenGL 3.3 sem *compute* (`HIPÓTESE`, validar em S3) |
| 2 | Qt como "provisório", com alternativas | Qt/CXX-Qt decidido | **Vale a anterior.** Alternativas ficam só como plano B |
| 3 | Nós como projeção da árvore da Aipo | Gameplay IR; nós **não** espelham a AST | **Vale a anterior**, que também satisfaz o seu pedido "nós 1:1 com a **API**". Projeção da AST vira estudo opcional (S5b) |
| 4 | Aipo + Luau disponíveis juntos | Uma linguagem ativa **por projeto** | **Vale a anterior.** Padrão: Aipo. Menos manutenção; nós funcionam nas duas porque vêm do IR |

## Convergências (a pesquisa nova reforça o que já estava decidido)

- **"Não duplicar API de gameplay entre Event Sheets, grafos, Luau e Aipo"** (princípio 3 da especificação) é a mesma ideia do schema único aqui.
- **Handles geracionais**, **capabilities**, **hot reload com regras de estado**, **budgets de execução**: todos já planejados.
- **Issue Browser / Project Doctor** com *Quick Fix* ("porta sem trigger", "spawn fora da navegação"). Esta documentação apenas amplia as verificações (mundo natural e acessibilidade).
- **MCP sem UI aberta** (`map.get`, `brush.create`, `trigger.create`, `play.test`): compatível com o MCP já existente no Petunia3D.
- **Input por ações**, remapeamento e mouse/teclado/gamepad.
- **Acessibilidade** como decisão ("[Decidido] minimizar complexidade e oferecer editor compreensível"); aqui virou plano com gates.

## Crescimento de escopo (precisa de decisão consciente)

| Tema | Antes | Agora (seu pedido) | Comentário |
|---|---|---|---|
| Terreno | Heightmap: subir, descer, suavizar | + camadas de material, bioma, erosão, carimbos, blocos com LOD | Maior |
| Água | **Plano com textura rolando** | **Rios com fluxo**, escavação, lagos, quedas | Muito maior |
| **Estradas** | Não havia | **Rede ligada**, cruzamentos, pontes | **Novo** |
| Vegetação | "Scatter de props" (proposta) | Regras, streaming, LOD, impostores | Maior |
| Clima | Presets de chuva, neve, fog como emissores | Estado único, transições, molhado/neve, zonas | Maior |
| Billboards | Cilíndrico e esférico | + octaédrico (impostor 3D) | Maior |
| Decals | Pool + fade | + projeção em tempo real, estradas, faixa | Médio |
| **Física** | Cinemática; rigidbody **fora do MVP** | Física leve + **cordas** (XPBD) | **Novo** (cordas) |
| **Online** | "Networking massivo" **não perseguir** | Sala por código, réplica de tabelas | **Novo** (nível pequeno) |
| Pós-processamento | Sem item | Cadeia leve + conforto sensorial | **Novo** |
| Acessibilidade | Editor | Editor **e jogos criados** | **Novo** (jogos) |
| Editor de código | Proposta | Integrado, com diagnósticos | Mantido |

A consequência está na [ordem das fases](../05-roadmap/01-fases-e-experimentos.md): a base da especificação anterior (P0: núcleo, editor, brushes, jogabilidade mínima, regras) **vem primeiro**. O mundo natural vem depois.

## Armadilhas apontadas pela própria especificação

1. **Não confundir 3D com a engine 2D** (`tinyEngine`, Python/PySide6) que o mesmo repositório recebeu depois. Estados de conclusão do repositório **não** provam que a 3D exista.
2. **Nomes históricos** (`map.tiny.json`, `tiny run`, `isWater`) estão "sujeitos a alteração". A decisão é migrar com compatibilidade, nunca renomear em silêncio.
3. **Não tratar propostas como decisões.** Esta documentação usa os rótulos da legenda (`LOCAL`, `HIPÓTESE`…) pelo mesmo motivo.
4. **Não mudar a stack sem ADR.** É por isso que as decisões herdadas aparecem marcadas como tal no [registro](../06-decisoes/registro-de-decisoes.md).

## As 12 pendências da especificação anterior × este registro

| # | Pendência (seção 29 do arquivo) | Onde aparece aqui |
|---|---|---|
| 1 | Nome final e extensões | `DR-11` |
| 2 | Forma exata do renderer OpenGL | `DR-03`, S3 |
| 3 | Física 3D e rigidbodies | `DR-08` |
| 4 | ECS interno enxuto | `DR-05` |
| 5 | CSG e round-trip brush ⇄ GLB | Fora do MVP; `DR-14` |
| 6 | Matriz de formatos de assets | `DR-10` |
| 7 | Animação esquelética V1 | Fora de escopo da v1 (importar clipes) |
| 8 | Formato físico do Gameplay IR | `DR-07` |
| 9 | Hot reload e estado | [Linguagens](../03-linguagens-e-api/01-estrategia-de-linguagens.md) |
| 10 | Limites de Web/WASM | Fora de escopo da v1 |
| 11 | Plugins e capabilities | [Linguagens](../03-linguagens-e-api/01-estrategia-de-linguagens.md) |
| 12 | Live Link com Petunia3D | `DR-11` |

## Cenários de aceite herdados (A–E)

A especificação traz cinco cenários de ponta a ponta (dungeon jogável, IA simples, escolher linguagem, intercâmbio com Petunia3D, agente via MCP). Eles **continuam valendo** e entram como demos das fases iniciais. Os cenários novos (rio, estrada, clima, cordas, online) ficam nas fases seguintes.

## Decisões ligadas

`DR-02` · `DR-03` · `DR-04` · `DR-06` · `DR-07` · `DR-10` · `DR-11` · `DR-14`.
