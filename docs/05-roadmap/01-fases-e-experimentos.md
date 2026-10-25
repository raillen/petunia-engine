# Fases e experimentos

**Revisão:** 2026-10-24 (9 fases, após incluir o [Digi-Tactics](../07-prova-de-conceito/01-digi-tactics.md)) · **Estado:** proposta. Sem datas: só **ordem** e **demo** de cada fase.

## Resumo (TL;DR)

Ordem: **fechar 2 pendências → rodar experimentos curtos (spikes) → construir em 9 fases**, cada uma com uma **demo**.

Duas **provas** guiam o plano:

- **Prova 1 — Digi-Tactics** (jogabilidade, animação, UI, rede): núcleo testado já na **fase 3**; versão visual na **fase 4**; online na **fase 7**.
- **Prova 2 — a Ilha** (rios, estradas, clima, cordas): **fases 5 e 6**.

Princípio herdado: *"não começar pela UI cosmética antes de modelo, serialização, comandos e execução"* (`LOCAL`). Sem datas, porque o Petunia3D adota "não inventar números; medir" (`LOCAL`).

## Fase 0 — Fundações (agora)

| Item | Estado |
|---|---|
| Análise do Petunia3D, da Aipo, da especificação anterior e do Digi-Tactics | Feito |
| `DR-01` licença (código entre seus projetos) | **Decidida** |
| Confirmar `DR-02` (objetivo) e `DR-11` (nome e relação) | **Pendente** |
| Aprovar as regras de `DR-16` (provas e ativos) | **Pendente** |
| Auditar o repositório `petunia-game-engine` (separar o 3D do 2D) | Pendente |
| Rodar S0 a S4 | Pendente |

## Experimentos (spikes)

Cada experimento é **curto e descartável**, com critério de aceite. Resultado: **1 página com medidas**.

| # | Pergunta | Aceite | Decide |
|---|---|---|---|
| **S0** | Quanto custa compilar o núcleo sem Qt, `mlua`, `rmcp` e `wasmtime`? **Parcial: 27,7 s do zero (com janela), 3,3 s incremental.** Repetir a cada fase | Tempo de build **frio** e **incremental**, na sua máquina, com `CARGO_BUILD_JOBS=2` | `DR-20` (decidida) |
| **S1** | A Aipo aguenta ser a linguagem de gameplay? | Rodar a suíte da Aipo (hoje **não executada**). **Carga real:** portar `coordinates`, `rng`, `economy`, `shop` e `combat` do Digi-Tactics e passar no **teste diferencial** contra o TypeScript (mesma semente, mesmo registro de eventos). Medir custo por passo de 50 ms | `DR-06` |
| **S2** | O Luau via `mlua` entra bem como alternativa? | Sandbox + limite de CPU (`set_interrupt`) + tipo `vector`; **mesma API** do S1, do **mesmo schema**; tempo de build | `DR-06` |
| **S3** | O OpenGL 3.3 cobre os efeitos pedidos? | Cubo + 1 chunk de terreno + água + 10 mil plantas instanciadas + 1 decal + pós fundido **e** 1 personagem com *skinning* e material toon, **na sua máquina**, sem compute, dentro do subconjunto WebGL2 | `DR-03` |
| **S4** | O Qt/CXX-Qt se sustenta? | Viewport OpenGL embutido; leitor de tela lê o inspetor; **tempo de build e memória** locais; prova de editor de código | `DR-04` |
| **S5** | O Gameplay IR dá conta de um Event Sheet real? | Regra "porta + chave"; "Ver como código" em Aipo **e** Luau; edição desfazível; vista em contorno | `DR-07` |
| **S5b** | *(opcional, depois da v1)* Ida e volta completa texto ⇄ nós? | Teste de ida e volta sobre o corpo de conformidade da Aipo | — |
| **S6** | Cordas XPBD ficam boas e baratas? | Corda de 200 segmentos, colisão com colliders estáticos, âncoras em superfície, corte; determinismo repetido | `DR-08` |
| **S7** | A réplica de comandos e tabelas cabe na banda? | 1 anfitrião + 2 clientes; comandos do Digi-Tactics como mensagens; 1000 entidades com delta por coluna | `DR-09` |
| **S8** | Os gates de acessibilidade são viáveis? | Inspetor + paleta de comandos só por teclado e leitor de tela; contraste automático; um teste humano curto | `DR-15` |
| **S11** | *(opcional, só se S0 reprovar)* Vale C# com Avalonia na pilha inteira? | Uma fatia: janela com viewport OpenGL, leitor de tela no inspetor (Orca/NVDA), Aipo por ABI em C; medir build e iteração | `DR-20` |
| **S10** | *(opcional)* Vale portar o núcleo para Odin? | Portar `spline` + `sweep` + `triangulate` (malha de uma estrada); medir tempo de porte, de compilação e desempenho; **saída idêntica** à do Rust (teste diferencial) | `DR-20` |
| **S9** | Qual UI de jogo? | Tela de loja do Digi-Tactics: lista, arrastar e soltar **com alternativa por teclado**, dicas, en/pt-BR, árvore de acessibilidade | `DR-17` |

Ordem sugerida: **S0, S1, S2, S3, S4** → **S5, S9** → **S6, S7** → **S8** (começa cedo, repete sempre).

## Fases de construção

### Fase 1 — Esqueleto andando  *(em andamento; veja o [estado da implementação](03-estado-da-implementacao.md))*
**Entrega:** `check` e `run` em projeto de texto; janela independente de Qt desenha uma sala de brushes; flycam.

- Workspace Cargo, `petunia-libs`, gates de qualidade.
- `world` (tabelas, ids), `schema` (estendendo o AHS), `commands` (undo).
- Renderer OpenGL mínimo; entrada por ações nomeadas.
- **Demo:** "`run` abre uma sala com 5 brushes; ando com a flycam; `check` valida o projeto sem janela."

### Fase 2 — Editor e Level Builder
**Entrega:** o editor Qt, com brushes.

- Qt/CXX-Qt + viewport OpenGL; hierarquia, inspetor, assets.
- Caixa, rampa, cilindro, gizmos, grade com snap, material por face.
- Undo/Redo, salvar/carregar sem perder ids.
- **Demo:** "Desenho uma sala, uma rampa e um corredor; salvo, fecho, abro."

### Fase 3 — Regras, Script Host e o núcleo do Digi-Tactics
**Entrega:** jogar uma dungeon, montar regras sem código, e **rodar o núcleo do Digi-Tactics em Aipo, sem gráficos**.

- Controlador cinemático; colliders; volumes de gatilho; prefabs **Porta, Pickup, Spawn Point**.
- **Script Host**, schema único, backend **Aipo** (e Luau se S2 aprovar).
- **Gameplay IR + Event Sheet**; "Ver como código".
- **Módulo `grid`** (hex, A\*).
- **Porte do núcleo do Digi-Tactics** (coordenadas, RNG, economia, loja, sinergias, Jogress, combate) com **teste diferencial**.
- **Project Doctor, versão 1.**
- **Demo A:** "Dungeon jogável; `OnInteract → OpenDoor` sem código."
- **Demo B (Prova 1, parte 1):** "Rodo 1.000 partidas por linha de comando e o resultado é **idêntico** ao da versão TypeScript."

### Fase 4 — Personagens, UI de jogo e Digi-Tactics offline  *(Prova 1)*
**Entrega:** a fatia mínima jogável do Digi-Tactics.

- Reprodução de **animação esquelética** (papéis, mistura, estados terminais).
- **Material toon** original e contorno.
- **Pano de fundo 2.5D** com câmera calibrada e perfil de pós.
- **UI de jogo** com arrastar e soltar, dicas, en/pt-BR e acessibilidade.
- Efeitos de habilidade simples (projéteis, partículas).
- **Cozimento de assets** (poda de clipes e buffers).
- **Demo (Prova 1):** "Partida 1v1 offline: compro, posiciono, o combate roda a 20 Hz com criaturas animadas, em português ou inglês, só por teclado se eu quiser."

### Fase 5 — Mundo natural  *(Prova 2, parte 1)*
**Entrega:** terreno, rios, estradas, vegetação, clima, decals, billboards 3D e pós leve.

- Terreno (esculpir, pintar, blocos, colisão).
- Rios (escavação, fluxo, espuma de margem); estradas em rede (cruzamentos, pontes).
- Vegetação por regras com impostores; clima + hora do dia; decals (etapa 1); pós-processamento com conforto sensorial.
- Project Doctor ganha verificações do mundo.
- **Demo:** "Ilha com rio, estrada com cruzamento e ponte, floresta, chuva que molha o chão, lama em decals."

### Fase 6 — Física leve, cordas, navegação e IA  *(Prova 2, parte 2)*
**Entrega:** cordas, corpos simples, waypoints, A\*, Patrol e Chase.

- Corpos simples e consultas (Rapier) com orçamento.
- **Sistema de cordas** (XPBD) e preset de ponte.
- Waypoints, A\*, Patrol, Chase, com depuração visual.
- Áudio com mixer e legendas de som.
- **Demo:** "Atravesso a ponte de corda; um inimigo patrulha e me persegue; corto a corda."

### Fase 7 — Online
**Entrega:** salas por código; servidor com autoridade.

- Réplica de comandos e tabelas, interpolação, predição do próprio personagem.
- Botão **Testar online**; chat por símbolos.
- **Demo (Prova 1, parte 2):** "Digi-Tactics com **2 jogadores** em sala por código (depois, 8)."
- **Demo (Prova 2):** "Dois jogadores atravessam a ponte da Ilha."

### Fase 8 — Intercâmbio, automação e exportação
**Entrega:** integração com o Petunia3D, MCP, depuração e build.

- Importar/reimportar GLB com identidade preservada; **Live Link** com o Petunia3D.
- MCP sem editor aberto (`map.get`, `brush.create`, `trigger.create`, `event.list`, `play.test`).
- Runtime Inspector, depurador de regras, profiler; Project Doctor completo.
- Exportação **Windows e Linux** com runtime independente de Qt. Web: estudo.
- **Demo:** "Modelo no Petunia3D → GLB → aparece na Ilha; um agente cria um brush e testa."

### Fase 9 — Acabamento e lançamento
**Entrega:** teste com pessoas, documentação, modelos iniciais.

- **Teste de acessibilidade com pessoas reais**; correções.
- Modelos iniciais (Ilha, Vila, Floresta, Corredor, Dungeon, Tabuleiro tático).
- Documentação, tutoriais interativos.
- **Demo final:** a **demo alvo** do [resumo executivo](../00-visao/01-resumo-executivo.md) **e** o Digi-Tactics com criaturas originais.

## Dependências entre fases

```text
 F0 ─► F1 ─► F2 ─► F3 ─► F4 ─► F5 ─► F6 ─► F7 ─► F8 ─► F9
                     │     │                │
                     │     │                └─ réplica depende dos comandos (F1) e das tabelas
                     │     └─ UI e personagens são usados também pela Ilha
                     └─ porta o núcleo do Digi-Tactics (carga real da Aipo)
```

## Se você quiser antecipar o mundo natural

Possível, mas **aumenta o risco**: terreno, rios e estradas dependem de `world`, `commands`, renderer e editor. Caminho de menor risco: fazer na **fase 2** só o **terreno** (esculpir e pintar). Registrar em `DR-14`.

## Critério de "pronto" por fase

1. A **demo** roda de ponta a ponta (vídeo ou captura).
2. Os **gates** passam (formatação, lint, testes, documentação, acessibilidade).
3. O **registro de decisões** está atualizado.
4. Cada item tem estado honesto: `passou`, `falhou`, `não rodou` ou `bloqueado` ("entrega verdadeira", `LOCAL`).

## Decisões ligadas

`DR-14` escopo e ordem · `DR-15` gates · `DR-16` provas · `DR-17` UI de jogo.
