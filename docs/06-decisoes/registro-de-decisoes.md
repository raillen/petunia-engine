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
| DR-07 | Programação visual | **HERDADA** (Gameplay IR) + detalhes **PROPOSTOS** | IR + Event Sheet + grafos especializados |
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
| **DR-24** | Workspaces e prioridades antes da API | **EM PARTE ACEITA** (andares, cavernas B) | Mapa (com Jogar), Pintura sem UV, Animação, Efeitos por nós; antes da API: andares, cavernas, terreno, prefabs, Jogar ([análise](../02-arquitetura/14-workspaces-e-prioridades.md)) |

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
