# Registro de decisões

**Revisão:** 2026-10-24 (atualizada após ler a [especificação anterior](../01-pesquisa/05-especificacao-anterior.md))

## Resumo (TL;DR)

São **24 decisões**. Estados possíveis:

- **HERDADA:** já decidida na especificação anterior (10/10/2026). Esta documentação **mantém**; só reabre com um ADR.
- **PROPOSTA:** recomendação minha, aguardando você ou um experimento.
- **ABERTA:** falta informação sua.

`DR-01` (licença do código), `DR-02` (filosofia própria + bibliotecas), `DR-20` (Rust) e `DR-21` (ponte do editor) foram **decididas** em 2026-10-24. `DR-22` (níveis 1 e 3) está feita; o nível 2 está em parte; `DR-23` aguarda você. **`DR-11` (nome e relação com os outros projetos) segue aberta, mas não trava o código da fase 1.** As demais podem esperar os experimentos.

## Painel

| ID | Tema | Estado | Recomendação em uma linha |
|---|---|---|---|
| DR-01 | Licenciamento | **DECIDIDA** (reuso livre entre seus projetos); falta só a licença pública | Runtime permissivo; editor GPL |
| DR-02 | Base técnica | **DECIDIDA** (2026-10-24) | **Engine própria + bibliotecas auxiliares atrás de adaptadores** ([Godot](filosofia-propria-vs-godot.md) · [bibliotecas](../02-arquitetura/10-bibliotecas-auxiliares.md)) |
| DR-03 | API gráfica | **HERDADA** (OpenGL leve) | Manter; validar em S3 |
| DR-04 | UI do editor | **HERDADA** (Qt 6 / CXX-Qt) | Manter; medir build em S4 |
| **DR-05** | Modelo de dados | PROPOSTA (compatível com "ECS enxuto") | Tabelas próprias, atrás de interface pequena |
| DR-06 | Linguagens | **HERDADA** (Luau ou Aipo, uma por projeto) | Padrão Aipo; confirmar com S1 e S2 |
| DR-07 | Programação visual | **HERDADA** (Gameplay IR); Event Sheet **trocado por nós** (DR-30) | IR como grafo de nós + grafos especializados |
| DR-08 | Física | **EM PARTE ACEITA** (DR-27) | Cinemática e consultas primeiro; Rapier; XPBD para cordas |
| DR-09 | Online | PROPOSTA (**escopo novo**) | Sala + autoridade do servidor |
| DR-10 | Formatos | **HERDADA** (texto difável; GLB) + detalhes **PROPOSTOS** | Pasta de texto no desenvolvimento; ZIP ao exportar |
| DR-11 | Relação com Petunia3D, Poppy e nome | **ABERTA** | Produtos separados; ponte por GLB e `petunia-libs` |
| DR-12 | Decals | PROPOSTA | Etapa 1 simples; etapa 2 se precisar |
| DR-13 | Pós-processamento | PROPOSTA | AgX + passe fundido + conforto sensorial |
| DR-14 | Escopo e ordem das fases | PROPOSTA | Base primeiro, mundo natural depois |
| DR-15 | Gates de qualidade | PROPOSTA | Pirâmide do Petunia3D + acessibilidade |
| **DR-16** | Prova de conceito (Digi-Tactics + Ilha) | PROPOSTA | Duas provas; fatia mínima; criaturas originais no público |
| **DR-17** | UI de jogo | PROPOSTA | UI própria retida, como dados, com acessibilidade; S9 |
| **DR-18** | Como reusar a GUI do Petunia3D | PROPOSTA | Cópia fotografada agora; biblioteca compartilhada `ui-kit` depois |
| **DR-19** | Idioma da documentação | PROPOSTA | Inglês canônico + pt-BR sincronizado, ao iniciar a implementação |
| DR-20 | Linguagem do núcleo (7 candidatas) | **DECIDIDA** (2026-10-24) | **Rust**; S0 mede de novo a cada fase |
| DR-21 | Ponte entre o editor Qt e o motor | **DECIDIDA** (2026-10-24) | **Qt Widgets + JSON sobre interface C**; CXX-Qt fica para depois |
| **DR-22** | Edição de primitivas e malha para mapas | **Níveis 1 e 3 FEITOS** (nível 3: 2026-10-25, a seu pedido); nível 2 em parte | Brushes paramétricos + malha editável guardada na cena (booleanos, inset, loop cut, bevel…); falta a interface ([análise](../02-arquitetura/13-primitivas-e-edicao-de-malha.md)) |
| **DR-23** | Pintura e animação do Petunia3D | PROPOSTA | Reprodução e IK sim; autoria e pintura de textura não; só o pincel de mundo ([análise](../01-pesquisa/07-pintura-e-animacao.md)) |
| **DR-25** | Script Host: Aipo embutida, sem acesso ao sistema | **ACEITA** (2026-10-25, a seu pedido) | Aipo por dependência de caminho; módulos `sh`/`fs`/`env`/`io` **removidos**; permissões fixas; orçamento por chamada ([doc](../03-linguagens-e-api/05-script-host.md)) |
| **DR-26** | Layout e design da janela do editor | **ACEITA** (2026-10-25, a seu pedido; revista) | Medidas do Unity com piso da WCAG; menus no `≡`; paleta `Ctrl+K`; workspaces Cena e Código ([design](../02-arquitetura/15-design-da-interface.md)) |
| **DR-27** | Colisão e corpos físicos | **ACEITA** (2026-10-25, suas escolhas) | Colisões com nome como entidades; `RigidBody`; Rapier em `petunia_physics`; API `collision.*` e `physics.*` ([física](../02-arquitetura/06-fisica-e-cordas.md)) |
| **DR-28** | Personagem, modo Jogar e solidez automática | **ACEITA** (2026-10-26) | `Character` com tempo de coiote; `F6` roda a cena numa cópia; tudo que é desenhado é sólido; camadas de colisão ([física](../02-arquitetura/06-fisica-e-cordas.md)) |
| **DR-29** | Luzes de cena e importação de modelos GLB | **ACEITA** (2026-10-26, "Fase 2: aprovado") | Tabela `Light` (sol e pontual); `gltf` só em `petunia_import`; modelo vira malhas editáveis ([editor](../02-arquitetura/11-editor.md)) |
| **DR-30** | Sistema de nós (no lugar do Event Sheet) | **ACEITA** (2026-10-26, "tudo aprovado"; N1 implementado) | Grafo tipado 1:1 com a API; compila para Aipo; vista em contorno completa como primeira entrega; tela de nós própria sem biblioteca nova |
| **DR-31** | Prefabs embutidos e o nó Esperar | **ACEITA** (2026-10-26, "prossiga com a implementação"; N4 implementado) | Porta, Item coletável e Ponto de nascimento como moldes de linhas + grafos; `Esperar` compila para uma função por espera |
| **DR-24** | Workspaces e prioridades antes da API | **EM PARTE ACEITA** (andares, cavernas B) | Mapa (com Jogar), Pintura sem UV, Animação, Efeitos por nós; antes da API: andares, cavernas, terreno, prefabs, Jogar ([análise](../02-arquitetura/14-workspaces-e-prioridades.md)) |
| **DR-33** | Sistema de mundos: terreno, cavernas, rios e estradas | **ACEITA** (2026-10-27, suas respostas) | Modelo híbrido (heightfield + SDF esparso em chunks), brushes 3D, dois estilos de mapa (CSG estilo Doom/Quake e orgânico), superfície segura e `surface_follow` opt-in ([terreno](../02-arquitetura/04-terreno-rios-estradas.md)) |

---

## DR-01 — Licenciamento · DECIDIDA (2026-10-24)

**Decisão:** o Petunia3D (GPL) e a Aipo (MIT) são **projetos seus**. O código pode circular entre eles e para `petunia-libs` **sem restrição de direitos autorais**.

**Escolha:** reuso livre de código entre seus projetos.
**Motivo:** você é a única pessoa com os direitos (confirmado por você).
**Reabrir se:** entrar código de terceiros sob GPL, ou contribuições externas.

**Ainda falta (não bloqueia o código):** a **licença pública** do runtime para quem for criar jogos com a engine. A GPL no runtime obrigaria os jogos distribuídos a serem GPL. Recomendação mantida: **runtime e `petunia-libs` permissivos (MIT/Apache), editor GPL**. Veja [licenças](../01-pesquisa/04-licencas.md).

**Cuidado:** a decisão vale para **código**. **Ativos de terceiros** (modelos e shaders do jogo Digimon) são outro assunto: veja `DR-16`.

---

## DR-02 — Base técnica · DECIDIDA (2026-10-24)

**Decisão:** construir a **nossa engine** e usar **bibliotecas auxiliares** onde o assunto é comum e bem resolvido (física, rede, áudio, janela, texto…), sempre **atrás de adaptadores**.

**Escolha:** opção 1 (filosofia própria) com bibliotecas para cortar caminho.
**Motivo:** o pedido conflita com o Godot em três eixos (dados em vez de classes, Aipo de primeira classe, editor redesenhado). Análise: [Godot](filosofia-propria-vs-godot.md).
**Reabrir se:** S4 reprova o Qt e os planos B; a fase 4 estoura o tempo aceito; o objetivo muda para "lançar jogos".

**Como escolher bibliotecas:** regra "construir × comprar" com checklist de 8 perguntas e **registro de dependências**: [bibliotecas auxiliares](../02-arquitetura/10-bibliotecas-auxiliares.md).

**Filosofia de engenharia:** a mesma do Digi-Tactics, traduzida para Rust: [filosofia de engenharia](../00-visao/05-filosofia-de-engenharia.md) e [`AGENTS.md`](../../AGENTS.md).

**Já herdado (`LOCAL`):** Rust; `RuntimeWorld` separado da autoria; crates em `petunia-libs`; "não adicionar framework grande para um caso pequeno".

---

## DR-03 — API gráfica · HERDADA (OpenGL leve)

**Decidido (`LOCAL`):** OpenGL leve, sem depender de compute shaders. O spike com `wgpu` foi "histórico; substituído".

**Esta documentação:** **mantém.** Em uma versão anterior deste registro eu recomendei `wgpu`; **retirei** a recomendação, porque a decisão já existia e os recursos pedidos cabem em OpenGL 3.3 (`HIPÓTESE`).

**Plano B (só com ADR):** `wgpu`, se S3 mostrar bloqueio ou se o Web virar prioridade.

**Regra:** nunca manter dois renderers em paridade (lição do ADR-C11 do Petunia3D).

**Para fechar:** experimento **S3** na sua máquina.

---

## DR-04 — UI do editor · HERDADA (Qt 6 / CXX-Qt)

**Decidido (`LOCAL`):** Qt 6 com CXX-Qt; o runtime nunca depende de Qt.

**Esta documentação:** **mantém.** O experimento **S4** mede o que o Qt custa (build, memória, leitor de tela, editor de código). Planos B só se S4 reprovar: editor web, ou Rust puro com AccessKit.

**Riscos já conhecidos:** build pesado na sua máquina; licença Qt (`DR-01`).

**Revisão (2026-10-27):** o mínimo prático é **Qt 6.4** (Pop!_OS 24.04); não há 6.5+ nos repositórios oficiais. O editor compila e passa o teste de fumaça com 6.4.2, e o código não usa API de 6.5. A ponte é Qt Widgets + JSON (`DR-21`), não CXX-Qt.

---

## DR-05 — Modelo de dados · PROPOSTA

**Opções**
1. **Tabelas próprias** (conjunto esparso), atrás de interface pequena.
2. **`bevy_ecs` sozinho.**
3. **Árvore de cena.**

**Recomendação:** **1.** É um "ECS enxuto", como a especificação anterior pediu. Veja [mundo e dados](../02-arquitetura/02-mundo-e-dados.md).

**Para fechar:** protótipo curto em S1 (consultas em lote e custo por regra).

---

## DR-06 — Linguagens · HERDADA

**Decidido (`LOCAL`):** Luau **ou** Aipo, **uma ativa por projeto na V1**, escolhida ao criar o projeto, pela mesma Script Host API. Backends isolados e versionáveis.

**Esta documentação:** **Aipo é o padrão** (você a quer de primeira classe). Luau é a alternativa. Se S2 mostrar custo alto do Luau, **só Aipo** continua válido.

**Plano B** se S1 reprovar a Aipo em tempo real: veja [riscos](../05-roadmap/02-riscos-e-fora-de-escopo.md).

**Para fechar:** **S1** e **S2**.

---

## DR-07 — Programação visual · HERDADA + detalhes propostos

**Decidido (`LOCAL`):** **Gameplay IR** como representação compartilhada; Event Sheets, grafos e Behaviors são vistas dele; **a interface visual não espelha a AST**; nada de grafo genérico obrigatório.

**Pedido seu:** nós **1:1 com a API**. O desenho atende: cada função do schema é um nó/ação.

**Detalhes propostos**
1. **Event Sheet** como padrão; **grafos especializados** (estados, comportamento); **vista de nós** avançada.
2. **Vista em contorno** equivalente para acessibilidade.
3. **"Ver como código":** IR → Aipo/Luau sempre; editar o código **separa** o bloco do visual.
4. Layout em arquivo ao lado.

**Ponto aberto herdado:** formato físico do IR e ids estáveis.

**Estudo opcional:** ida e volta completa texto ⇄ nós (S5b), depois da v1.

**Para fechar:** **S5** (um Event Sheet real, gerando Aipo e Luau).

---

## DR-08 — Física · PROPOSTA

**Contexto herdado (`LOCAL`):** movimento **cinemático** primeiro; rigidbodies "fora do MVP"; consultas (raycast, overlap, shape cast); camadas.

**Novo:** cordas e física leve.

**Opções**
1. **Cinemática + consultas primeiro; Rapier para o resto; XPBD próprio para cordas.**
2. **Jolt** (ligação Rust).
3. **Tudo próprio.**

**Recomendação:** **1**, em fases: cinemática e consultas (fase de jogabilidade), cordas e corpos simples (fase de física).

**Para fechar:** **S6**.

---

## DR-09 — Online · PROPOSTA (escopo novo)

**Contexto:** a especificação anterior listava "networking massivo" em "não perseguir" (`LOCAL`). O que você pede agora é **pequeno** (sala por código), não massivo.

**Opções (transporte e réplica)**
1. **Réplica própria sobre `renet`.**
2. **`naia`.**
3. **`matchbox` (WebRTC P2P).**

**Recomendação:** **1** no nível 1; **3** como alternativa para web/P2P.

**Perguntas de produto:** quem hospeda o relay? (autohospedado por padrão) · o que acontece quando o anfitrião sai?

**Para fechar:** **S7**.

---

## DR-10 — Formatos · HERDADA + detalhes propostos

**Decidido (`LOCAL`):** formatos de projeto **textuais, difáveis e amigáveis ao Git**; **GLB/glTF** como intercâmbio; IDs persistentes; migração explícita por versão; arquivos de autoria separados de cache e de layout pessoal.

**Detalhes propostos**
1. **Projeto = pasta de arquivos de texto** (Git).
2. **Pacote exportado = ZIP versionado** (como o `.petunia`, `LOCAL`), com limites contra arquivo hostil.
3. **Nomes históricos** (`map.tiny.json`): migrar com compatibilidade, nunca renomear em silêncio.

---

## DR-11 — Relação com Petunia3D, Poppy e nome · ABERTA

**Perguntas para você**
1. **Esta pasta é a retomada de `petunia-game-engine`?** A especificação anterior diz que sim (retomada da linha 3D). Confirmar.
2. A **"Poppy Game Engine"** citada nos documentos da Aipo é outro projeto, ou nome antigo desta engine?
3. **Nome final** (a especificação anterior lista como pendente). "Petunia Engine" é provisório.
4. O **Petunia3D** continua **separado** (exporta GLB; **Live Link** depois) ou vira um modo do editor?
5. Onde **assar impostores** (billboards 3D): Petunia3D ou engine?

**Opções para (4)**
1. **Separados**, ponte por GLB, `petunia-libs` e Live Link. — **recomendada** (é o que a especificação anterior decidiu)
2. **Integrados.**
3. **Separados agora, integrados depois.**

---

## DR-12 — Decals · PROPOSTA

**Opções**
1. **Etapa 1:** projetados após o opaco usando só profundidade; etapa 2 clusterizada se precisar. — **recomendada**
2. **Deferred completo** (exige G-Buffer).
3. **Clusterizados desde o início.**

Pool e fade (herdados, `LOCAL`) valem em todas.

---

## DR-13 — Pós-processamento · PROPOSTA

**Opções (tonemap padrão)**
1. **AgX.** — **recomendada**
2. **Khronos PBR Neutral.**
3. **ACES.**

**Obrigatório:** passe fundido e **conforto sensorial** (limitador de flash, reduzir movimento, perfis).

---

## DR-14 — Escopo e ordem das fases · PROPOSTA

**Contexto:** o escopo cresceu em relação à especificação anterior ([tabela](../01-pesquisa/05-especificacao-anterior.md)). A base antiga (P0) é pré-requisito do mundo natural.

**Proposta:**
1. **Base primeiro** (núcleo, editor, brushes, jogabilidade mínima, regras).
2. **Mundo natural** depois.
3. **Física/cordas, online** em seguida.
4. **Fora de escopo da v1** conforme a [lista](../05-roadmap/02-riscos-e-fora-de-escopo.md).

**Pergunta:** o mundo natural (rios, estradas, clima) é o **diferencial** e deveria vir **antes** de partes da base? Isso é possível (cada fase tem demo), mas **aumenta o risco**.

**Atualização:** o Digi-Tactics pede personagens, UI e grade **antes** do mundo natural (fase 4 antes da 5). Veja a [proposta](../00-visao/03-proposta.md).

---

## DR-15 — Gates de qualidade e acessibilidade · PROPOSTA

**Proposta:** adotar do Petunia3D (`LOCAL`): *never crash* (sem `unwrap`), comandos transacionais, dados hostis, **pirâmide de testes em 6 camadas** (domínio → contratos → GUI headless → fuzz → regressão visual → **acessibilidade**), "entrega verdadeira" (`passou`/`falhou`/`não rodou`/`bloqueado`) e orçamento de recursos do host.

**Acréscimos propostos:** exemplos da documentação rodam nos testes; teste do gerador IR → código; **Project Doctor** nos testes de aceite.

---

## DR-16 — Prova de conceito · PROPOSTA

**Contexto:** você quer um remake do **Digi-Tactics** na engine para provar que ela é utilizável. Análise em [prova de conceito](../07-prova-de-conceito/01-digi-tactics.md).

**Pontos**
1. O Digi-Tactics prova **jogabilidade, animação, UI, determinismo e rede**; **não** prova o mundo natural.
2. Os ativos vêm de um jogo comercial; a redistribuição está **por verificar** (`LOCAL`, README do projeto).
3. O jogo original é **web**; a engine mira Windows e Linux primeiro.

**Opções**
1. **Duas provas:** Digi-Tactics (jogo) + Ilha (mundo natural). — **recomendada**
2. Só o Digi-Tactics.
3. Só a Ilha.

**Regras propostas**
- **Fatia mínima** do remake (tabela no documento) e **teste diferencial** contra o TypeScript (mesma semente, mesmo registro de eventos).
- **Modelos do jogo Digimon:** só como fixtures **locais, fora do Git**.
- **Repositório e demos públicos:** criaturas **originais**, feitas no Petunia3D, com o mesmo layout de esqueleto e animações.
- **Shader toon:** escrever um **original**; não copiar o GLSL recriado.
- **Web:** manter o renderer no subconjunto WebGL2/GLES3 e o runtime sem Qt; exportar depois.

**Para fechar:** você aprovar as regras acima e a fatia mínima.

---

## DR-17 — UI de jogo · PROPOSTA

**Contexto:** o Digi-Tactics tem HUD complexo (loja, banco, arrastar e soltar, dicas, en/pt-BR). A engine ainda não tem UI de jogo. Veja [personagens, UI e grade](../02-arquitetura/09-personagens-ui-e-grade.md).

**Opções**
1. **UI própria retida, descrita como dados**, com árvore de acessibilidade. — **recomendada**
2. **Reaproveitar** `aipo-ui` / `aipo-egui` / Zoe (adequação a verificar).
3. **Modo imediato** (estilo egui).

**Para fechar:** experimento **S9**.

---

## DR-18 — Como reusar a GUI do Petunia3D · PROPOSTA

**Contexto:** a GUI tem ~27,5 mil linhas de C++ (Qt Widgets): ~10,1 mil genéricas, ~13,2 mil adaptáveis, ~4,2 mil de modelagem. Análise em [cópia do Petunia3D](../01-pesquisa/06-copia-do-petunia3d.md).

**Opções**
1. **Cópia fotografada agora, biblioteca compartilhada depois (`petunia-libs/ui-kit`).** — **recomendada**
2. **Biblioteca compartilhada desde o início** (o Petunia3D passa a depender dela já).
3. **Só cópia**, sem nunca unificar (as duas GUIs divergem).

**Por que 1:** o Petunia3D ainda está em integração (branch não enviado, ondas de acabamento planejadas). Copiar primeiro não arrisca o projeto atual. Unificar depois evita divergência.

**Cuidados:** a acessibilidade da GUI **não está certificada** (S8 antes); ícones sem procedência documentada não entram.

---

## DR-19 — Idioma da documentação · PROPOSTA

**Contexto:** o Digi-Tactics usa **inglês canônico + pt-BR sincronizado** com identificadores de página iguais (`LOCAL`). Nossa documentação atual está **só em pt-BR** (rascunhos de discussão). O novo `AGENTS.md` já nasce em inglês.

**Opções**
1. **Inglês canônico (`docs/en`) + pt-BR (`docs/pt-BR`)**, referência da API **gerada do schema** nos dois idiomas, ao iniciar a implementação. — **recomendada**
2. **Só pt-BR.**
3. **Só inglês.**

**Custo:** a opção 1 dobra a manutenção de prosa; a geração a partir do schema reduz isso para a referência da API. O `xtask docs-check` verifica a paridade.

---

## DR-20 — Linguagem do núcleo · DECIDIDA (2026-10-24)

**Decisão:** o núcleo continua em **Rust**. A implementação da fase 1 começou.

**Escolha:** opção 1 (manter Rust e reduzir o custo de build).
**Motivo:** nenhuma alternativa vence em todos os critérios; a Aipo e o Luau integram bem em Rust; a medição inicial mostrou que o build **não é gargalo** hoje.

**Evidência (S0 parcial, na sua máquina):** workspace inteiro com janela e OpenGL em **27,7 s** do zero; build incremental sem janela em **3,3 s**; 212 pacotes no `Cargo.lock`. Detalhes em [estado da implementação](../05-roadmap/03-estado-da-implementacao.md).

**Análises:** [comparativo de 7 linguagens](linguagem-do-nucleo-comparativo.md) e [porte para Odin](odin-vs-rust.md).

**Reabrir se:** o tempo de build crescer de forma inaceitável depois de entrarem Qt, `mlua`, `rmcp` e Aipo (S0 repetido a cada fase); S10 (Odin) ou S11 (C#) trouxerem ganho grande sem perder Luau e Aipo; o objetivo mudar para "web primeiro".

---

## DR-21 — Ponte entre o editor Qt e o motor · DECIDIDA (2026-10-24)

**Decisão:** a primeira versão do editor usa **Qt Widgets em C++** e conversa com o motor por uma **interface C pequena que troca JSON** (`petunia_editor_ffi`). O **CXX-Qt não entra agora**.

**O que isso muda na [DR-04](#painel):** nada no essencial. O editor continua em Qt 6 e a janela continua atrás de um adaptador. Muda só a técnica de ligar Rust e C++.

**Motivos:**
- **A mesma porta serve tudo.** A janela, os testes automáticos e, no futuro, agentes (MCP) usam as mesmas operações JSON. Não existe segundo caminho para testar.
- **Menos peças no build.** O `cxx-qt-build` soma tempo de compilação e versões acopladas ao Qt. A fronteira em C é estável e de poucas funções.
- **Verificável sem a janela.** A lógica do editor tem testes em Rust, sem Qt.
- **Cabe no princípio do projeto.** Operações como dado, validadas, que devolvem erro com código e ação.

**Custo:** escrevemos a ponte à mão (7 funções C e o `EngineBridge`), e `unsafe` aparece em mais um crate (`petunia_editor_ffi`, isolado como o adaptador OpenGL).

**Evidência:** a janela abriu, desenhou, selecionou por clique, editou, desfez e salvou num display virtual. Veja [editor](../02-arquitetura/11-editor.md) e [estado da implementação](../05-roadmap/03-estado-da-implementacao.md).

**Reabrir se:** precisarmos de propriedades reativas ou de QML com ligação direta a objetos Rust, ou se o `EngineBridge` virar gargalo de manutenção. A troca fica restrita a `editor/` e a `petunia_editor_ffi`, porque as operações JSON não mudam.

---

## DR-22 — Edição de primitivas e malha para construir mapas · níveis 1 e 3 FEITOS

**Nível 1 (decidido e feito em 2026-10-24):** **brushes paramétricos** (caixa, rampa, cilindro, cone, esfera, escada) com gizmo de mover, girar e escalar, snap à grade e desfazer por gesto.

**Nível 2 (em parte):** empurrar/puxar, extrudar, cavar e tornar oco existem. Faltam cortar com plano, arranjo e espelhar.

**Nível 3 (decidido por você em 2026-10-25).** Pedido: "falta você trazer do petunia3d operações booleanas, inset, loopcut e etc…". Respostas à pergunta de modelo de dados:

- **Malha:** "A: malha pronta". Cada operação troca a malha inteira, e o `Ctrl+Z` restaura a anterior. A variante "A + booleano vivo depois" **não** foi escolhida.
- **Biblioteca:** "Sim, aprovo" o `manifold-rust` 0.13.1. O `glam` fica só dentro do crate, nunca na API nem nos arquivos.

| Ponto | Escolha | Motivo |
|---|---|---|
| O que trazer | Subconjunto do crate `mesh` do Petunia3D **com** booleanos; **sem** UV, desenho (`poly_pen`, curvas, varredura) e `shape_ops` | O pedido foi booleanos, inset, loop cut "e etc."; UV e desenho não pesam em construir mapas |
| Como guardar | **A malha pronta** numa tabela `Mesh` (tipo de valor `mesh`), não a lista de operações | O que você vê é o que fica salvo; mudar um algoritmo nunca altera um trabalho antigo; o desfazer comum já funciona |
| Formato no arquivo | Uma linha de texto por vértice e por face | Diferenças legíveis (DR-10) |
| Booleanos | `manifold-rust` 0.13.1 (Rust puro, Apache-2.0, sem `unsafe`), a mesma do Petunia3D | Booleano robusto em malhas é um projeto de meses; é assunto comum, atrás de adaptador (checklist do `AGENTS.md`) |
| Matemática do código copiado | `glam` 0.27.0 **só dentro** de `petunia_mesh` | O código copiado usa `glam`; reescrever 10,9 mil linhas para trocar o tipo traria risco sem ganho. `glam` nunca aparece na API, no schema, nos arquivos nem na rede |
| Código copiado | Módulo `legacy/`, quase sem mudanças, origem no `ATTRIBUTION.md` | Regra de legado: migra nomes e índices quando for tocado, com teste antes |

**Proteções que vieram junto:** a cena pode ter até 64 MB, e salvar uma cena maior é recusado (antes, salvava e depois não abria); o histórico de desfazer tem teto de 256 MB.

**Falta:** a interface Qt (menu Malha, modo de edição com `Tab`, atalhos). Lista em [ganchos da interface](../02-arquitetura/13-primitivas-e-edicao-de-malha.md#ganchos-da-interface-que-faltam).

**Reabrir se:** as malhas ficarem grandes demais para o arquivo de texto (aí, um arquivo binário por malha); se precisarmos de booleano "vivo" (mover o cortador e o buraco ir junto), que pediria guardar as operações; ou se o `manifold-rust` parar de ser mantido.

---

## DR-23 — Pintura e animação do Petunia3D · PROPOSTA

**Recomendação:**

| Capacidade | Veredito |
|---|---|
| Reprodução de animação de esqueleto e IK | **Trazer** (núcleo de ~3,8 mil linhas), na fase de personagens |
| Animação de propriedades (portas, luzes, cinemáticas) | **Fazer novo**, pequeno |
| Pincel de mundo (terreno, cor de vértice, decals) | **Trazer só o pincel**, na fase 4 |
| Pintura de textura em modelos, editor de esqueleto e poses | **Não trazer**; importar GLB |

**Motivo:** pintura e autoria já são o trabalho do Petunia3D; trazê-las puxa `mesh`, UV e o crate `project` (o maior salto de dependências da engine). Reprodução é indispensável para os personagens do Digi-Tactics.

Números, opções e o que mudaria a recomendação: [pintura e animação](../01-pesquisa/07-pintura-e-animacao.md).

---

## DR-24 — Workspaces e prioridades antes da API · EM PARTE ACEITA

**Proposta:** cinco workspaces sobre a mesma cena (Mapa com Jogar, Pintura, Animação, Efeitos) e, antes da API, da física e do editor de código, as ferramentas de montagem de mapa: andares completos, cavernas por volume esculpido, terreno, prefabs, modo Jogar, importação de GLB, cor por face e luz básica.

**Motivo:** essas ferramentas mudam o formato dos dados; fazê-las depois da API obrigaria a mudar a API.

**Aceito (2026-10-25):** andares completos e **cavernas pela opção B** (volume esculpido). Base da aceitação: depois da pergunta "opção B?", você respondeu "continue com a implementação"; tratei isso como sim à recomendação. Se não era, as cavernas ficam isoladas no crate `petunia_sculpt` e em duas tabelas, e saem sem tocar no resto.

**Ainda proposta:** os workspaces (Pintura, Animação, Efeitos) e a ordem dos itens 3 a 8.

**Já feito:** andares completos, tornar oco e cavar caixas, **cavernas completas** (blocos de 16³, 5 modos de pincel, caminho, gerador, fundir traços, facetada). Medido: uma pincelada passou de 54 a 1063 ms para ~1 ms. Detalhes: [workspaces e prioridades](../02-arquitetura/14-workspaces-e-prioridades.md).

---

## DR-25 — Script Host: Aipo embutida · ACEITA (2026-10-25)

**Decisão:** o Script Host executa scripts **Aipo** (a linguagem padrão do DR-06), embutida pelas crates `aipo-*` como **dependências de caminho** (`../aipo-lang`, projeto seu, MIT; `DR-01` permite o reuso). Luau segue adiado para o experimento S2.

**Regras (todas com teste):**

1. A VM de um script é montada por **lista de permitidos**: `sh`, `fs`, `env` e `io` são **removidos**.
2. Permissões fixas (`world.read`, `world.write`, `game`); nada vem do sistema.
3. Orçamento de 100 mil instruções por chamada; uma falha desliga só o objeto afetado.
4. Toda mudança do mundo é um **comando** do despachante.
5. O estado do jogo chega às funções nativas por um handle da VM, sem variável global.

**Por que a lista de permitidos:** ao ler o código da Aipo achei que `sh.run` (executa programas do sistema) **não passa por checagem de capability**, e que `register_stdlib` o registra. Embutir a biblioteca padrão sem remover o módulo daria a qualquer script de mod a execução de comandos na máquina do jogador. Vale avisar o projeto da Aipo.

**Contra:** o build do motor passa a depender de uma pasta irmã (`../aipo-lang`) e compila mais ~70 mil linhas. **Reverter:** o crate `petunia_script` é uma folha; sem ele, o motor volta a funcionar sem scripts.

---

## DR-26 — Layout e design da janela do editor · ACEITA (2026-10-25, revista no mesmo dia)

**Pedidos:** "continue mexendo na ui/ux/design do GUI conforme as referências" e, depois, "campos desnecessariamente grandes… textos demais… ícones grandes… menu hambúrguer… barra de comandos… workspaces, com o editor de código numa aba… busque um design system".

**Decisão** (regras e fontes em [design da interface](../02-arquitetura/15-design-da-interface.md)):

1. **Medidas do Unity Foundations com piso da WCAG**: texto de 12 px, ícones de 16 px, controles de 24 px (alvo mínimo da WCAG 2.5.8); densidade Confortável para quem quer mais.
2. **Menus atrás de `≡`** (JetBrains New UI), com `Alt+letra`, `F10` e a opção "Menus por extenso".
3. **Paleta de comandos** (`Ctrl+K`) feita a partir dos menus: todo comando com menu aparece nela.
4. **Workspaces Cena e Código**; cada um guarda o arranjo dos painéis. O código deixa de ser uma aba de baixo.
5. **Ferramentas numa coluna no viewport**; opções da ferramenta (eixos, pivô, snap) na faixa da cena; snap num botão com painel.
6. **Painéis fixos** em cartões com cantos de 8 px; neutros quentes e um acento violeta; borda de campo com 3:1.
7. **Sem botão Jogar** até existir o modo Jogar.

**Por quê:** a primeira versão desta decisão ainda tinha campos e texto grandes. As medidas agora vêm de design systems publicados, não de gosto.

**Sem biblioteca nova:** folhas de estilo do Qt bastam. **Copiado** do Petunia3D: o controle segmentado; **adaptada**: a paleta de comandos.

**Reabrir se:** os testes com pessoas (S8) mostrarem que ícones sem texto, menus atrás do `≡` ou 12 px atrapalham.

---

## DR-27 — Cubo orbital e projeções da câmera · ACEITA (2026-10-25)

**DECLARADO:** pedido de arrastar o cubo global, alternar projeção e acrescentar a vista de games 2:1.

**Decisão:** arrasto acima de 4 pixels orbita sem executar o clique ao soltar; faces, arestas, cantos, bússola e início continuam disponíveis. O menu e o rótulo alternam perspectiva/ortográfica e oferecem dois presets ortográficos: isométrico verdadeiro (azimute 45°, elevação 35,264389°) e games 2:1 (azimute 45°, elevação 30°). **GERAL:** no segundo, `atan(sin(30°)) = atan(0,5) = 26,565051°` nas diagonais da tela; 26,565° não é a elevação da câmera.

**Contrato:** operações validadas `camera.orbit {dx, dy}`, `camera.projection {mode}` e `camera.view {name: isometric | game_2_1}`; `camera.get` e `viewport.overlay.camera` expõem a projeção. A troca de projeção preserva o tamanho no plano de foco; picking e gizmos usam a mesma matriz; zoom ortográfico altera a altura visível. Estado somente da sessão, sem alterar a cena ou o histórico de desfazer. Sem dependências novas.

**Reabrir se:** forem necessárias câmeras de jogo persistentes ou projeções oblíquas; esses recursos não estão incluídos neste pedido.

---

## DR-28 — Seleção de componentes e correção da extrusão · ACEITA (2026-10-25)

**DECLARADO:** selecionar arestas e vértices dos objetos e corrigir a extrusão de brushes (`E`). Confirmado: modo de edição por `Tab`, com `1` vértices, `2` arestas e `3` faces.

**Decisão:** reutilizar `mesh.convert`, `mesh.pick`, `mesh.select` e o destaque de componentes existentes. Entrar na edição de um brush exige conversão explícita e reversível para malha; sair não desfaz a conversão. Seleção é estado da sessão, sem passos no histórico; clique substitui, `Shift` soma e `Ctrl` alterna. Controles visíveis acompanham os atalhos. Não inclui transformação de componentes nem novas operações de modelagem.

**Correção:** extrudar um brush deve ser um único gesto reversível: cancelar ou clicar sem arrastar não deixa um objeto; confirmar gera um único passo de desfazer. Corrigir somente defeitos reproduzidos com testes, sem redefinir silenciosamente a extrusão de brush como edição de malha.

**LOCAL:** infraestrutura de seleção e destaque em `crates/petunia_editor/src/mesh_tool.rs`; a ferramenta atual cria o brush em `face_drag_begin` antes da confirmação. Evidência de execução será registrada após os testes. Sem dependências novas.

---

## DR-27 — Colisão e corpos físicos · ACEITA (2026-10-25)

**Pedido:** "sistema de colisão… redimensionarmos e editarmos a colisão… múltiplas colisões nomeadas no objeto… acessadas via API de maneira fácil".

**Suas escolhas** (perguntadas antes de implementar): **"Filhos nomeados"** (formato), **"Também malha convexa já"** (formas), **"Já com corpos físicos"** (comportamento).

**Decisão:**

1. Cada colisão é **uma entidade** com a tabela `Collider` (`owner`, `name`, `shape`, centro, rotação, tamanho, raio, altura, gatilho, atrito, quique). Um objeto pode ter quantas quiser. É o mesmo padrão dos traços de caverna.
2. Corpo físico na tabela `RigidBody` do próprio objeto; sem ela, um objeto com colisões é estático.
3. Motor: **Rapier 0.36.1** com determinismo reforçado, só dentro de `petunia_physics` (DR-08 já recomendava). A validação (números finitos, tamanhos, volume da convexa, limites) é **nossa**, antes do Rapier.
4. Scripts: `collision.list/get/set/overlaps`, `physics.raycast/raycast_from/velocity/set_velocity/impulse`, `on_collision` e `on_collision_end`.

**Reabrir se:** o experimento S6 mostrar que o Rapier não cabe no orçamento, ou se camadas de colisão pedirem outro formato.

---

## DR-28 — Personagem, modo Jogar e solidez automática · ACEITA (2026-10-26)

**Pedido:** "prossiga com pendências soltas e fases 1 a 6" (primeiro bloco: fim da fase 3).

**Decisão:**

1. **Tudo que é desenhado é sólido**, sem configurar: caixa → caixa; rampa, escada, cilindro, malha e caverna → superfície exata de triângulos; com corpo dinâmico → casco convexo; personagem → cápsula. Quem quiser controle põe colisões próprias (DR-27); onde há uma, a automática não entra.
2. **Tabela `Character`**: `speed`, `jump_speed`, `step_height`, `max_slope_degrees`, `coyote_seconds` (**tempo de coiote**, ajuda de acessibilidade), `gravity_scale`, `player`. O corpo é cinemático; o movimento usa o controlador de personagem do Rapier (desliza em paredes, sobe degraus e rampas, cola no chão ao descer).
3. **Modo Jogar** (`F6`, `Esc` para): roda a cena numa **cópia** do mundo com scripts e física; parar descarta a cópia. Sem personagem do jogador, nasce um **herói de reserva** no `PlayerStart`, só na cópia. Enquanto joga, o motor recusa qualquer edição (`PLAY_ACTIVE`).
4. **Camadas e máscaras** (`layer`, `mask`) em cada colisão.
5. Scripts: `character.move/jump/grounded`; colisão e física recebem mais efeitos (apagar um objeto leva as colisões; mudar `Collider` ou `RigidBody` refaz a física).
6. `petunia run` usa o mesmo caminho: herói garantido, câmera atrás do jogador.

**Por quê:** sem isso o motor montava cenas mas não deixava andar nelas. A solidez automática evita o "atravessei o chão" na primeira tentativa.

**Reabrir se:** o custo das superfícies exatas de cavernas grandes passar do orçamento (experimento S6).

## DR-29 — Luzes de cena e importação de modelos GLB · ACEITA (2026-10-26)

**Pedido:** "Fase 2: aprovado" (painel de assets, luz básica, importação GLB com a biblioteca `gltf`).

**Decisão:**

1. **Luz** é a tabela `Light` (`kind`: `point` ou `sun`, cor, intensidade, alcance, ambiente). Sem nenhuma luz na cena vale o sol e o ambiente padrão; **com uma luz, só as luzes da cena valem**. Até 8 luzes pontuais por quadro; o renderizador calcula no fragmento. Sem sombras nesta etapa.
2. **Importar modelo** (`asset.import`, `asset.list`): um `.glb`/`.gltf` vira **malhas editáveis** (tabela `Mesh`, DR-22), um objeto por nó com geometria, com posição, rotação, escala e cor base. Tudo entra em **um passo de desfazer**. Uma **cópia** do arquivo fica em `assets/`, para o projeto guardar de onde veio.
3. **Biblioteca `gltf` 1.4.1**, só dentro do novo crate `petunia_import`; os tipos dela nunca saem. Texturas, animações, esqueletos e câmeras ficam de fora e voltam como **aviso**, não como falha silenciosa.
4. **Dados hostis:** limite de bytes do arquivo, de nós, de profundidade e de vértices; só números finitos; arquivos externos de um `.gltf` são lidos apenas da pasta do próprio modelo; links simbólicos não são seguidos ao listar `assets/`.

**Por quê:** sem luz de cena o jogo só tinha o sol padrão; sem importação, todo cenário precisava ser feito dentro do editor.

**Fica de fora (declarado):** material por face (exige cor por face em `PolygonMesh`), texturas e alças de redimensionar colisão no viewport.

**Reabrir se:** modelos grandes passarem do limite de vértices no uso real, ou se texturas virarem requisito.

## DR-30 — Sistema de nós · ACEITA (2026-10-26)

**Pedido:** "decidimos por sistema de nós no lugar de event sheet"; "se necessário, libs externas… o mais completo, mais fácil e mais acessível".

**Contexto (`LOCAL`):** o schema da API já existe como dados tipados (`ApiFunction` em `crates/petunia_script/src/api.rs`: módulo, nome, parâmetros com tipo, retorno, permissão, exemplo) e o Script Host já compila Aipo com validação antes de jogar. Os manipuladores (`on_start`, `on_update`, `on_event`, `on_collision`, `on_collision_end`) também são uma tabela. Isso torna "1 nó = 1 função da API" **gerável**, sem catálogo escrito à mão.

### Decisão proposta

1. **Documento.** Um grafo é um arquivo de texto `scripts/<nome>.graph.json` (a mesma pasta dos scripts, para a tabela `Script` achar os dois do mesmo jeito): `schema_version`, `nodes` (`id` estável, `kind`, `params`), `edges` (de `nó.pino` para `nó.pino`). **Posições ficam em arquivo ao lado** (`<nome>.layout.json`), para o diff da lógica não sumir sob mudanças de layout.
2. **Tipos de nó.** Todos vêm do schema ou de um conjunto pequeno e fixo:
   - **Evento** (começa uma cadeia): um por manipulador (Ao começar, A cada quadro, Ao receber evento, Ao tocar, Ao parar de tocar).
   - **Ação** (função da API com efeito): tem pino de execução; ex. `world.set`, `character.move`.
   - **Valor** (função da API sem efeito): só pinos de dados; ex. `world.get`, `physics.raycast`.
   - **Fluxo**: Se/Senão, Sequência, Repetir N vezes, Para cada item.
   - **Operadores**: comparar, lógica (e/ou/não), matemática, texto, constante.
3. **Execução e fios.** Dois tipos de pino: **execução** (uma cadeia, de cima para baixo) e **dados** (valores). Ao adicionar um nó "depois" de outro, a execução liga **sozinha**; fios de execução quase nunca precisam ser desenhados. Isso evita o espaguete que o `04-programacao-em-nos.md` já apontava.
4. **Tipos nos pinos.** `Int`, `Float`, `Bool`, `Text`, `Vec3`, `Entity`, `List`. Só liga o que combina; `Int` → `Float` é a única conversão automática. Recusa vem com **a razão em frase**.
5. **Compilação.** O grafo vira **texto Aipo** (formato canônico, sempre o mesmo para o mesmo grafo), com **mapa de origem** (linha → id do nó). O Script Host passa a aceitar um `.graph.json` onde aceita um `.aipo`; compila **em memória** a cada carga (nunca fica arquivo gerado velho). **Ver como código** mostra esse texto. Editar o texto **separa** o bloco do visual (aviso claro); o inverso não existe.
6. **Erros do compilador e falhas em jogo** voltam como **"nó X, pino Y: frase"** usando o mapa de origem; em Jogar, o nó que executou fica destacado.
7. **Tudo é comando.** `graph.create/add_node/remove_node/connect/disconnect/set_param/insert_after/set_layout`, mais consultas `graph.catalog/validate/explain`. Cada edição é desfazível e um gesto composto é **um** passo (como `asset.import`).
8. **Dados hostis.** Limites herdados do `op_graph` do Petunia3D: 512 nós, 2048 arestas, profundidade, tamanho de nomes e textos; só números finitos; ids únicos; ciclos de **dados** recusados; nó de tipo desconhecido vira **nó-fantasma** preservado, com aviso (uma ação removida da API não apaga o grafo).

### Acessibilidade (o centro desta decisão)

| Exigência | Como |
|---|---|
| **Tudo sem mouse** | A **vista em contorno** é uma segunda interface **completa** do mesmo grafo: árvore `QUANDO … FAÇA` navegável por teclado e leitor de tela. Ela **mantém o que o Event Sheet tinha de bom** (lista legível) sem ser outro sistema |
| Adicionar nó | `Ctrl+K`-estilo: digite o nome em português (com sinônimos); só aparece o que combina com o pino selecionado |
| Ligar | **Clique-clique ou teclado**: escolher o pino de saída, navegar só pelos destinos compatíveis, Enter. **Sem arrastar** como único caminho |
| Cor nunca sozinha | Cada tipo tem **cor + forma + rótulo**; nó e pino têm nome falado completo ("Ao tocar. Saída *Então* ligada a *Tocar som*") |
| Conforto | Zoom até 400 %, contraste alto, **sem animação** quando o sistema pede, texto do nó no tamanho da interface |
| Erros | Frase simples, com atalho para ir ao nó |

### Bibliotecas externas (avaliadas, nenhuma adotada)

| Opção | O que é | Veredito |
|---|---|---|
| **Tela própria sobre `QGraphicsView`** (Qt, já no projeto) | Canvas de nós escrito por nós, com acessibilidade pensada desde o início | **Recomendada.** Sem dependência nova; ~1,5–2,5 mil linhas de C++ estimadas (**HIPÓTESE**) |
| `QtNodes` (paceholder/nodeeditor) | Editor de nós pronto para Qt | **Não.** Modelo feito para fluxo de dados, não para execução + dados; itens gráficos sem acessibilidade nativa; atar o grafo ao modelo dele (`GERAL`, de memória, **não pesquisado agora**) |
| `imnodes` / Dear ImGui | Nós em modo imediato | **Não.** Outro sistema de interface dentro do Qt; sem acessibilidade |
| `petgraph` (Rust) | Algoritmos de grafo | **Não.** Para ≤512 nós a ordenação e a detecção de ciclo são ~60 linhas; o resultado precisa ser **determinístico** e idêntico (AGENTS §2) |

Se uma biblioteca for necessária mais tarde, passa no checklist de admissão antes.

### Ordem de entrega

1. **N1 — modelo + compilador** (Rust, sem tela): documento, tipos, validação, grafo → Aipo com mapa de origem. Testes, incluindo arquivos hostis e saída idêntica para o mesmo grafo.
2. **N2 — comandos + vista em contorno** (Qt): primeira interface **já acessível**, útil antes da tela de nós.
3. **N3 — tela de nós** (QGraphicsView) com teclado e leitor de tela.
4. **N4 — prefabs como grafos**: Porta, Pickup, Spawn, para provar o sistema.
5. **N5 — depuração:** destaque do nó em execução e "por que esta condição falhou?".

### Perguntas abertas: respondidas

1. **Estado entre chamadas — resolvido.** A Aipo aceita `var` no nível do módulo, **mas o motor de scripts é um por arquivo, compartilhado por todos os objetos que usam o script**. Por isso a variável de um nó é guardada num dicionário do módulo **com o objeto na chave** (`graph_state`). Vale enquanto o jogo roda; não entra no desfazer nem no arquivo da cena. Variáveis que precisam **sobreviver ao salvar** (progresso de jogo) ficam para o sistema de saves. A tabela `GraphState` proposta antes **não** foi criada.
2. **Esperar/tempo:** fica para o passo 4 (máquina de estados gerada), como aprovado.
3. **Rótulos:** português primeiro; títulos e rótulos dos nós vêm de uma tabela no código (`catalog.rs`), prontos para trocar por uma tabela de idiomas.

### Mudanças feitas ao implementar o N1 (e por quê)

| Proposta | Feito | Motivo |
|---|---|---|
| Arquivo em `logic/` | `scripts/` | A tabela `Script` e o carregador já olham ali |
| Tipo `Vec3` | `Dict` + nós **Montar vetor** e **Parte do vetor** | A API de hoje usa `Dict` para vetores; o 1:1 fica fiel |
| Nó **Sequência** | Fica de fora | Ligar um nó depois do outro já é uma sequência; seria um nó redundante |
| Valores de ações lidos por nós de outro caminho | Recusados com `VALUE_NOT_READY` | O valor só existe depois que o nó roda, no mesmo caminho |
| Cada nó "puro" lido várias vezes | Escrito **na linha onde é usado** (como fórmula de planilha); limite de 2000 partes por conta | Mesma semântica que Blueprint; evita crescer sem limite |
| Execução: cada nó roda depois de **um só** | `EXEC_MERGE` / `EXEC_FANOUT` recusam duas ligações | Mantém o código gerado em blocos (`if`/`repeat`/`each`) |

**Reabrir se:** a vista em contorno não bastar para quem usa leitor de tela (teste com uma pessoa real antes do N3), ou se a tela própria passar de 3 mil linhas.

## DR-31 — Prefabs embutidos e o nó Esperar · ACEITA (2026-10-26)

**Pedido:** "prossiga com a implementação" (passo N4 do DR-30).

**Decisão:**

1. **Prefab = molde de linhas + grafos.** Criar um prefab (`prefab.spawn`) cria os objetos numa única ação de desfazer, na frente da câmera, no andar atual, e põe na pasta `scripts/` os grafos de que ele precisa. **Um grafo que já existe com o mesmo nome nunca é sobrescrito** (a pessoa pode tê-lo editado). O comportamento é, portanto, um grafo comum que se abre e muda na aba Lógica.
2. **Três moldes embutidos**, em `crates/petunia_editor/src/prefabs.rs`:
   - **Porta:** objeto cinemático com uma colisão sólida e um sensor maior; sobe 3,2 m quando um personagem entra no sensor, espera 3 s e desce; a variável `aberta` impede abrir duas vezes. Grafo `porta`.
   - **Item coletável:** esfera com sensor; some e avisa no console quando um personagem a toca. Grafo `item`.
   - **Ponto de nascimento:** objeto com a tabela `PlayerStart` e uma marca que não bloqueia. Sem grafo.
3. **Nó Esperar (`flow.wait`).** Compila para: `graph_wait_start(entity, id, segundos)` no ponto da espera e **uma função `graph_after_<id>(entity)` com o que vem depois**, chamada quando o tempo acaba. Cada objeto conta as suas esperas (chave pelo objeto). O contador desce em `on_update`; se o grafo não tem esse evento, o compilador cria um. Regras: depois do Esperar **só o próprio objeto continua disponível** (outros valores dão `VALUE_NOT_READY`); esperar de novo antes do fim **reinicia** a contagem; dentro de repetição dá aviso `WAIT_IN_LOOP`.
4. **Defeito achado pelos prefabs e corrigido:** o motor **nunca avisava** que um personagem (cinemático) tocou um sensor ou uma parede estática, porque o Rapier ignora por padrão pares cinemático × fixo. Agora todas as colisões ligam `KINEMATIC_FIXED` e `KINEMATIC_KINEMATIC`. Sem isso, `on_collision` não disparava para o jogador (teste de regressão que falha sem a correção).

**Fora desta etapa (declarado):** prefabs do próprio projeto, com revisão e instâncias que seguem o molde; porta com animação (hoje ela só sobe e desce de uma vez); "spawn" que **cria** criaturas (hoje é só o ponto de nascimento do jogador); item que conta pontos (variáveis valem por objeto e só enquanto o jogo roda).

**Reabrir se:** os moldes precisarem de variantes por projeto antes do sistema de pacotes, ou se a contagem de esperas por objeto pesar com muitos objetos.

## DR-32 — Tela de caixas e fios e depuração (N3 e N5) · ACEITA (2026-10-26)

**Pedido:** "prossiga com a implementação" (passos N3 e N5 do DR-30).

**Decisão:**

1. **A tela de nós é uma vista, não outra interface.** `QGraphicsView` desenha o mesmo grafo que o contorno mostra (`graph.view.canvas`), e **toda ação dela passa pelas mesmas operações** (`graph.add`, `graph.connect`, `graph.remove`, `graph.set_layout`). Nada é alcançável só com o mouse: Tab e setas andam entre as caixas, Enter edita, Delete remove, `+` `-` `0` e Ctrl+roda dão zoom (25 % a 400 %). O contorno continua sendo **a** interface acessível: cada caixa é um item gráfico sem árvore de acessibilidade nativa, então o canvas **não finge** ser um; ele anuncia a caixa escolhida numa frase (título, resumo, problemas, "rodou no último quadro", "esta condição deu falso") pelo mesmo canal de mensagens do painel. Quem usa leitor de tela trabalha no contorno.
2. **Posições em `scripts/<nome>.layout.json`**, como o DR-30 já previa: arrastar uma caixa grava **só** esse arquivo e **não** entra no histórico de desfazer (posição não é lógica). Grafo sem layout abre com posições **calculadas** a partir das ligações (eventos na coluna 0, o que eles rodam à direita; coluna 300, linha 180), determinísticas, para um teste poder comparar bytes. Layout quebrado **nunca impede abrir o grafo**: é ignorado, as caixas são calculadas e a barra de estado diz por quê (`graph.view.layout_problem`). Nós removidos têm a posição podada do arquivo.
3. **Marcas de depuração são opcionais no compilador.** `compile_with(document, catalog, debug)`: com `debug: false` o texto é **byte a byte igual** ao de `compile` (teste que compara os dois), então nada muda para o jogo nem para o arquivo salvo. Com `debug: true` cada nó que roda chama `graph_debug.node`, cada `Se` chama `graph_debug.check` com o que decidiu, e cada valor que a condição usou chama `graph_debug.value`. O módulo `graph_debug` **não** entra em `API`: não aparece na paleta de nós, não é coisa que uma pessoa escreve; ele é registrado no host e somado à superfície do compilador.
4. **A depuração é uma consulta, não um modo novo de rodar.** `play.start` liga as marcas (`set_debug_marks(true)`) — custo pequeno e limitado (últimos quadros, texto truncado) — e `play.debug` devolve o que o último quadro reportou (`tick`, `marks` com `ran`/`check`/`value`) mais `playing`. A janela consulta a cada 200 ms enquanto o jogo roda (um `QTimer`, não o laço do viewport: 200 ms é o suficiente para ler e não compete com o desenho), pinta as caixas que rodaram e, para "por que esta condição falhou?", lista as condições que deram **falso** com os valores lidos; Enter na linha leva ao nó.
5. **A consulta continua respondendo com o jogo parado** (`playing: false`, nada marcado), para a janela não precisar de estado próprio.

**Fora desta etapa (declarado):** ler as marcas de quadros anteriores (só o último), depurar passo a passo ou com ponto de parada, marcas no jogo exportado, e editar o grafo dentro do canvas com o mouse além de arrastar, clicar e ligar dois pinos.

**Não verificado:** leitor de tela no canvas (o contorno tem texto e descrição falados; **não ouvi** nenhum dos dois), e o destaque com um grafo grande (os testes usam dois ou três nós).

**Reabrir se:** o contorno não bastar para quem usa leitor de tela depois de um teste com uma pessoa real (era a condição do DR-30), ou se 200 ms não acompanharem um grafo grande.

---

## DR-33 — Sistema de mundos: terreno, cavernas, rios e estradas · ACEITA (2026-10-27)

**Pedido:** assumir por completo o sistema de mundos e pesquisar as melhores formas de fazer terreno, rios e estradas. Perguntas respondidas pelo criador: SDF completo (custo), mapas estilo Doom/Quake, chunks em SDF, escavar cavernas sem entrar, brushes 3D e ferramentas; `surface_follow` opt-in com padrão configurável; ferramentas avançadas **sim**; brushes 2D e 3D **coexistem**.

**Decisão:**

1. **Modelo híbrido, não SDF em todo o mapa.** O exterior é um **heightmap** em chunks (barato de guardar, desenhar e colidir). Cavernas e volumes orgânicos são **SDF 3D esparso em chunks**: só os blocos não vazios existem na memória e só os blocos tocados são malhados (a base já existe em `petunia_sculpt`). O SDF completo para o mapa inteiro foi recusado por **memória e desempenho**, não por dificuldade de implementação: a 1 m de resolução, um mapa de 1 km × 1 km × 200 m pede ~200 milhões de células (~800 MB), e a malhagem e a colisão crescem junto. O heightmap continua sendo a representação do exterior.
2. **Dois estilos de mapa, um editor.** (a) **Sólido/CSG estilo Doom/Quake**: brushes convexos (caixa, rampa, cilindro, cone, escada, cunha) com união, subtração e interseção, encaixados na grade — o `petunia_mesh` já tem os booleanos. (b) **Orgânico**: terreno esculpido e cavernas por SDF. Os dois **coexistem** na mesma cena; um modelo de projeto pode nascer com o estilo "boomer shooter" (grade grossa, brushes) ou "mundo aberto" (terreno + SDF), e as ferramentas dos dois ficam disponíveis.
3. **SDF em chunks: sim.** Blocos de 16³ (como em `petunia_sculpt`), alocados só quando tocados, malhados por *surface nets* por bloco, com atualização incremental. Isso mantém o custo de uma pincelada proporcional ao brush, não ao tamanho da caverna, e permite várias cavernas no mesmo mundo. LOD de SDF fica para depois (as cavernas são locais; o terreno é que precisa de LOD).
4. **Brushes 3D e falloffs.** Além dos brushes 2D de pegada (heightmap), existem brushes de **volume**: esfera, cápsula, caixa, cilindro, cone, plano (half-space), *stamp* de malha e caminho (spline). O falloff é radial ou **anisotrópico**, com perfis linear, suave (*smoothstep*), gaussiano e curva configurável. Os brushes 2D e 3D **coexistem** (o 2D altera altura; o 3D altera o campo de distância ou projeta um volume sobre o heightmap).
5. **Escavar sem entrar: modo "raio X" com prévia fantasma.** O brush é um volume no **espaço do mundo** (não depende da câmera), então dá para esculpir de fora. No modo de edição, a região do brush mostra: o volume do brush translúcido, a superfície resultante em cor de prévia (adicionar/remover) e o terreno em volta em 30 % de opacidade ou arame; **nada é gravado até soltar o botão/Enter**. Um plano de corte (secção) opcional revela o interior sem alterar a superfície. Um raio de alcance e a máscara de profundidade mostram por onde o pincel está passando.
6. **Objetos acompanham a superfície (`surface_follow`).** É uma **flag opt-in por objeto no inspetor**, com o padrão configurável no menu de configurações da engine. Quando ligada, ao editar terreno/caverna os objetos são re-projetados por raio na nova superfície, preservando o deslocamento (`surface_offset`), com alinhamento opcional à normal (`surface_align_normal`). Corpos dinâmicos não seguem. A re-projeção só percorre os chunks dentro do volume editado e usa ordem fixa por handle (determinismo).
7. **Todas as ferramentas avançadas entram** (erosão hidráulica, *stamp*, booleanos, espelhamento, preencher, oco, deslocamento por ruído, terraços etc.), em ordem de valor: primeiro as de uso diário, depois as avançadas. A lista completa fica em `docs/02-arquitetura/04-terreno-rios-estradas.md`.
8. **Rios e estradas** seguem o desenho já proposto: spline Bézier cúbica com perfis, escavação/aterro com falloff, malha por varredura (*sweep*), vetor de fluxo nos vértices, espuma por máscara de margem; estradas como **grafo** de nós e arestas, com cruzamentos por divisão de spline e nó, pontes por diferença de altura e decals de faixa. A spline e a varredura são **nossas** (determinismo), portadas do Petunia3D com `ATTRIBUTION.md`.
9. **Sem dependência nova para gerar terreno.** O ruído (valor/fBm, semente explícita) é implementado no crate `petunia_terrain`: "ruído de jogo é nosso" (AGENTS.md §2). O crate novo é domínio puro, sem GPU, e segue os limites de `petunia_sculpt`.

**Por quê:** o modelo híbrido dá o melhor custo por estilo de mapa, mantém a física barata no exterior e ainda permite cavernas orgânicas; os dois estilos de edição cobrem tanto *boomer shooters* quanto mundos abertos sem escolher um único paradigma. As escolhas de `surface_follow`, brushes e ferramentas foram feitas pelo criador nesta data.

**Fica de fora (declarado):** LOD de SDF, malhagem na GPU, SDF para o mapa inteiro, edição de caverna por dentro em primeira pessoa (fica para quando houver modo de jogo no editor), e o modo "blockout" dedicado do Level Builder.

**Reabrir se:** o custo de memória ou de malhagem do SDF em chunks passar do orçamento medido; ou se o modo raio X não bastar para esculpir cavernas sem entrar nelas.

---

## Como registrar uma decisão

Ao decidir, troque o estado por `DECIDIDA` e acrescente:

```text
Decidida em: AAAA-MM-DD
Escolha:     opção N
Motivo:      uma ou duas frases
Reabrir se:  condição objetiva
```

## Próximo passo

**Responder `DR-11`, pergunta 1** (esta pasta é a retomada do `petunia-game-engine`?) e **`DR-01`** (direitos autorais do Petunia3D). Elas destravam as outras.
