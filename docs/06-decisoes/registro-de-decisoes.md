# Registro de decisões

**Revisão:** 2026-10-24 (atualizada após ler a [especificação anterior](../01-pesquisa/05-especificacao-anterior.md))

## Resumo (TL;DR)

São **24 decisões**. Estados possíveis:

- **HERDADA:** já decidida na especificação anterior (10/10/2026). Esta documentação **mantém**; só reabre com um ADR.
- **PROPOSTA:** recomendação minha, aguardando você ou um experimento.
- **ABERTA:** falta informação sua.

`DR-01` (licença do código), `DR-02` (filosofia própria + bibliotecas), `DR-20` (Rust) e `DR-21` (ponte do editor) foram **decididas** em 2026-10-24. `DR-22` (nível 1) está feita; `DR-22` (níveis 2 e 3) e `DR-23` aguardam você. **`DR-11` (nome e relação com os outros projetos) segue aberta, mas não trava o código da fase 1.** As demais podem esperar os experimentos.

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
| DR-08 | Física | PROPOSTA | Cinemática e consultas primeiro; Rapier; XPBD para cordas |
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
| **DR-22** | Edição de primitivas e malha para mapas | **Nível 1 FEITO**; nível 2 PROPOSTO; nível 3 **aguarda você** | Brushes agora, alças de face a seguir; malha só se a necessidade aparecer ([análise](../02-arquitetura/13-primitivas-e-edicao-de-malha.md)) |
| **DR-23** | Pintura e animação do Petunia3D | PROPOSTA | Reprodução e IK sim; autoria e pintura de textura não; só o pincel de mundo ([análise](../01-pesquisa/07-pintura-e-animacao.md)) |
| **DR-25** | Script Host: Aipo embutida, sem acesso ao sistema | **ACEITA** (2026-10-25, a seu pedido) | Aipo por dependência de caminho; módulos `sh`/`fs`/`env`/`io` **removidos**; permissões fixas; orçamento por chamada ([doc](../03-linguagens-e-api/05-script-host.md)) |
| **DR-26** | Layout da janela do editor | **ACEITA** (2026-10-25, a seu pedido) | Uma barra no topo; laterais na altura toda; inspetor em seções; sem botão Jogar até existir o modo ([layout](../02-arquitetura/11-editor.md#layout-da-janela)) |
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

## DR-22 — Edição de primitivas e malha para construir mapas · nível 1 FEITO

**Decisão tomada (2026-10-24):** a engine passa a ter **brushes paramétricos** (caixa, rampa, cilindro, cone, esfera, escada) com gizmo de mover, girar e escalar, snap à grade e desfazer por gesto. Está implementado e testado.

**Proposta, aguardando você:** o que vem depois.

| Nível | Recomendação |
|---|---|
| 2. Edição de brushes (alças de face, cortar, arranjo, espelhar) | **Fazer a seguir** (~1,5 mil linhas, estimativa; sem dependência nova) |
| 3. Edição de malha (vértice, aresta, face) | **Não agora.** Opção A (importar GLB) até a necessidade aparecer; depois opção B (subconjunto de 11,6 mil linhas) |

**Motivo da cautela no nível 3:** o subconjunto do crate `mesh` tem 11.612 linhas, e a engine inteira tem cerca de 8,6 mil. É uma decisão de manutenção. Detalhes, medidas e plano em [primitivas e edição de malha](../02-arquitetura/13-primitivas-e-edicao-de-malha.md).

**Reabrir se:** os usuários da engine precisarem esculpir malhas dentro do editor, ou se o Petunia3D deixar de ser o modelador deles.

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

## DR-26 — Layout da janela do editor · ACEITA (2026-10-25)

**Pedido:** "continue mexendo na ui/ux/design do GUI conforme as referências" (imagens de um redesenho do Unity), com o tema claro como padrão.

**Decisão:**

1. **Uma barra no topo**: menus, ferramentas em ícones, eixos Global/Local e pivô, desfazer, refazer e salvar. Sai o trilho à esquerda e a barra de texto.
2. **Faixa do viewport em uma linha** (snap e andar); as opções de esculpir ficam sob ela, só quando a ferramenta está ativa.
3. **Painéis laterais na altura toda**; os de baixo ficam só sob o viewport, com as abas em cima.
4. **Inspetor em seções que recolhem**, com rótulo à esquerda e o valor à direita; ações raras no menu `⋯`.
5. **Sem botão Jogar** até existir o modo Jogar.

**Por quê:** menos faixas antes da cena (eram quatro, agora duas) e menos texto repetido; os controles ficam onde a referência e as ferramentas conhecidas (Unity, Godot) os põem.

**Sem biblioteca nova:** folhas de estilo do Qt bastam. **Mantido:** atalhos, menus com `Alt`, nomes acessíveis, foco visível e o alvo mínimo de 28 px.

**Reabrir se:** os testes com pessoas (S8) mostrarem que ícones sem texto atrapalham; nesse caso, a barra ganha rótulos.

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
