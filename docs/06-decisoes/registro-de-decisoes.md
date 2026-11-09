# Registro de decisões

**Revisão:** 2026-11-02 (`DR-41` câmeras fixas, `DR-42` parentesco, `DR-43` `Vec3`/`Color` e câmera do personagem, `DR-44` interagir e vida; primeiro corte do `DR-13`) · 2026-10-30 (índice completado; `DR-27`/`DR-28` duplicados renumerados para `DR-38`/`DR-39`) · primeira leitura da [especificação anterior](../01-pesquisa/05-especificacao-anterior.md) em 2026-10-24

## Resumo (TL;DR)

São **45 decisões**. Estados possíveis:

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
| DR-13 | Pós-processamento | **PRIMEIRO CORTE ACEITO** (2026-11-02); AgX, bloom e SMAA continuam proposta | Passe fundido de conforto (contraste, vinheta, grão, teto de brilho) |
| DR-14 | Escopo e ordem das fases | PROPOSTA | Base primeiro, mundo natural depois |
| DR-15 | Gates de qualidade | PROPOSTA | Pirâmide do Petunia3D + acessibilidade |
| **DR-16** | Prova de conceito (Digi-Tactics + Ilha) | PROPOSTA | Duas provas; fatia mínima; criaturas originais no público |
| **DR-17** | UI de jogo | PROPOSTA | UI própria retida, como dados, com acessibilidade; S9 |
| **DR-18** | Como reusar a GUI do Petunia3D | PROPOSTA | Cópia fotografada agora; biblioteca compartilhada `ui-kit` depois |
| **DR-19** | Idioma da documentação | PROPOSTA | Inglês canônico + pt-BR sincronizado, ao iniciar a implementação |
| DR-20 | Linguagem do núcleo (7 candidatas) | **DECIDIDA** (2026-10-24) | **Rust**; S0 mede de novo a cada fase |
| DR-21 | Ponte entre o editor Qt e o motor | **DECIDIDA** (2026-10-24) | **Qt Widgets + JSON sobre interface C**; CXX-Qt fica para depois |
| **DR-22** | Edição de primitivas e malha para mapas | **Níveis 1 e 3 FEITOS** (nível 3: 2026-10-25, a seu pedido); interface parcial (2026-10-27); nível 2 em parte | Brushes paramétricos + malha editável guardada na cena (booleanos, inset, loop cut, bevel…); a barra **Malha** já liga as operações de faces/arestas/vértices ([análise](../02-arquitetura/13-primitivas-e-edicao-de-malha.md)) |
| **DR-23** | Pintura e animação do Petunia3D | PROPOSTA | Reprodução e IK sim; autoria e pintura de textura não; só o pincel de mundo ([análise](../01-pesquisa/07-pintura-e-animacao.md)) |
| **DR-25** | Script Host: Aipo embutida, sem acesso ao sistema | **ACEITA** (2026-10-25, a seu pedido) | Aipo por dependência de caminho; módulos `sh`/`fs`/`env`/`io` **removidos**; permissões fixas; orçamento por chamada ([doc](../03-linguagens-e-api/05-script-host.md)) |
| **DR-26** | Layout e design da janela do editor | **ACEITA** (2026-10-25, a seu pedido; revista) | Medidas do Unity com piso da WCAG; menus no `≡`; paleta `Ctrl+K`; workspaces Cena e Código ([design](../02-arquitetura/15-design-da-interface.md)) |
| **DR-27** | Colisão e corpos físicos | **ACEITA** (2026-10-25, suas escolhas) | Colisões com nome como entidades; `RigidBody`; Rapier em `petunia_physics`; API `collision.*` e `physics.*` ([física](../02-arquitetura/06-fisica-e-cordas.md)) |
| **DR-28** | Personagem, modo Jogar e solidez automática | **ACEITA** (2026-10-26) | `Character` com tempo de coiote; `F6` roda a cena numa cópia; tudo que é desenhado é sólido; camadas de colisão ([física](../02-arquitetura/06-fisica-e-cordas.md)) |
| **DR-29** | Luzes de cena e importação de modelos GLB | **ACEITA** (2026-10-26, "Fase 2: aprovado") | Tabela `Light` (sol e pontual); `gltf` só em `petunia_import`; modelo vira malhas editáveis ([editor](../02-arquitetura/11-editor.md)) |
| **DR-30** | Sistema de nós (no lugar do Event Sheet) | **ACEITA** (2026-10-26, "tudo aprovado"; N1 implementado) | Grafo tipado 1:1 com a API; compila para Aipo; vista em contorno completa como primeira entrega; tela de nós própria sem biblioteca nova |
| **DR-31** | Prefabs embutidos e o nó Esperar | **ACEITA** (2026-10-26, "prossiga com a implementação"; N4 implementado) | Porta, Item coletável e Ponto de nascimento como moldes de linhas + grafos; `Esperar` compila para uma função por espera |
| **DR-32** | Tela de caixas e fios e depuração (N3 e N5) | **ACEITA** (2026-10-26) | O canvas é mais uma vista do mesmo grafo, com as mesmas operações; o contorno continua sendo a interface acessível; marcas de depuração no compilador e "por que esta condição falhou?" |
| **DR-24** | Workspaces e prioridades antes da API | **EM PARTE ACEITA** (andares, cavernas B) | Mapa (com Jogar), Pintura sem UV, Animação, Efeitos por nós; antes da API: andares, cavernas, terreno, prefabs, Jogar ([análise](../02-arquitetura/14-workspaces-e-prioridades.md)) |
| **DR-33** | Vários scripts por objeto | **ACEITA** (2026-10-27, "prossiga") | Primeiro script na linha legada; os seguintes são entidades-filhas com `owner` e `order`; todos rodam |
| **DR-34** | Intercâmbio Aipo ⇄ IR | **EM IMPLEMENTAÇÃO** (2026-10-28, "continue") | IR com subconjunto explícito; recusa o resto com diagnóstico; uma VM por anexo |
| **DR-35** | Schema de grafo v2 e migração | **DECIDIDA** (2026-10-28, "continue") | `SCHEMA_VERSION` 2; grafos v1 são lidos e migrados ao abrir; nós de laço e `with_index` |
| **DR-36** | Terreno, rios e estradas | **ACEITA e MESCLADA** (2026-10-29) | Spline Bézier por comprimento de arco, perfis varridos, escavação/aplainamento/aterro, descida do rio, cruzamentos, lagos, quedas, confluências e grafo de navegação; comandos e consultas, barra no editor, gravação na cena, convés de ponte, leito sólido na física, erosão, ruído, passo por teclado e carimbos. Terreno em `petunia_world`. **Faltam** decals (`DR-12` é proposta), LOD em quadtree, tubo de bueiro e partículas ([arquitetura](../02-arquitetura/04-terreno-rios-estradas.md)) |
| **DR-37** | Giro do carimbo de terreno | **ACEITA** (2026-10-29; 15° por passo, `Ctrl` para 1°) | Ângulo por carimbo (`rotation`, em graus) aplicado ao deslocamento dentro do disco, campo **Girar** só no modo Carimbo e o mesmo estado no passo por teclado |
| **DR-38** | Cubo orbital e projeções da câmera | **ACEITA** (2026-10-25; antes numerada `DR-27`) | Arrastar o cubo orbita; menu alterna perspectiva/ortográfica com isométrico verdadeiro e games 2:1; estado só da sessão |
| **DR-39** | Seleção de componentes e correção da extrusão | **ACEITA** (2026-10-25; antes numerada `DR-28`) | `Tab` edita componentes (`1` vértices, `2` arestas, `3` faces); extrudar brush é um gesto reversível |
| **DR-40** | Um método de programação por projeto | **ACEITA** (2026-10-30, seu pedido) | Aipo, Luau ou nós, escolhido ao criar e mantido até o fim; a aba do outro método some; nós de gameplay continuam compilando para Aipo |
| **DR-41** | Câmeras fixas (várias por cena) | **ACEITA** (2026-11-02, seu pedido) | Tabela `Shot`: caixa em volta do objeto, prioridade e campo de visão; no Jogar, a de maior prioridade que contém o jogador substitui a câmera de perseguição |
| **DR-42** | Parentesco na hierarquia | **ACEITA** (2026-11-02, seu pedido) | Coluna `Transform.parent`; a posição gravada continua local; arrastar ou o menu do botão direito define o pai |
| **DR-43** | Vetores no script e câmera do personagem | **ACEITA** (2026-11-02, seu pedido) | `Vec3` e `Color` são structs da Aipo; `transform.position` devolve `.x` `.y` `.z`; a tabela `Camera` escolhe primeira pessoa, terceira, isométrica ou fixa |
| **DR-44** | Interagir e vida | **ACEITA** (2026-11-02, continuação do kit) | Tabela `Interact` (raio, aviso) e tecla E; tabela `Health`; o evento `died` quando a vida chega a zero |
| **DR-45** | Modo de edição e inspetor | **ACEITA** (2026-11-02, teste na janela) | Empurrar e extrudar só no modo edição; `Camera.mode` é uma lista, inclusive `custom`; interagir com um `Shot` olha por essa câmera |

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

**Revisão (2026-10-27):** o Luau será implementado **depois de fechar a integração da Aipo**; o usuário escolhe **uma** das duas linguagens por projeto. Até o backend Luau existir, um manifesto com `"luau"` continua válido, mas o Project Doctor emite o aviso `project.manifest.luau_pending`; a execução de scripts Luau ainda não acontece.

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

## DR-13 — Pós-processamento · PRIMEIRO CORTE ACEITO (2026-11-02)

**Opções (tonemap padrão)**
1. **AgX.** — **recomendada**, ainda não feita
2. **Khronos PBR Neutral.**
3. **ACES.**

**Obrigatório:** passe fundido e **conforto sensorial** (limitador de flash, reduzir movimento, perfis).

**Primeiro corte (2026-11-02, pedido explícito de pós-processamento).** A cena vai para um framebuffer e um triângulo de tela inteira devolve contraste, vinheta, grão e um teto de luminância. **Exibir › Conforto visual** tira vinheta e grão, abaixa o teto e só deixa esse teto subir 0,04 por quadro. Bloom, AgX, LUT e SMAA ficam para um corte seguinte. O limitador é global (não por pixel): um flash verdadeiro por pixel precisaria ler a imagem de volta.

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

**Revisão (2026-10-27) — interface e nós:** quando a interface for traduzida, entra também o catálogo de nós/sheets. O contrato já está certo: os arquivos guardam ids, nunca rótulos; `catalog.rs`, `graph.rs` e `outline.rs` concentram os textos; Contorno e Caixas e fios usam o mesmo catálogo; o código Aipo não é traduzido. O inglês entra primeiro no catálogo, depois a interface Qt e a documentação (`docs/en` + `docs/pt-BR`).

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

**Execução (2026-10-30/31) — reprodução feita, sem copiar do Petunia3D.** O Petunia3D **não está
nesta máquina**, então o plano original (copiar ~3,8 mil linhas) não podia ser seguido. A pesquisa
deste mesmo documento já dizia qual era o pré-requisito e a ordem: *"fazer a importação de GLB com
esqueleto e só então copiar o núcleo; sem o GLB, não há como validar"*. Foi o que se fez, com
código **novo e próprio**:

| Peça | Onde | O que faz |
|---|---|---|
| `petunia_anim` | crate novo | `Mat4`/`Quat` (slerp, inversa rígida), esqueleto com pais-antes-de-filhos, clipes com canais de translação/rotação/escala, amostragem degrau/linear, **um passo** para a pose, matrizes de skinning e mistura com a pose de repouso |
| Leitor de GLB | `petunia_import/src/animation.rs` | lê o primeiro *skin*, as matrizes de bind e todas as animações; leitor de accessor próprio (a `gltf` 1.4 não expõe um), com limite de tamanho, recusa de arquivo truncado e recusa **por nome** de interpolação cúbica |

**Determinismo:** custo e amostragem são inteiros e explícitos, sem dependência e sem desempate por
ponto flutuante; 9 execuções dão a mesma pose (teste).

**O que ficou de fora, declarado:** IK (o `ik.rs` do Petunia3D segue no `DR-23` como proposta),
autoria de esqueleto e poses, interpolação cúbica, e o desenho de malha com skinning no
renderizador (hoje o esqueleto e os clipes são lidos e calculados, mas a GPU ainda desenha a malha
na pose de repouso).

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

## DR-38 — Cubo orbital e projeções da câmera · ACEITA (2026-10-25)

> **Renumerada em 2026-10-30.** Esta seção nasceu como `DR-27`, número que já pertencia a
> [Colisão e corpos físicos](#dr-27--colisão-e-corpos-físicos--aceita-2026-10-25) e que o índice
> do registro usa para ela. Duas decisões com o mesmo número tornavam `DR-27` ambíguo em qualquer
> referência; o conteúdo não mudou.

**DECLARADO:** pedido de arrastar o cubo global, alternar projeção e acrescentar a vista de games 2:1.

**Decisão:** arrasto acima de 4 pixels orbita sem executar o clique ao soltar; faces, arestas, cantos, bússola e início continuam disponíveis. O menu e o rótulo alternam perspectiva/ortográfica e oferecem dois presets ortográficos: isométrico verdadeiro (azimute 45°, elevação 35,264389°) e games 2:1 (azimute 45°, elevação 30°). **GERAL:** no segundo, `atan(sin(30°)) = atan(0,5) = 26,565051°` nas diagonais da tela; 26,565° não é a elevação da câmera.

**Contrato:** operações validadas `camera.orbit {dx, dy}`, `camera.projection {mode}` e `camera.view {name: isometric | game_2_1}`; `camera.get` e `viewport.overlay.camera` expõem a projeção. A troca de projeção preserva o tamanho no plano de foco; picking e gizmos usam a mesma matriz; zoom ortográfico altera a altura visível. Estado somente da sessão, sem alterar a cena ou o histórico de desfazer. Sem dependências novas.

**Reabrir se:** forem necessárias câmeras de jogo persistentes ou projeções oblíquas; esses recursos não estão incluídos neste pedido.

---

## DR-39 — Seleção de componentes e correção da extrusão · ACEITA (2026-10-25)

> **Renumerada em 2026-10-30.** Esta seção nasceu como `DR-28`, número que já pertencia a
> [Personagem, modo Jogar e solidez automática](#dr-28--personagem-modo-jogar-e-solidez-automática--aceita-2026-10-26)
> e que o índice do registro usa para ela. Duas decisões com o mesmo número tornavam `DR-28`
> ambíguo em qualquer referência; o conteúdo não mudou.

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

1. **Estado entre chamadas — resolvido, com revisão.** A Aipo aceita `var` no nível do módulo. Até 2026-10-27 o motor era **um por arquivo**, compartilhado por todos os objetos; por isso a variável de um nó era guardada num dicionário com o objeto na chave (`graph_state`). Desde o DR-34 (2026-10-28) o motor tem **uma VM por anexo (objeto + script)**: o estado de módulo já é isolado por objeto. O `graph_state` gerado continua porque o formato do código do grafo é estável e porque ele expressa estado por objeto de forma explícita; não é mais uma necessidade do host. Vale enquanto o jogo roda; não entra no desfazer nem no arquivo da cena. Variáveis que precisam **sobreviver ao salvar** (progresso de jogo) ficam para o sistema de saves. A tabela `GraphState` proposta antes **não** foi criada.
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

**Revisão (2026-10-27) — fluxo código ↔ nós:** o **grafo é a fonte** da lógica visual. **Ver como código** é somente leitura e mostra o texto gerado; **Transformar em script** copia esse texto para um `.aipo` independente. Um script `.aipo` editado **não** altera o grafo, e texto livre **não** vira grafo automaticamente. O workspace de programação lista os dois tipos e marca o que está ligado a cada objeto.

**Revisão (2026-10-28) — S5b aprovado como requisito:** o experimento de ida e volta deixou de ser opcional: o pedido foi “os nós não refletem 1:1 o código, então é impossível intercambiar”. A decisão [DR-34](#dr-34--intercâmbio-aipo--ir--em-implementação-2026-10-28) substitui a promessa de 1:1 total por um **IR com subconjunto explícito**, com recusa diagnosticada do que fica fora. A vista visual continua sendo uma vista do IR, não um espelho de toda a AST.

## DR-31 — Prefabs embutidos e o nó Esperar · ACEITA (2026-10-26)

**Pedido:** "prossiga com a implementação" (passo N4 do DR-30).

**Decisão:**

1. **Prefab = molde de linhas + grafos.** Criar um prefab (`prefab.spawn`) cria os objetos numa única ação de desfazer, na frente da câmera, no andar atual, e põe na pasta `scripts/` os grafos de que ele precisa. **Um grafo que já existe com o mesmo nome nunca é sobrescrito** (a pessoa pode tê-lo editado). O comportamento é, portanto, um grafo comum que se abre e muda na aba Lógica.
2. **Quatro moldes embutidos**, em `crates/petunia_editor/src/prefabs.rs`:
   - **Porta:** objeto cinemático com uma colisão sólida e um sensor maior; sobe 3,2 m quando um personagem entra no sensor, espera 3 s e desce; a variável `aberta` impede abrir duas vezes. Grafo `porta`.
   - **Item coletável:** esfera com sensor; some e avisa no console quando um personagem a toca. Grafo `item`.
   - **Ponto de nascimento:** objeto com a tabela `PlayerStart` e uma marca que não bloqueia. Sem grafo.
   - **Personagem jogável (revisão 2026-10-27):** um `Brush` cilindro com `Character.player`, uma cápsula sólida e o jogador de teclado. Resolve o caso de criar um objeto "player" que não colidia.
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

**Revisão (2026-10-27):** o canvas ganhou arrastar caixas e fios (além do clique-clique), pan, zoom com centralizar, seleção múltipla (`Ctrl+clique`/`Ctrl+A`), **grupos visuais salvos no layout**, menu de objetos nos pinos e uma lista de scripts/grafos ligados ao objeto. O código gerado continua somente leitura; **Transformar em script** copia o texto para um `.aipo` independente.

**Não verificado:** leitor de tela no canvas (o contorno tem texto e descrição falados; **não ouvi** nenhum dos dois), e o destaque com um grafo grande (os testes usam dois ou três nós).

**Reabrir se:** o contorno não bastar para quem usa leitor de tela depois de um teste com uma pessoa real (era a condição do DR-30), ou se 200 ms não acompanharem um grafo grande.

---

## DR-33 — Vários scripts por objeto · ACEITA (2026-10-27)

**Pedido:** "prossiga com a implementação" (lista de pendências de script/nós/sheets).

**Decisão:**

1. O **primeiro** script de um objeto continua na linha `Script` do próprio objeto (`owner` 0), para não migrar projetos existentes.
2. Ao anexar um **segundo** script, a linha legada vira uma entidade-filha com `Script { owner, file, enabled, order }`, e o novo script vira outra filha. A conversão é **um** `world.batch` (um desfazer).
3. A execução ordena por **id do objeto**, depois `order`, depois nome do arquivo. Cada anexo tem o seu `on_start`, `on_update`, `on_event` e handlers de colisão; uma falha desliga só aquele anexo.
4. Excluir o objeto leva os anexos; duplicar copia os anexos. Um anexo cujo objeto sumiu por `world.remove` deixa de rodar.
5. Na janela, **Anexar** acrescenta (não substitui); os anexos aparecem na Hierarquia sob o objeto. `owner` é somente leitura; `order` e `enabled` são editáveis.

**Fora desta etapa (declarado):** reordenar por arrastar na Hierarquia, um painel dedicado de scripts por objeto, e escolher a linguagem por anexo (a linguagem é uma por projeto).

**Reabrir se:** a linha legada atrapalhar o formato ou se a ordem precisar ser visual em vez de um número.

---

## DR-34 — Intercâmbio Aipo ⇄ IR · EM IMPLEMENTAÇÃO (2026-10-28)

**Pedido:** “os nós não estão funcionando muito bem e nem estão refletindo 1:1 o código, portanto impossível de intercambiar, o que poderíamos fazer?” seguido de “continue”.

**Decisão:**

1. **Não prometer 1:1 total com Aipo.** O grafo atual representa chamadas da API e fluxo de alto nível; ele não é uma AST. A troca completa exigiria um parser e um IR, e o que não for suportado não pode ser convertido “mais ou menos”.
2. **IR com subconjunto explícito.** O primeiro caminho implementado lê Aipo para dados tipados e escreve Aipo canônico de volta. O subconjunto está em `crates/petunia_script/src/source_ir.rs`: funções simples, globais `let`/`var`, atribuição, `if`/`elif`/`else`, `while`, `loop`, `repeat`, `each`, `break`, `continue`, `return`, chamadas posicionais, campos, índices, listas, dicionários, literais e operadores unários/binários.
3. **Recusa explícita.** `struct`, `enum`, `impl`, imports, closures, `attempt`, `await`, `?.`, ranges, pipelines e diretivas retornam `IR_UNSUPPORTED` com linha, coluna e motivo. O texto continua válido como script; ele só não vira IR/nós.
4. **Uma VM por anexo (objeto + script).** O módulo compilado é compartilhado, mas o topo roda uma vez por anexo. `var` de módulo não vaza mais de um objeto para outro. O código gerado por grafo continua usando `graph_state` por compatibilidade e por explicitar a chave do objeto.
5. **IR → grafo com variáveis e fluxo.** `crates/petunia_script/src/ir_graph.rs` converte evento, funções auxiliares (`flow.function`, `flow.call`, `flow.call_value`, `flow.param`, `flow.return`), globais (`flow.global`, `global.get`, `global.set`), variáveis locais de função (`local.declare`, `local.get`, `local.set`), `let`/`var` de evento e atribuição composta (para `var.get`/`var.set`), chamadas de API, `if`/`elif`/`else` (aninhados em `flow.if`), `while` (`flow.while`), `loop` (`flow.loop`), `repeat` (com índice opcional em `flow.repeat`), `each` com uma variável (`with_index=false`) ou duas (`with_index=true`), `break`/`continue` (`flow.break`/`flow.continue`) e `return`, além de expressões (parâmetros, literais, chamadas de valor e operadores) para o `GraphDocument` atual. O slot da variável de evento inclui o handler (`on_update/speed`), então o mesmo nome em dois handlers não se mistura; reatribuir um `let` é recusado. Campos, índices, listas e dicionários como valor, e esperas dentro de função, ainda são recusados com `IR_GRAPH_UNSUPPORTED`.
6. **Editor ligado.** `script.to_graph` cria um grafo novo a partir do script (ou do texto não salvo); o menu **Transformar em grafo…** no painel Código abre o resultado na aba Lógica. O script de origem fica intacto e um grafo existente nunca é sobrescrito.
7. **Próximas etapas, na ordem:** exercitar o menu e o botão no smoke; só depois expandir o catálogo de nós. Luau continua depois da Aipo (DR-06). O schema v2 e a migração de grafos v1 estão registrados no DR-35.

**Verificação de 2026-10-29:** os 6 testes de `source_ir`, os 15 testes de `ir_graph` (incluindo função auxiliar e global/local em execução), os 21 testes públicos de grafo (incluindo `flow.while`, `each` sem índice, `break` fora de laço e migração v1→v2), 4 testes de compilação de funções/globais/locais em `petunia_graph`, os 14 testes de modelo (incluindo a migração) e o teste de `script.to_graph` no editor, que agora converte função auxiliar e recusa lista como valor. A suíte total soma **446 testes**. O teste de ida e volta compara o texto canônico depois de Aipo → IR → grafo → Aipo → IR; ainda **não** cobre o corpo de conformidade inteiro da Aipo. O menu Qt **não** foi exercitado no smoke.

**Fora desta etapa (declarado):** converter qualquer Aipo, IR neutro para Luau, e interface. A edição contínua existe no recorte suportado via `graph.from_code`; fora dele, diagnóstico. Não há promessa de que um script fora do subconjunto abra como nós.

**Reabrir se:** o subconjunto não cobrir os projetos reais; nesse caso, ampliar com casos de teste, não com conversão silenciosa.

---

## DR-35 — Schema de grafo v2 e migração · DECIDIDA (2026-10-28)

**Pedido:** “complete completamente a implementacao, incluindo o schema v2 sem parar.”

**Decisão:**

1. **Subir `SCHEMA_VERSION` de 1 para 2.** O schema v2 acrescenta `flow.while`, `flow.loop`, `flow.break`, `flow.continue` e a opção `with_index` em `flow.each`. `flow.each` sem a opção mantém o comportamento v1: posição/chave e item/valor.
2. **Ler v1 e migrar ao abrir.** `GraphDocument::parse` aceita `schema_version: 1`, troca a versão para 2 e entrega o documento pronto. Como o formato de nós e fios não mudou, a migração não reescreve nós nem ligações; os padrões preservam o comportamento antigo. Grafo v3 ou mais novo é recusado com mensagem para atualizar o editor; versão 0 ou desconhecida é recusada como inválida.
3. **Compilar os novos nós.** `while` gera `while condição { ... }`; `loop` gera `loop { ... }`; `break`/`continue` só são aceitos dentro de `flow.repeat`, `flow.each`, `flow.while` ou `flow.loop`; `flow.each` com `with_index=false` gera `each item` e não expõe `index` (ligar esse pino é erro `EACH_INDEX_UNAVAILABLE`).
4. **Editor sem migração manual.** Grafos v1 existentes continuam abrindo; ao salvar, saem em v2. O layout `.layout.json` continua na versão 1, porque posições e grupos não mudaram.
5. **Fora desta etapa:** campos/índices/listas/dicionários como valor e edição contínua das duas vistas. Eles continuam recusados com diagnóstico, sem conversão aproximada. Funções auxiliares e globais passaram a ser suportados na etapa seguinte do DR-34.

**Verificação:** `version_one_is_migrated_to_two_when_read` no modelo; `version_one_graphs_are_migrated_to_version_two`, `a_while_node_repeats_until_its_condition_is_false`, `each_can_hide_the_index_from_the_body` e `a_break_outside_a_loop_is_refused` na suíte pública de grafo; testes de `while`, `loop` com `break`/`continue` e `each` de uma/duas variáveis em `ir_graph`; ida e volta de `loop`/`each` em `source_ir`.

**Fora desta etapa (declarado):** schema v3, migração de layout, edição contínua e qualquer promessa de que um script fora do subconjunto abra como nós.

---

## DR-36 — Terreno, rios e estradas: protótipo de domínio · ACEITA (2026-10-28, mesclada na `main` em 2026-10-29)

**Pedido:** "prossiga com a implementação completa" dos rios, estradas e água, sem bibliotecas novas e em worktree isolada (`feat/mundo-natural-rios-estradas`, a partir da `main` @ `3db1c42`).

**Numeração:** esta linha nasceu como `DR-33`/`DR-34`, números que na `main` já eram de outras decisões (scripts por objeto e intercâmbio Aipo ⇄ IR). Na mesclagem de 2026-10-29 ela virou **`DR-36`** e o giro do carimbo virou **`DR-37`**.

**Decisão:** implementar primeiro o **domínio puro**, com dados serializáveis e funções pequenas; a integração com comandos, editor e render fica declarada como etapa seguinte.

1. **Uma curva só.** `CubicBezierSpline` mora em `petunia_math` (parâmetro em unidades de segmento, handles, largura/profundidade/extra por ponto). A amostragem é **por comprimento de arco** e cada quadro carrega o parâmetro da curva, para largura/profundidade não dependerem de adivinhação. Limites: `MAX_SPLINE_POINTS` e `MAX_CURVE_SAMPLES`. `petunia_world` **não** redefine a curva; importa a de `petunia_math`.
2. **Perfil normalizado.** `offset_x` é múltiplo da meia largura e `offset_y` do raio/profundidade. O perfil de **água** é plano (a superfície não afunda); o leito é escavado no terreno por outro caminho. A varredura devolve `Result<PathMesh, PathMeshError>` com orçamento de vértices e perfis de 2 a `MAX_PROFILE_POINTS` pontos; o fluxo e a máscara de margem vão nos vértices.
3. **Terreno em `petunia_world`.** `Heightfield` com blocos de `HEIGHT_CHUNK_SIZE`, pincel com falloff, aplainamento com patamar interno (o resultado não depende de quantos carimbos se sobrepõem) e `chunks_touching`, para um caminho que cruza a borda do bloco continuar contínuo. `petunia_sculpt` escava, achata e aterra ao longo da spline; a escavação de rio usa largura e profundidade **por ponto**.
4. **Rios.** `validate_river_descent` devolve relatório (`descends`, `maximum_ascent`) e `lower_river_bed` baixa o leito; confluências entre pontas de splines, lago por leque de triângulos, queda como fita vertical e `water_volume_for_spline` para flutuação/nado/correnteza.
5. **Estradas.** Interseção por busca grossa + refino local determinístico, divisão por **de Casteljau** preservando os dois lados da curva, polígono de cruzamento ordenado por ângulo e aviso de cruzamento agudo. Faixas de **ponte** (estrada acima do terreno) e **bueiro/corte** (terreno acima da estrada) são detectadas quando a diferença de altura passa de um limite; a malha da ponte em si não entra nesta etapa.
6. **Navegação em `petunia_graph`.** A rede vira grafo por arestas compartilhadas; o Project Doctor recebe pontas soltas, ilhas desconectadas e arestas com menos de dois extremos.
7. **Sem dependência nova.** Tudo é matemática própria, determinística e sem `unwrap` em produção.

**Fora desta etapa (declarado):** comandos e consultas (`petunia_commands`), interface no editor Qt, armazenamento do terreno e das redes no arquivo de cena, malha de pontes e bueiros, decals, colisão de água na física, sombreadores de fluxo/espuma e desfazer de pinceladas.

**Complemento (2026-10-28, mesma data, depois da aceitação):** a etapa seguinte foi implementada nesta branch, e a lista acima fica **cumprida** assim:

1. **Comandos e consultas:** `terrain.create`, `terrain.paint`, `path.create`, `path.set`, `road.detect_structures`; `terrain.sample` e `path.report`. Cada pincelada é **um** comando, logo **um** passo de desfazer: não houve código novo de desfazer por pincelada.
2. **Editor Qt:** barra do terreno/rio/estrada (modo, raio, força, largura, profundidade), ferramentas `T`, `Y` e `U`, botão **Pontes e bueiros** e caixa de defensas. O **modo erosão** entrou no combo como `erode`. A barra mostra **só os controles da ferramenta ativa**, e o botão só habilita com uma estrada selecionada.
3. **Gravação:** `Terrain`, `TerrainChunk`, `River`, `Road` e `Bridge` são tabelas normais da cena e sobrevivem a salvar e reabrir.
4. **Malha da ponte:** laje fechada de 0,6 m sob a estrada mais as duas defensas, com **normal plana por triângulo** (a varredura dava a normal "para cima" a todas as faces, e o lado de baixo da laje ficava iluminado como céu). O **bueiro/corte continua só dado**: dentro de um corte é o terreno que faz a estrutura, e um tubo visível pertence a drenagem, não a esta etapa.
5. **Colisão de água:** cada `TerrainChunk` vira `TriMesh` sólida (com passo reduzido quando o bloco passaria de `MAX_SOLID_TRIANGLES`), então o **leito** escavado do rio é chão andável. A água **não** vira parede: continua meio, com empuxo, nado e correnteza. A colisão que faltava era a do fundo.
6. **Sombreadores de fluxo e espuma:** já estavam no `petunia_render_gl` (ondulação e espuma pela máscara de margem gravada nos vértices).
7. **Pincel de erosão:** uma passada de erosão térmica em `HeightChunk::erode_region` — cada célula lê a cópia do início, cede material ao vizinho mais baixo, e o pé ganha exatamente o que o pico perdeu (determinístico).
8. **Dois defeitos da janela, achados ao ligar o teste de fumaça aos botões de verdade (2026-10-28):** a barra de opções mostrava tudo de uma vez e empurrava o botão **Pontes e bueiros** para fora da área visível (o canto `⋯` da barra só alcança ações, não widgets); e a pergunta "há estrada selecionada?" comparava o **número** de `selected_road` com booleano, então o botão e a caixa **Defensas** nunca habilitavam. Os dois estão corrigidos e o teste de fumaça passa por eles (**7/7**, com erosão e ponte).
9. **Pincel de ruído (2026-10-29):** `petunia_math::noise` traz ruído de valor e fractal **próprios** — retículo de 1,5 m, três oitavas e hash de coordenada inteira —, porque ruído é determinismo-crítico e não pode vir de biblioteca. `HeightChunk::roughen_region` soma o ruído com falloff e **desconta a média** do que carimbou, então a pincelada detalha a superfície sem criar nem sumir com material; o teste de fumaça aplica o mesmo critério de volume da erosão. O modo entrou no combo como `noise` (**Ruído**) e o teste de fluxo do editor confere altura, sinal e desfazer. O teste de fumaça foi de 7/7 para **9/9**.
10. **Passo por teclado (2026-10-29):** a regra de acessibilidade "esculpir/pintar sem arrastar" (`docs/04-acessibilidade/01-editor.md`, pilar 1) virou `terrain.step`: **um** carimbo com o modo, o raio e a força da barra, sem `terrain.begin`/`drag`/`end` no meio do caminho, e sempre **um** `terrain.paint`, logo um desfazer. `times` repete o carimbo no mesmo ponto (teto de 32 no motor, 64 na porta) sem transformar o gesto em N passos de histórico. A resposta carrega `mode`, `radius`, `strength`, `point` e `height` (esta por `terrain.sample`), e a janela usa esses campos para escrever a frase do rodapé; o rótulo do modo vem do próprio combo, para não existirem duas listas de nomes. No viewport, a **barra de espaços** com a ferramenta de terreno e sem arrasto em curso dispara o passo, e o carimbo cai no **centro da tela** quando o ponteiro está fora dela — as setas bastam. O teste de fumaça foi de 9/9 para **11/11** (subiu + desfazer devolveu).
11. **Carimbos de terreno (2026-10-29):** um **carimbo** é uma **silhueta nomeada** aplicada com o pincel — não um pincel novo. `petunia_world::stamp::StampShape` traz quatro formas (`crater`, `mound`, `plateau`, `ridge`) como **função fechada** do afastamento em unidades do raio dentro do disco, de `±1` no ponto mais forte a **zero na borda**: dois carimbos vizinhos se encontram sem degrau, e a mesma forma no mesmo lugar dá sempre o mesmo terreno (nada de ruído, nada de biblioteca). A **força é em metros**: é o quanto a forma desloca o chão no seu ponto mais forte. No motor, `HeightChunk::stamp_region` percorre só as células do disco e soma `forma × força`. No comando, `terrain.paint` aceita `mode: "stamp"` com `stamp: "<forma>"` e **recusa** o modo sem forma (nome desconhecido já era recusado pelo `serde`); a forma viaja por carimbo, então uma pincelada pode misturar modos. Na barra, o combo **Modo** ganhou **Carimbo** e um combo **Forma** aparece **só** nesse modo — a barra não muda de largura nos pincéis do dia a dia. O passo por teclado funciona com carimbo sem nenhuma mudança, porque lê o mesmo estado. O teste de fumaça foi de 11/11 para **14/14** (cratera com meio afundado e borda alta, monte que só sobe).

**Continua fora:** decals (bloqueados pelo `DR-12`, que é proposta), LOD em quadtree, tubo visível de bueiro e partículas na queda.

**Não verificado:** a janela na sessão real do usuário; a evidência de janela é o teste de fumaça no display virtual (terreno, alturas pintadas, erosão, ruído, **passo por teclado e desfazer**, **carimbo de cratera e de monte**, rio, estrada e **ponte**), nenhuma imagem foi tirada na sessão real. O convés da ponte é provado pelo teste de malha e pelo quadro entregue à janela, não pela captura do teste de fumaça, cuja câmera fica atrás do barranco levantado. O passo por teclado **não** foi testado com leitor de tela: o texto do rodapé existe, não se sabe se é anunciado. Os carimbos **não** giram sozinhos: o ângulo é digitado e vale **só** para carimbos (o `DR-37` entregou o campo **Girar** com 15° por passo e 1° com `Ctrl`); a previsualização da silhueta antes do clique não existe e a forma **platô** só tem teste no domínio, não na janela.

**Reabrir se:** a integração com o editor precisar de outro formato de dados, ou se o custo de amostragem por comprimento de arco pesar em rios muito longos.

---

## DR-37 — Giro do carimbo de terreno · ACEITA (2026-10-29)

**Pedido:** fechar o último limite declarado do carimbo do `DR-36`. Hoje a forma é sempre aplicada **alinhada ao mundo** — a crista corre ao longo do eixo x —, então não há como levantar uma crista perpendicular a um rio ou a uma estrada sem girar o terreno inteiro.

**Opções**
1. **Ângulo por carimbo**, em graus, no próprio registro do carimbo (`rotation`, padrão `0`). O motor gira o deslocamento antes de avaliar a forma, então a silhueta continua sendo a mesma função de antes, apenas orientada: a força segue em metros e a borda segue zerando, o que mantém a emenda invisível entre carimbos vizinhos. Na barra, um campo **Girar** aparece no modo Carimbo ao lado de **Forma**; o passo por teclado lê o mesmo estado e a frase do rodapé ganha o ângulo. Cenas antigas continuam válidas (`serde(default)`), não há comando novo e uma pincelada inteira continua sendo **um** desfazer.
2. **Giro pela direção da pincelada:** a crista segue o traço. Não custa campo na barra, mas o resultado deixa de ser previsível — o mesmo clique em pontos diferentes gira sem aviso, um carimbo parado não tem direção e o passo por teclado, que não tem traço, não teria o que seguir.
3. **Sem giro:** entregar mais formas fixas (vala, degrau, encosta) e considerar a orientação resolvida por composição de formas.

**Recomendação:** **1.** É o menor acréscimo que resolve o problema real (crista perpendicular a um rio ou a uma estrada, encosta virada para o vale), cabe no formato de dados que já vai para a cena e mantém a regra "carimbo é dado, não gesto". A opção 2 não tem como ser testada com firmeza: depende do ponteiro, que é justamente o que a regra de acessibilidade quer tornar opcional.

**Detalhes a decidir junto**
1. **Passo do campo:** 15° com `Ctrl` para 1°, ou ângulo livre digitado.
2. **Alcance:** recomendo que o giro valha **só para carimbos**. Um pincel radial (subir, descer, suavizar, erosão, ruído) não tem orientação, e um campo que não muda nada ensina a interface errada.

**Fora desta proposta:** espelhar a forma (a crista é simétrica; o espelho só faria diferença numa forma assimétrica futura) e pré-visualizar a silhueta no viewport antes do clique.

**Decisão (2026-10-29, "o giro do carimbo: ângulo por carimbo", "15°, com Ctrl para 1°"):** opção **1**, com o campo **Girar** a 15° por passo e 1° com `Ctrl` segurado. O giro vale **só** para carimbos; os pincéis radiais continuam sem ângulo, e o campo nem aparece neles.

**Escolha:** opção 1 (ângulo por carimbo), passo de 15° e ajuste fino de 1° com `Ctrl`.
**Motivo:** é o menor acréscimo que resolve o problema real (crista perpendicular a um rio ou a uma estrada, encosta virada para o vale), mantém "carimbo é dado, não gesto" e deixa o caminho sem ponteiro (o passo por teclado) com o mesmo resultado do caminho com mouse.
**Reabrir se:** aparecer necessidade de espelhar a forma, de girar por eixo diferente do vertical, ou de orientar a silhueta pela direção de uma estrada ou de um rio (aí o giro passaria a ser derivado, não digitado).

**Como ficou (LOCAL, verificado):** `StampShape::height_at_rotated` gira o deslocamento antes de avaliar a forma, então a borda continua zerando e a força continua em metros; `HeightChunk::stamp_region` recebe o ângulo e descarta um ângulo não finito. No comando, cada carimbo aceita `rotation` (padrão `0`, recusado se não for finito), então uma pincelada pode misturar ângulos. Na barra, o campo **Girar** aparece ao lado de **Forma**, só no modo Carimbo; `Ctrl`+seta dá 1° e a digitação é enviada ao soltar o campo ou apertar Enter. A frase do passo por teclado passa a dizer o ângulo — "Passo: Carimbo (Crista, 90°), raio 4,00 m…".

**Verificação:** 2 testes novos em `petunia_world::stamp` (a crista vira 90° e troca os eixos; 0°/360° dão a mesma altura e um ângulo hostil não escreve nada), 1 no `heightfield` (a crista girada levanta ao longo de z e o pico não muda) e 1 de fluxo no `petunia_editor` (mesma forma, duas medidas, um desfazer entre elas). No teste de fumaça o mesmo carimbo de crista é aplicado reto e girado 90°: **16/16** (a medida ao longo de x cai e a ao longo de z cresce).

---

## DR-40 — Um método de programação por projeto · ACEITA (2026-10-30)

**Pedido:** "manter o grafo/node-system separado da programação em si e o usuário poder escolher só
um método/caminho de programação para o projeto, parece que isso está deixando o fluxo mais
confuso"; completado por "Aipo, Luau ou NodeSystem, o usuário deve seguir até o fim apenas com um
desses modos".

**O problema real (LOCAL).** Não eram dois métodos: eram **três coisas na mesma tela**. As abas
**Cena** (montar o mundo), **Código** (Aipo) e **Lógica** (nós) apareciam sempre as três, e
Código e Lógica são duas vistas do **mesmo** motor. O `ProjectManifest` **já** tinha
`scripting_language` (`"aipo"` | `"luau"`), validado e com teste, mas **ninguém lia esse campo**:
`editor/src/*.cpp` não o consulta, então as três abas apareciam em qualquer projeto. A confusão não
era da ideia de escolher um método — era de o campo existir e ser ignorado.

**Decisão.**

1. **Um método por projeto, escolhido ao criar e mantido até o fim.** O manifesto passa a ter
   `programming_method` com três valores: `aipo`, `luau` ou `nodes` (nome do valor em discussão, ver
   o item 5). Scripts `.aipo` e grafos `.graph.json` **não se misturam** no mesmo projeto.
2. **A janela mostra o método escolhido.** Em `aipo`/`luau` a aba **Código** aparece e a aba
   **Lógica** some; em `nodes` a aba **Lógica** aparece e a aba **Código** some. A aba **Cena**
   continua em todos, porque montar o mundo não é programar.
3. **Os nós de gameplay continuam compilando para Aipo** (`petunia_graph::compile`). `nodes` é uma
   vista de autoria, não uma quarta linguagem de execução: por baixo, o grafo vira o mesmo texto
   Aipo que o Script Host já roda. Isso evita segundo runtime, segundo depurador e segundo conjunto
   de erros.
4. **A ponte `graph.from_code` deixa de ser o fluxo normal** e passa a ser uma ação pontual e
   consciente ("ver isto como texto"), disponível só quando o projeto é `nodes`. Hoje ela está
   quebrada na janela: o botão **Aplicar código** nunca foi criado (`applyCodeButton_` só é
   declarado e ligado), o campo é `setReadOnly(true)` e o texto promete um botão inexistente. Com
   esta decisão, o conserto certo é **converter para Aipo de verdade** (`analysis`, não ida e
   volta contínua) e não reabrir a edição contínua que o `DR-30`/`DR-32` fecharam.
5. **Nome do método.** O valor do manifesto é técnico e o rótulo da interface fica em português.
   **Nome de produto decidido: `pNodes`** (grafia exata, "p" minúsculo) — provisório, para não
   travar o código; pode mudar sem tocar no formato dos arquivos, porque o valor gravado continua
   sendo `nodes`.
6. **Grafos que sobrevivem a um projeto `aipo`/`luau`.** No futuro haverá grafos de **outras
   coisas** (animação, processos procedurais), que não são programação de gameplay. Este `DR`
   vale só para o grafo de **gameplay**; os outros terão decisão própria.
7. **Prefab no formato do projeto (decidido).** Um prefab entrega os mesmos grafos nos dois
   métodos: em `nodes` grava `<nome>.graph.json`; em `aipo` **compila e grava `<nome>.aipo`**
   (`petunia_graph::compile`, o texto já usado por "Ver como código"). Nos dois casos a regra de
   nunca sobrescrever um arquivo que já existe continua valendo, e agora precisa considerar **as
   duas extensões** como "já existe": um projeto em `aipo` com `porta.graph.json` de uma migração
   não pode ganhar um `porta.aipo` por cima sem aviso.

**Consequências que precisam ser tratadas ao implementar**

| Ponto | Situação hoje |
|---|---|
| Prefabs | `prefabs.rs` grava **`.graph.json`** (`porta`, `item`). **Decidido:** com o projeto em `aipo`, o prefab compila o grafo e grava `.aipo` (item 7) |
| Projeto de exemplo | não cria script algum, então não é afetado |
| Project Doctor | `petunia_script::project_diagnostics` varre a pasta `scripts/`; precisa avisar quando houver `.aipo` num projeto `nodes` (e o inverso), em vez de carregar os dois em silêncio |
| Migração | projeto sem `programming_method` assume `aipo` (é o padrão de hoje) e continua abrindo; um projeto com os dois tipos de arquivo recebe aviso, não erro |
| `luau` | continua "planejado, backend não existe": escolher Luau hoje dá aviso e não executa (`project.manifest.luau_pending`) |

**Correção de 2026-10-30 (o aviso era falso):** o Project Doctor passou a avisar
`project.method_graph`/`project.method_script`, mas o **carregador continuava lendo os dois tipos**
— `ScriptHost::load_directory` varria `.aipo` e `.graph.json` sempre. Na prática o aviso dizia "não
roda" enquanto o arquivo **rodava**, que é pior do que não avisar. Agora o método é um parâmetro do
carregador (`load_directory(directory, programming_method)`), o `PlayerConfig` leva
`programming_method` (o `petunia run` passa o do manifesto) e o **Jogar** do editor carrega só o
método escolhido. Com isso o conflito de nomes entre `.aipo` e `.graph.json` deixou de existir: cada
projeto lê só os seus. Verificado por `only_the_projects_own_method_is_loaded` (um projeto Aipo com
um grafo na pasta carrega o script e **não** o grafo; o pNodes, o contrário) e por uma execução
manual sobre um projeto real: `metodo=aipo carregados=[]`, `metodo=nodes carregados=["sobra"]`.

**Fora desta decisão:** escolher método **por objeto** (fica um método por projeto), tradução
automática Aipo ⇄ Luau, e o catálogo de nós das outras áreas.

**Decisão de 2026-10-30 — não há troca de método.** "Como já falei, quero um único modo de seleção
para o jogador, portanto, não precisa disso." A escolha é feita ao criar o projeto e vale até o fim.
Fica **fechado** o item que este `DR` deixava em aberto: não se constrói botão de troca nem
conversão de lógica entre métodos. Quem editar `programming_method` à mão não recebe apoio: o
editor não converte nada, e os arquivos do outro método deixam de rodar (o Doctor avisa). Isso é
coerente com a assimetria já registrada acima — grafo → Aipo sempre compila, Aipo → grafo só num
subconjunto —, então uma troca "de verdade" prometeria algo que falha em metade dos projetos.

**Reabrir se:** um projeto real precisar dos dois métodos ao mesmo tempo (aí a regra vira "um método
por objeto ou por pasta de scripts"), ou se o Luau entrar e a escolha em tempo de criação se
mostrar irreversível demais na prática.

---

## DR-41 — Câmeras fixas · ACEITA (2026-11-02)

**Pedido:** sistema de câmera, inclusive várias câmeras, para jogos de câmera fixa no estilo Resident Evil.

**Decisão.** Um objeto com `Transform` pode ganhar a tabela `Shot`: campo de visão, prioridade, meia-caixa (`half_extents`) e se está ligada. A caixa fica em volta da pose **composta** do objeto. No modo Jogar, `active_shot` escolhe a de maior prioridade que contém o jogador; empate fica com o id menor. Fora de toda caixa, a câmera de perseguição (distância 4,5 m, altura 1,5 m) continua. A hierarquia oferece **Câmera fixa neste objeto** (prioridade 1).

**Fora deste corte:** corte entre câmeras com curva, gatilho por volume de colisão separado do objeto, e várias câmeras desenhadas ao mesmo tempo (tela dividida).

**Escolha:** uma câmera ativa por vez, escolhida pela posição do jogador.
**Motivo:** é o mínimo que permite uma sala com ângulos fixos sem um grafo de cena novo.
**Reabrir se:** um jogo precisar de tela dividida ou de um volume de ativação que não seja a caixa do próprio objeto.

---

## DR-42 — Parentesco · ACEITA (2026-11-02)

**Pedido:** arrastar na hierarquia com opção de parentesco entre os elementos.

**Decisão.** `Transform` ganha a coluna `parent` (inteiro, 0 = raiz). A posição gravada continua **local**. `composed_transform` sobe até 32 pais e para em ciclo ou em pai ausente. Física, desenho e a posição do jogador para scripts usam essa pose. Cenas antigas não têm a coluna: o leitor preenche 0. Arrastar solta no pai (ou na raiz, o que zera o pai). O menu do botão direito faz o mesmo sem o mouse: **Soltar da hierarquia** e **Tornar filho de…**. Com o filtro de busca preenchido o arrasto desliga, porque linhas ocultas receberiam o objeto no lugar errado.

**Limite:** soltar o pai **não** grava a posição mundial de volta. O deslocamento local permanece, então o objeto pode saltar. O `Ctrl+Z` desfaz o parentesco.

**Escolha:** coluna `parent` mais pose composta, sem uma classe de grafo de cena.
**Motivo:** a arquitetura já dizia que a hierarquia é uma coluna, não uma árvore de objetos.
**Reabrir se:** soltar o pai tiver de manter o objeto visualmente parado (aí o comando grava a pose composta antes de zerar).

---

## DR-43 — Vetores no script e câmera do personagem · ACEITA (2026-11-02)

**Pedido:** a API em Aipo fácil de ler, com campos por ponto (não chaves de dicionário), e o jogador provisório no mesmo molde do prefab jogável (câmera e colisão).

**Decisão.** Todo script compilado ganha, no começo, `struct Vec3` (`x`, `y`, `z`) e `struct Color` (`r`, `g`, `b`, `a`), com campos `var`. O diagnóstico desconta essas linhas, então o sublinhado do editor continua na linha do autor. Quem já declara `struct Vec3` não recebe o preâmbulo.

`transform.position`, `rotation` e `scale` devolvem `Vec3`. `set_position`, `set_rotation`, `set_scale`, `move` e `rotate` recebem um `Vec3` depois da entidade. `forward`, `right`, `up`, `physics.velocity` também. `set_velocity` e `impulse` recebem um `Vec3`. `move_x`/`move_y`/`move_z` e `rotate_x`/`rotate_y`/`rotate_z` continuam: são a forma de um eixo, e grafos e scripts antigos já as chamam. Um dicionário com as mesmas chaves ainda é aceito na entrada, porque o compilador de nós emite dicionário. `world.get` de uma célula vetor ou cor devolve a struct. `world.row` continua um dicionário; as células vetor e cor dentro dele são structs. O acerto do raio continua um dicionário (`entity`, `collider`, `distance`); `point` e `normal` são `Vec3`.

A tabela `Camera` fica no personagem: `mode` (`fixed`, `third`, `first`, `isometric`), `distance`, `height`, `fov_degrees`, `enabled`. `fixed` deixa o `Shot` do `DR-41` vencer e, fora da caixa, persegue com a distância e a altura da linha. `third` persegue sempre. `first` põe o olho nos pés mais `height`, com o yaw da pose composta. `isometric` usa projeção ortográfica. Sem a linha, o jogo segue o caminho de hoje (shot, depois perseguição). O herói criado na hora e o prefab `player` nascem com `mode` `fixed` e uma cápsula `corpo` (raio 0,3, altura 1,8). O WASD continua no anfitrião, não num script.

**Escolha:** structs da linguagem para os dois tipos de valor, uma função por vetor inteiro, eixos à parte, e a câmera como linha do personagem.
**Motivo:** em Aipo o ponto lê campo de struct; a chave de dicionário não aceita atribuição. Uma forma só para o vetor inteiro, e a forma de um eixo que os grafos já usam.
**Reabrir se:** um script de projeto já publicado chamar `set_position`/`move`/`rotate` com três números (a assinatura mudou), ou se `Quat` entrar como tipo de valor.

---

## DR-44 — Interagir e vida · ACEITA (2026-11-02)

**Pedido:** o kit que falta depois da câmera: chegar perto e apertar a tecla, e ter vida que avisa quando acaba.

**Decisão.** Um objeto ganha `Interact`: `radius` (2 m), `prompt` ("Interagir") e `enabled`. No Jogar, a tecla **E** (uma vez por aperto, sem repetir enquanto fica segurada) avisa o objeto mais perto cuja distância até o personagem cabe no raio. O script ouve `on_event(entity, "interact")`. A barra de status mostra `E —` e o texto enquanto o personagem está no raio. O próprio personagem não interage consigo.

`Health` tem `current` e `maximum` (100). `health.damage` e `health.heal` mudam a vida por comando, sem passar de zero nem do máximo. Se a vida estava acima de zero e, depois dos eventos do quadro, está em zero ou abaixo, o objeto recebe `on_event(entity, "died")` uma vez. `world.set` na mesma coluna também dispara esse aviso.

**Fora deste corte:** som, slot de partida, animação de três estados, projétil e zona de dano.

**Escolha:** duas tabelas e dois nomes de evento, a tecla E fixa.
**Motivo:** porta e item passam a ser um objeto com aviso e um script, sem um sistema de quest. A vida é um número que o resto do combate, mais tarde, só diminui.
**Reabrir se:** um jogo precisar de outra tecla por objeto, ou de um volume que não seja a distância até a posição do objeto.

---

## DR-45 — Modo de edição e inspetor · ACEITA (2026-11-02)

**Pedido:** as ferramentas de malha estavam no modo objeto e não respondiam; o inspetor precisava buscar tabela e script pelo nome; o modo da câmera precisava ser uma lista; uma câmera de sala precisava poder ser escolhida ao interagir.

**Decisão.** Empurrar/puxar e extrudar só aparecem e só funcionam no modo edição. O arame da malha nesse modo fica claro, com um ponto no meio da aresta ou da face conforme a seleção. `Camera.mode` oferece `fixed`, `third`, `first`, `isometric` e `custom` (`custom` não move a câmera sozinho). Interagir com um objeto que tem `Shot` passa a olhar por ele; interagir de novo volta à câmera do personagem. Adicionar tabela abre uma busca. Adicionar script lista os arquivos do projeto, inclusive um novo.

**Fora deste corte:** clima como várias entidades (vento, raio, skybox), e variáveis do script desenhadas no inspetor. A vida e a velocidade do personagem continuam colunas; o script lê com `world.get`.

**Escolha:** consertar o modo que já existe, e nomear a câmera pela tabela `Shot` do objeto.
**Motivo:** o clique do modo edição não chegava na ferramenta de face, e o arame estava escuro demais para ver.
**Reabrir se:** cada componente precisar de um id próprio, separado do objeto, ou o clima tiver de ser gravado na cena.

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

**Responder `DR-11`, pergunta 1** (esta pasta é a retomada do `petunia-game-engine`?), que destrava as outras. Dos itens declarados do `DR-33` ficam decals (bloqueado pelo `DR-12`, que é proposta), LOD em quadtree, tubo de bueiro e partículas na queda — cada um precisa de proposta antes do código.
