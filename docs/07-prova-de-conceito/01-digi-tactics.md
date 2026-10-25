# Prova de conceito: remake do Digi-Tactics

**Revisão:** 2026-10-24 · **Fonte:** leitura direta de `~/Documentos/digimon-stranger-story-animated-models` (`LOCAL`)

## Resumo (TL;DR)

O Digi-Tactics é um **auto-battler** (estilo Teamfight Tactics) em TypeScript, com núcleo determinístico em funções puras e comandos validados por schema. Isso é **a mesma filosofia da Petunia Engine**, escrita antes em TypeScript. Por isso ele é uma prova muito boa.

**Mas ele prova uma metade.** Não usa rios, estradas, cordas nem clima. A prova do mundo natural precisa de um segundo projeto (a "Ilha").

O melhor uso do Digi-Tactics: **(1)** fonte de requisitos agora, **(2)** teste de desempenho real da Aipo logo no início, **(3)** teste de aceite do remake depois.

## O que o projeto é (fatos conferidos)

| Item | Fato | Fonte |
|---|---|---|
| Gênero | Auto-battler competitivo; tabuleiro hexagonal; 8 jogadores (alvo) | `docs/GDD.md` |
| Tabuleiro | 28 hexes por lado (4×7), 9 slots de banco, coordenadas cúbicas | GDD §3 |
| Combate | **20 ticks por segundo**, A\* no grid, mana, habilidades, determinístico | GDD §2.2, `docs/en/tactics.md` |
| Limite em campo | **Nível do jogador = unidades em campo, máximo 9** | `tactics.md`, GDD |
| Conteúdo | 30 linhas compráveis (93 formas aprovadas), sinergias, Jogress, Mega, brasões | `roadmap.md` |
| Núcleo | `src/game/core`: ~4 mil linhas (cerca de 2 mil de lógica; o resto é catálogo) | contagem local |
| Camada de aplicação | ~4,1 mil linhas; comandos com Zod; consultas devolvem cópias | `src/application` |
| Estado implementado | **Partida offline 1v1**, 3 de 9 brasões; itens, Digiport e multiplayer adiados | `tactics.md` |
| Visual | Three.js / React Three Fiber; **cel-shading**; arena em arte pintada calibrada | README, `arena-editor.md` |
| Modelos | 625 GLB (476 Digimon, 149 NPCs), 28 mil clipes, **13 GB**; 4 modelos prontos para o jogo | README, `models/` |
| Exemplo medido | `agumon.glb`: 3,1 MB, 25 juntas, 33 animações, 19 imagens, 6 materiais | leitura do GLB |
| Plataforma-alvo | **Web** (desktop e celular); Tauri opcional | GDD §1.2 |
| Engenharia | Nomes completos em inglês, dados separados de comportamento, sem hierarquia de classes, "inspirado em structs e funções associadas da Aipo" | `AGENTS.md` |

## Por que é uma boa prova

1. **Mesmo desenho.** Comandos com schema, consultas imutáveis, núcleo sem dependência de renderização, os mesmos comandos para interface e agentes. É o que a [proposta](../00-visao/03-proposta.md) chama de "tudo é comando".
2. **Núcleo pequeno e já testado.** Cerca de 2 mil linhas de lógica e 34 arquivos de teste. Dá para portar para Aipo e **comparar resultados**.
3. **Determinismo comprovável.** O projeto já exige que a partida produza o mesmo vencedor, estado e registro de eventos que um replay (`tactics.md`, invariante 2).
4. **Rede natural.** Os comandos (`shop.purchase`, `unit.move`, `round.advance`…) já são o protocolo. Num servidor com autoridade, eles viajam como mensagens.
5. **Acessibilidade e documentação bilíngue** já fazem parte do processo do projeto.

## O que ele **não** prova

| Recurso da engine | Usado pelo Digi-Tactics? |
|---|---|
| Terreno, rios, estradas, vegetação, clima | Não |
| Cordas e física leve | Não |
| Decals, billboards 3D | Pouco (talvez efeitos) |
| Level Builder com brushes | Não (arena é arte pintada) |
| Online com 8 jogadores | **Sim**, mas adiado no projeto atual |

Conclusão: manter **duas provas**. **Prova 1:** Digi-Tactics (jogabilidade, animação, UI, rede). **Prova 2:** a Ilha (mundo natural).

## Requisitos novos que o Digi-Tactics traz

Estes itens **não estavam** nos documentos de arquitetura anteriores. Entram em [personagens, UI e grade](../02-arquitetura/09-personagens-ui-e-grade.md).

| # | Requisito | Evidência |
|---|---|---|
| R1 | **Reprodução de animação esquelética** (glTF com skin, mistura entre clipes, papéis `idle/walk/attack/skill/hit/death/victory`) | `combat.md`: 8 papéis, mistura de 0,15 s, "one-shot" e "terminal" |
| R2 | **Material toon (cel-shading) e contorno** | README, `shaders/` |
| R3 | **Pano de fundo 2.5D** (arte pintada + câmera calibrada) e **perfil de pós-processamento** exportável | `arena-editor.md` |
| R4 | **UI de jogo** com arrastar e soltar, dicas, textos em **en/pt-BR**, foco por teclado | GDD §2.1; HUD em React |
| R5 | **Módulo `grid`** (hexagonal, coordenadas cúbicas, distância, A\*) | GDD §3 |
| R6 | **Efeitos de habilidade** (projéteis, partículas, sincronia com animação) | `combat.md`, "Planned" |
| R7 | **Cozimento de assets** (remover clipes não usados, poda de buffers, carga sob demanda) | `combat.md`: compactação binária ainda não feita |
| R8 | **Registro de eventos e replay** como recurso da engine | `tactics.md` |
| R9 | **Internacionalização** de textos do jogo | docs bilíngues; wiki gerada |

> Ajuste de escopo: antes a [lista de fora de escopo](../05-roadmap/02-riscos-e-fora-de-escopo.md) dizia "animação esquelética". Esclarecendo: **tocar** animações vindas de glTF **entra** na v1. **Criar/editar** esqueletos e animações continua no Petunia3D.

## Desempenho: a Aipo aguenta este jogo?

Estimativa de ordem de grandeza (`HIPÓTESE`; a medição real é o experimento S1):

- Em campo há **no máximo 9 + 9 = 18 unidades** (`LOCAL`).
- Combate a **20 passos por segundo**.
- Suponha ~1.000 instruções por unidade por passo (busca de alvo, movimento, ataque): 18 × 1.000 × 20 = **360 mil instruções por segundo**.
- A Aipo registra um piso de ~100 ns por instrução (`LOCAL`, ADP-011): **cerca de 36 ms de CPU por segundo**, ou ~4% de um núcleo.

Mesmo com 10 vezes mais, cabe. **Este é o tipo de jogo em que a Aipo brilha**: lógica discreta, poucos objetos, tempo fixo. O risco da Aipo está em jogos de ação com milhares de entidades, não aqui.

## O plano de aproveitamento

### Passo 1 — Núcleo em Aipo, sem gráficos (Fase 3)

Portar `src/game/core` (coordenadas, economia, loja, sinergias, Jogress, combate, RNG) para **Aipo**, rodando em modo sem janela.

**Critério de aceite (teste diferencial):**

```text
 Para cada semente de 1 a N, com os mesmos elencos:
   resultado_Aipo.vencedor    == resultado_TypeScript.vencedor
   resultado_Aipo.registro    == resultado_TypeScript.registro   (byte a byte)
```

- Os registros de referência vêm da versão TypeScript (**gerados uma vez, versionados**).
- Valida a Aipo, o Script Host e o determinismo **antes** de existir renderer.
- A Aipo tem gerador com semente (`random`, passo SplitMix64, `LOCAL`). Falta confirmar se produz a **mesma sequência** do `rng.ts` do projeto; se não, adaptar um dos lados.

### Passo 2 — Comandos como schema (Fase 3)

Os comandos Zod do projeto (`shop.purchase`, `unit.move`, `unit.sell`, `unit.jogress`, `round.advance`…) viram **entradas do schema único** da engine. Resultado: o mesmo comando serve para **interface**, **script**, **MCP**, **teste** e **rede**.

### Passo 3 — Camada visual (Fase 4)

Tabuleiro hexagonal, personagens animados, material toon, HUD, arena pintada, efeitos. É aqui que entram R1 a R6 e R9.

### Passo 4 — Online (Fase 7)

Servidor com autoridade, mesma regra. O **conjunto compartilhado de cartas** (pool) é conservado no servidor. Sala de 8 jogadores com código.

### Passo 5 — Web (depois)

O jogo original é web. A engine mira **Windows e Linux** primeiro (decisão herdada). Para abrir a porta da web: manter o renderer dentro do subconjunto **WebGL2/OpenGL ES 3** (`HIPÓTESE`, validar em S3) e o runtime sem Qt. Exportar para WebAssembly fica depois da v1.

## Fatia mínima do remake (para não repetir o excesso de escopo)

O projeto original já limitou a si mesmo à **partida offline 1v1**. A prova repete a mesma fatia:

| Entra | Fica de fora (por enquanto) |
|---|---|
| 1 arena, 4 criaturas, loja, economia, nível, XP | As 30 linhas completas e as 93 formas |
| Tabuleiro hex, combate a 20 Hz, mana, 1 habilidade por criatura | Itens e receitas, Digiport |
| 3 brasões (coragem, bondade, confiança) | Os outros 6 brasões |
| Jogress simples | Mega, Omnimon |
| Português e inglês | Outros idiomas |
| 2 jogadores online (depois) | 8 jogadores e carrossel |

## Direitos e procedência dos ativos (importante)

- Os modelos vêm do **Digimon Story: Time Stranger**, extraídos por um visualizador de terceiros (`LOCAL`, README). O próprio README diz que **"os direitos de redistribuição devem ser verificados separadamente"**.
- O `shaders/` é uma **recriação feita por fãs**, a partir de engenharia reversa do jogo (`LOCAL`, `shaders/README.md`). A licença não está clara.
- Digimon é propriedade intelectual de terceiros.

Isto **não impede** a prova, mas muda **como** ela é feita. As regras de proveniência da engine (`AGENTS.md`, `ATTRIBUTION.md`) valem aqui.

| Uso | Recomendação |
|---|---|
| Testes locais da engine | Modelos como **fixtures locais, fora do Git** (o projeto já mantém quase todos fora; só 4 modelos estão liberados no `.gitignore`) |
| Repositório e demos **públicos** da engine | **Criaturas originais** com o mesmo layout (esqueleto de ~25 juntas, 8 papéis de animação), feitas no **Petunia3D** |
| Material toon | **Escrever um shader toon original** na engine. Não copiar o GLSL recriado do jogo |
| Importar `extras.dsts` | Somente num adaptador **privado**, nunca no núcleo da engine |

Boa notícia: o Petunia3D já tem esqueleto, animação e IK (`rig.rs`, `animation.rs`, `ik.rs`, `LOCAL`). Ele consegue produzir as criaturas originais da demo pública. Isso ainda **testa o intercâmbio** Petunia3D → engine por GLB.

## Riscos específicos

| Risco | Tratamento |
|---|---|
| O remake vira "refazer o jogo inteiro" | Fatia mínima fixa; critérios de aceite por passo |
| A sequência de números aleatórios da Aipo difere da do TypeScript | Decidir qual lado se adapta; teste diferencial detecta na hora |
| UI de jogo é um trabalho grande (arrastar e soltar, dicas, i18n) | Experimento S9 antes da Fase 4 |
| 13 GB de modelos pesam no desenvolvimento | Cozimento de assets (R7); usar só os 4 modelos da fatia |
| Direitos dos ativos | Tabela acima |
| Web é o alvo do jogo original | Desktop primeiro; manter subconjunto WebGL2; decisão `DR-16` |

## Decisões ligadas

`DR-16` prova de conceito · `DR-06` linguagens · `DR-09` online · `DR-14` ordem das fases.
