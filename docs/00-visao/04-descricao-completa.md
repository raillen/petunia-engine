# Petunia Engine — descrição completa

**Revisão:** 2026-10-24 · **Estado:** proposta consolidada. Nada está implementado. Os rótulos `HERDADO` (decidido antes), `PROPOSTO` e `HIPÓTESE` dizem o que é firme e o que é palpite.

## Resumo (TL;DR)

A Petunia Engine é uma **engine 3D em Rust** para criar mundos e jogos com **pouco atrito** e **acessibilidade verificada**.

Ela se define por uma regra: **tudo é um comando com schema**. Criar um rio, abrir uma porta, comprar uma carta e cortar uma corda são comandos. Sistemas internos e atualizações por quadro **não** são comandos. O mesmo comando serve ao editor, às linguagens **Aipo** e **Luau**, às telas visuais, aos testes, ao MCP e à rede.

---

## 1. Identidade

### O que é
Uma engine com: **editor** (Qt), **runtime** (independente do editor), **servidor sem tela** e **ferramentas de linha de comando**. Alvo: **Windows e Linux** primeiro. Web depois.

### Para quem
| Pessoa | O que a engine dá |
|---|---|
| Quem cria mapas e jogos sem programar | Ferramentas diretas, padrões bonitos, regras visuais |
| Pessoa neurodivergente | Menos carga cognitiva, previsibilidade, controle de estímulos |
| Pessoa com deficiência visual ou motora | Teclado completo, leitor de tela, alvos grandes |
| Quem programa | Uma API limpa, dados e funções, sem hierarquia de classes |

### Cinco promessas
1. **Uma API só**, descrita como dado.
2. **Dados em vez de classes.**
3. **Mapas fáceis**: terreno, rios, estradas ligadas, vegetação, clima.
4. **Acessibilidade padrão**, no editor e nos jogos criados.
5. **Online simples**: criar sala, passar código, jogar.

### O que ela não é
Não é um renderer de última geração, nem um modelador (isso é o Petunia3D), nem uma cópia do Godot ou da Unreal, nem uma plataforma com loja e contas.

---

## 2. Princípios

| # | Princípio |
|---|---|
| 1 | Acessibilidade é padrão, não recurso extra |
| 2 | Uma forma só para cada coisa |
| 3 | Palavra completa vence símbolo e abreviação |
| 4 | Dados, não classes |
| 5 | O erro diz o que fazer, em português |
| 6 | Nunca perder trabalho (undo em tudo, sem pânico) |
| 7 | Leve por padrão (hardware modesto) |
| 8 | Previsível (ordem fixa, mudanças em pontos seguros) |

Desempate: perda de trabalho › acessibilidade › hardware modesto › coerência › familiaridade com outras engines.

---

## 3. Visão em camadas

```text
 ┌──────────────────── APPS (finos) ─────────────────────┐
 │ Editor · Jogo (runtime) · Servidor · CLI · MCP         │
 └────────────────────────┬───────────────────────────────┘
                          │ comandos e consultas
 ┌────────────────────────▼───────────────────────────────┐
 │  Aplicação: despachante de comandos + undo/redo        │
 └──────┬──────────────────────────────────────┬──────────┘
        │                                      │
 ┌──────▼───────┐                     ┌────────▼─────────┐
 │   DOMÍNIO    │◄──── schema ───────►│     SCRIPTS      │
 │ mundo, mapa, │                     │ Aipo · Luau ·    │
 │ regras, dados│                     │ Event Sheet/nós  │
 └──────┬───────┘                     └──────────────────┘
        │ quadro extraído (somente leitura)
 ┌──────▼────────────────────────────────────────────────┐
 │ SERVIÇOS: render · animação · física · cordas · rede · │
 │ áudio · entrada · UI do jogo · grade/navegação         │
 └────────────────────────────────────────────────────────┘
```

**Regras de dependência**
1. O domínio não conhece GPU, janela nem Qt.
2. Módulos não se conhecem; falam por eventos e registro.
3. Só o crate de render toca a GPU.
4. **O runtime nunca depende do editor nem do Qt.**
5. Todo dado externo (arquivo, script, rede, mod) é hostil: tem limites e devolve `Result`.

**Decisão de base (2026-10-24):** engine **própria**, com **bibliotecas auxiliares atrás de adaptadores** ([lista](../02-arquitetura/10-bibliotecas-auxiliares.md)). Filosofia de engenharia igual à do Digi-Tactics ([análise](05-filosofia-de-engenharia.md)).

**Decisões herdadas** (`HERDADO`): Rust · Qt 6/CXX-Qt no editor · OpenGL leve · `RuntimeWorld` separado da autoria · crates em `petunia-libs` · GLB como intercâmbio · formatos de projeto em texto.

---

## 4. O mundo e os dados

O mundo é um conjunto de **tabelas**. Cada linha é uma **entidade**. Cada coluna é um dado.

| Peça | O que é |
|---|---|
| **Entidade** | Uma linha; identidade por handle (índice + geração) |
| **Tabela** | Conjunto de colunas: `Transform`, `Door`, `Health`, `Rope`… |
| **Tag** | Marca sem dados: `Tree`, `Enemy` |
| **Regra** | "Quando X, se Y, faça Z" |
| **Sistema** | Função nativa que roda numa fase fixa |
| **Chunk** | Célula espacial para streaming, vegetação e rede |
| **Prefab** | Molde reutilizável de linhas, com revisão e instâncias |

**Duas identidades:** UUID **persistente** (vai para o arquivo) e handle de **sessão** (só nesta execução). Handle velho vira "obsoleto", nunca aponta para outra coisa.

**Mudanças estruturais** (criar, remover) entram num buffer e só são aplicadas num ponto seguro do quadro.

**Hierarquia** é uma coluna `parent` mais um sistema de propagação.

**Autoria e runtime são o mesmo modelo.** O editor só acrescenta colunas próprias (`editor.locked`, `editor.hidden`), que a exportação remove.

### Um quadro, sempre na mesma ordem
```text
 1 Entrada               5 Cordas                9 Rede (enviar)
 2 Rede (receber)        6 Eventos pós-física   10 Extrair quadro p/ render
 3 Regras e scripts      7 Aplicar comandos     11 Renderizar
 4 Física                  adiados (spawn)
                         8 Mundo vivo (clima, vento, água)
```
Passo **fixo** (lógica, física, cordas) separado do desenho, que interpola.

### Determinismo
Semente única por mundo · ordem fixa por id · tempo fixo · threads combinadas em ordem fixa · física em modo determinístico quando disponível. Determinismo entre plataformas é **meta**, não garantia.

### Salvar
Estado de jogo = **só tabelas** (pequeno, repetível, bom para replay). Escrita **atômica**. Versão de schema com migração explícita.

---

## 5. O contrato: comandos, consultas e eventos

Esta é a ideia central.

| Conceito | O que é |
|---|---|
| **Comando** | Muda algo. Tem id estável, validação prévia, execução transacional, undo (quando aplicável) |
| **Consulta** | Só lê. Devolve cópia, nunca referência |
| **Evento** | Algo aconteceu (`touch`, `rope_cut`, `weather_changed`) |

Cada entrada do **schema** declara:

| Campo | Exemplo |
|---|---|
| Nome e módulo | `rope.create` |
| Resumo (1 linha) | "Cria uma corda entre duas âncoras." |
| Parâmetros | nome, tipo, **unidade**, faixa, padrão |
| Retorno | handle `Rope` |
| Falhas | código estável + sugestão de correção |
| Permissão | `world.write`, `physics.ropes` |
| Custo | barato ou **orçado** |
| Desfazer | sim/não |
| Exemplo | curto e **testado** |
| Aparência de nó | rótulo, cor, pinos |

**Do schema saem:** bindings Aipo · tipos Luau (`.d.luau`) · catálogo de nós e Event Sheet · autocomplete e ajuda · documentação · ferramentas MCP · validação de comandos de rede e CLI · controles do inspetor.

Base já existente: **AHS** da Aipo (schema, permissões, valores, falhas) e **`CommandIntent` + JSON Schema** do Petunia3D.

### Permissões
Nega por padrão. Só se pode **reduzir**, nunca ampliar. Sem arquivos, rede ou processos por padrão.

| Contexto | Permissões |
|---|---|
| Script do jogo | ler/escrever mundo, física, UI, som |
| Ferramenta do editor | + comandos do editor, importar |
| Mod de terceiros | só o manifesto **e** a aprovação do jogador |
| Agente de IA (MCP) | política própria; passa pelo mesmo despachante |

---

## 6. Linguagens e lógica

### Uma linguagem ativa por projeto (`HERDADO`)
| | **Aipo** (padrão) | **Luau** (alternativa) |
|---|---|---|
| Papel | Primeira classe | Para quem vem de Lua/Roblox |
| Sandbox | orçamento de instruções + permissões | globais somente leitura + limite de CPU |
| Tipos | contratos opcionais | graduais (`strict`/`nonstrict`) |
| Ferramentas | `check`, `fmt`, `test` (LSP a construir) | `luau-lsp` com definições geradas |

Luau com JIT nativo fica **desligado** na v1 (experimental no `mlua`).

### Três formas de escrever lógica
| Forma | Para quem | Como |
|---|---|---|
| **Event Sheet** | Quem não programa | Lista **QUANDO / SE / FAÇA**; cada linha é uma chamada da API |
| **Grafos especializados** | Estados e comportamento de IA | Máquina de estados; árvore de comportamento |
| **Script** | Quem programa | Aipo ou Luau, mesma API |

**Gameplay IR:** representação **tipada e neutra** das regras visuais, com ids estáveis. As telas editam o IR. Botão **"Ver como código"** gera Aipo ou Luau legível. Editar o código gerado **separa** o bloco do visual. As telas **não espelham a AST**: o valor está nos verbos grandes (`rope.create`, `weather.set`).

**Vista de nós** (avançada): fios só para **dados**; o **fluxo é empilhado**. Toda tela visual tem **vista em contorno** equivalente para teclado e leitor de tela.

### Desempenho dos scripts
A Aipo custa ~100 ns por instrução (`LOCAL`). Contrato: **API em lote** (`world.set_all`), regras reagem a eventos, matemática pesada fica na engine, o editor mostra o **custo por script**, e `aipo-wasm` é a saída de emergência.

### Outros recursos de script
Hot reload preservando as tabelas · tempo virtual da Aipo mapeado ao tempo de jogo · pacotes com SHA e lockfile (`aipo-package`) para compartilhar módulos e mods.

---

## 7. A API, por módulo

Nomes são **substantivos do criador de mundos**. Verbos são um **conjunto fechado**: `create`, `remove`, `get`, `set`, `find`, `on`, `play`, `stop`, `show`, `hide`, `move`, `share`.

| Módulo | Para quê |
|---|---|
| `world` | Entidades, tabelas, consultas em lote |
| `input` | Ações nomeadas (`jump`, `move_right`), nunca teclas |
| `rules` | `rules.on(evento, …)` |
| `time` | Passo fixo, `after`, `every` |
| `camera` | Seguir, travar, calibrar, tremer (respeita "reduzir movimento") |
| `animation` | Papéis: `idle walk attack skill hit death victory` |
| `physics` | Corpos, personagem, gatilhos, raios |
| `rope` | Cordas, cabos, pontes, ganchos |
| `grid` | Hex/quadrada, distância, vizinhas, A\* |
| `nav` | Waypoints, caminhos, Patrol, Chase |
| `terrain` | Esculpir, pintar, materiais |
| `river` | Curvas, largura, fluxo, quedas |
| `road` | Rede, cruzamentos, pontes |
| `plant` | Espalhar por regras |
| `weather` | Presets, hora do dia, zonas |
| `decal` | Projetar marcas |
| `particles` | Efeitos |
| `sound` | Tocar, mixer, legendas |
| `ui` | Elementos, arrastar e soltar, dicas |
| `net` | Sala, compartilhar, entrar |
| `save` | Salvar/ler com miniatura |
| `text` | Tradução (en, pt-BR) |
| `access` | Expor variáveis ao menu de assistência |
| `debug` | Desenhar, registrar, medir |

### 12 regras da API
Verbos primeiro · uma forma só · substantivos do domínio · **argumentos nomeados** · padrões bons · dados entram e saem · **unidades fixas** (metros, segundos, graus) · nada "à distância" · erro com correção · nada falha em silêncio · módulos pequenos (~12 funções) · todo recurso com **exemplo testado**.

```aipo
# Ilustrativo (Aipo real, não é contrato)
rules.on("touch", tag = "Door", do_this = fn(event) {
    if inventory.has(event.who, item = "chave_azul") {
        world.set(event.target, open = true)
        sound.play("porta_abre", at = event.target)
    } else {
        ui.say("Precisa da chave azul.")
    }
})
```

---

## 8. Renderização

**Base** (`HERDADO`): **OpenGL 3.3 leve, sem compute**, atrás de uma **fronteira neutra de quadro**. Nunca dois renderers em paridade. Plano B: `wgpu`, só com decisão registrada.

### Passes
```text
 sombras (cascatas) → pré-passe de profundidade → opaco (PBR simples)
 → terreno → vegetação → estradas e decals → céu → água
 → transparentes → pós-processamento → UI do jogo
```

### Materiais
PBR metal/rugosidade (glTF) · **toon** com contorno (original da engine) · presets de mundo (terra, grama, pedra, asfalto, água, lama) · grafo de materiais **só no modo avançado**.

### Pós-processamento (cadeia curta e fundida)
HDR → AO opcional → **bloom** em meia resolução → **um passe**: tonemap **AgX** (alternativa: Khronos PBR Neutral) + LUT 3D + vinheta + grão → **SMAA** (FXAA no perfil Simples).

### Conforto sensorial (parte do pós, não complemento)
| Controle | Efeito |
|---|---|
| **Limitador de flash** | Limita mudança brusca de luminância |
| **Reduzir movimento** | Sem tremor, balanço, desfoque |
| **Estímulo geral** | Controla partículas, saturação, bloom |
| **Paletas para daltonismo** | Mais simulação **só** como ferramenta do criador |

### Perfis e orçamentos
**Simples · Padrão · Alto.** Cada passe tem **orçamento declarado** e visível. Escala dinâmica de resolução opcional. Nenhum número prometido antes de medir.

### Personagens
Animação esquelética (**tocar**, não criar): glTF com skin, GPU skinning, mistura de clipes (padrão 0,15 s), clipes de uma vez que voltam a `idle`, estados **terminais** (`death`, `victory`). Papéis são **dados** (mapa papel → clipe).

### Outros
**Pano de fundo 2.5D** (arte pintada + câmera calibrada) · **partículas** · **billboards** (axial, cruzado, octaédrico) · decals.

---

## 9. O mundo natural

Terreno, rios e estradas são da **mesma família**: curva + perfil + efeito no terreno.

| Sistema | O que o criador faz | Como funciona |
|---|---|---|
| **Terreno** | Escolhe um bioma, esculpe e pinta | Altura em **chunks** com LOD e costura; camadas de material; pincéis (subir, descer, suavizar, aplainar, ruído, erosão, carimbos); collider de heightfield; undo por pincelada |
| **Rios** | Clica da nascente à foz | Spline com largura e profundidade; valida que o rio **desce** (oferece baixar o leito); **escava** com margens suaves; malha por varredura; **vetor de fluxo** nos vértices; espuma de margem barata; confluências, lagos, quedas; zonas de nado e **correnteza** |
| **Estradas** | Traça; cruzamentos nascem sozinhos | **Grafo** de nós e trechos; cruzamento por ordenação de ângulo e triangulação; terreno aplainado com aterro/corte; **pontes** e bueiros automáticos; faixas por **decals**; vira grafo de navegação; avisa pontas soltas e ilhas |
| **Vegetação** | Pinta uma área e escolhe "Floresta" | **Regras** (densidade pintada, inclinação, altitude, distância da água, exclusão por estrada, agrupamento, semente). Guarda **regras e retoques**, não milhões de instâncias. Desenho instanciado por chunk; **vento único** para todos os LODs; LOD até impostor |
| **Clima** | Move a hora do dia, escolhe um preset | **Estado único** (hora, nuvens, chuva, neve, névoa, vento, umidade) → céu, nuvens, precipitação presa à câmera, molhado/poças/neve no material, vento, áudio. Transições interpoladas e com **tempos diferentes** (o chão seca devagar). Zonas |
| **Decals** | Coloca lama, marcas, rachaduras | Projeção sobre o opaco usando profundidade; **orçamento e vida útil**; máscara de quem recebe; decal em faixa ao longo de spline; opção "sem sangue e gore" |
| **Billboards 3D** | Automático por LOD | Impostor octaédrico assado a partir do modelo |

---

## 10. Física, cordas, grade e IA

### Física leve
Cinemática e consultas primeiro (`HERDADO`). **Rapier** para corpos simples e personagem. Camadas por **nome**, não por bits. Orçamento e "dormir" quando parado. Sem veículos, ragdoll ou destruição na v1.

### Cordas (recurso de primeira classe)
**Solver próprio XPBD** (o Rapier só tem junta de distância). Corda, cabo, corrente, **ponte de corda**, gancho e balanço. Âncoras em corpos ou em pontos de superfície. Comprimento variável (guincho). Corte por script, dano ou ferramenta. Colisão por esferas varridas. Tensão e rompimento. Vento e água. Malha por varredura. Tecido leve depois. **Orçamento global** de partículas.

### Personagem
Controlador cinemático (degraus, rampas, plataformas móveis) com **assistências** de acessibilidade (tempo de coiote, ajuda em beirada).

### Grade e navegação
`grid` hex/quadrada com coordenadas cúbicas · A\* · waypoints · Patrol e Chase · depuração visual do caminho.

---

## 11. Áudio, entrada e UI de jogo

| Área | Descrição |
|---|---|
| **Áudio** | SFX, música, ambiente 3D com atenuação; **mixer com 4 barras** (efeitos, música, voz, ambiente); fades; **legendas de som**; opção de volume previsível |
| **Entrada** | **Ações nomeadas**, nunca teclas; remapeamento em execução; teclado, mouse e gamepad; alternar em vez de segurar; perfis salvos |
| **UI de jogo** | UI retida **descrita como dados**; texto, imagem, botão, barra, lista, grade, diálogo; **arrastar e soltar com alternativa por teclado**; dicas; layout por âncoras; **en/pt-BR em paridade**; árvore de acessibilidade (AccessKit); ligação a tabelas |

---

## 12. Online

**Nível 1 (v1):** um jogador é anfitrião e os outros entram por **código curto**. **Servidor com autoridade.**

| Tema | Desenho |
|---|---|
| Replicação | Tabelas e colunas marcadas com `net.share`; só **deltas** |
| Entrada | Clientes mandam **comandos**; servidor valida |
| Suavização | Interpolação dos outros; **predição** só do próprio personagem |
| Interesse | Cada cliente recebe os **chunks** perto dele |
| Identidade | Sem conta obrigatória |
| Chat | **Frases prontas e emotes** por padrão; texto livre opcional |
| Segurança | Silenciar/bloquear a um clique |
| Teste | Botão **"Testar online"**: N clientes + servidor, com latência e perda simuladas |
| Transporte | Réplica própria sobre `renet` (a validar); `matchbox` (WebRTC) para web/P2P |

Nível 2: servidor dedicado (mesmo binário, `--headless`). Nível 3: *rollback* só se houver demanda. **Pendência de produto:** quem hospeda o relay (padrão: autohospedado).

---

## 13. O editor

Qt 6 / CXX-Qt (`HERDADO`), com viewport OpenGL. **Geografia fixa** (cabeçalho, trilho de ferramentas, superfície de trabalho, estrutura, propriedades, gaveta), igual em todos os modos.

### Sete modos
| Modo | Conteúdo |
|---|---|
| **Mundo** | Terreno, água e rios, estradas, vegetação |
| **Construir** | Level Builder: **brushes convexos** (caixa, rampa, cilindro), grade com snap, depois assistente de salas, portas, janelas, escadas |
| **Objetos** | Props, prefabs, entidades com comportamento |
| **Atmosfera** | Hora, clima, pós, pano de fundo 2.5D |
| **Lógica** | Event Sheet, grafos, vista de nós |
| **Código** | Editor de texto (Aipo/Luau) |
| **Jogar** | Testar no editor; testar online |

### Princípios de uso
Bom por padrão (**modelos iniciais**: Ilha, Vila, Floresta, Corredor, Dungeon, Tabuleiro) · auto-popular com um clique · **edição ao vivo** · undo ilimitado · colocar sem mouse (teclado, valor numérico) · encaixe no chão e na normal · **simples por padrão, "Mostrar mais" para o avançado** · blockout primeiro.

Toda ferramenta tem **4 caminhos**: predefinição, campo numérico, atalho e desfazer.

### Project Doctor (validador)
Severidade, **código estável**, alvo, mensagem simples, ação, botão **"Corrigir"** ou **"Mostrar onde"**.

| Grupo | Exemplos |
|---|---|
| Mundo | Rio que sobe; estrada com ponta solta; ilha de estradas |
| Jogo | Porta sem gatilho; spawn fora da navegação; sem início do jogador |
| Física | Objeto sem colisão; corda sem âncora |
| Desempenho | Acima do orçamento |
| Script | Diagnósticos da Aipo e do Luau |
| **Acessibilidade** | Tecla fixa; flash sem limite; contraste; informação só por cor ou som; contagem sem pausa |

### Editor de código integrado
Destaque · diagnósticos ao digitar · formatar ao salvar · autocomplete/ajuda/assinatura vindos do **schema** · ir para definição e renomear (via `aipo-lsp`, a construir) · mensagens em português · fonte e espaçamento configuráveis · régua de foco · **modo calmo**.

### Outras ferramentas
Navegador de assets com busca · inspetor em runtime · **depurador de regras** ("por que esta condição falhou?") · profiler com orçamentos · paleta de comandos · atalhos remapeáveis em perfis · **MCP** sem editor aberto (`map.get`, `brush.create`, `trigger.create`, `event.list`, `play.test`).

---

## 14. Acessibilidade

### No editor
| Pilar | Itens |
|---|---|
| **Teclado completo** | Paleta de comandos; foco previsível; colocar objeto e esculpir sem arrastar; câmera por teclado/controle; sem tempo limite |
| **Leitor de tela, contraste e escala** | Árvore de acessibilidade em todo controle; **estrutura da cena narrada**; temas claro, escuro e alto contraste; escala de 100% a 300%; cor nunca é o único sinal |
| **Carga cognitiva baixa** | Modo simples; modo foco; layout fixo; mensagens simples; sem pop-ups surpresa; resumo ao reabrir; tutoriais curtos |

Viewport 3D: painel narrado, comandos "Descrever seleção" e "O que está perto?", e consulta em linguagem natural via MCP (sempre confirmar, sempre desfazer).

### Nos jogos criados (kit incluído, ligado por padrão)
Remapear controles · alternar em vez de segurar · pausar em qualquer momento · limitador de flash · reduzir movimento · mixer de 4 barras · volume previsível · desligar sangue e gore · legendas com nome e legendas de som · escala de interface e alto contraste · leitor de tela na UI · autosave com miniatura · dificuldade ajustável **durante o jogo** (`access.expose`) · pular partes não essenciais · chat por símbolos.

**Perfis sensoriais:** Padrão · **Calmo** · Foco · Personalizado. Escolhidos no primeiro início (tela curta, pulável).

### Como se verifica
Gates automáticos (rótulos, foco, árvore de acessibilidade, contraste AA) + **teste com pessoas**. Sem teste humano a engine diz "passa nos gates", não "é acessível".

---

## 15. Assets e formatos

| Tema | Desenho |
|---|---|
| Modelos | **GLB/glTF** (principal), OBJ secundário |
| Projeto | **Pasta de arquivos de texto**, difáveis e amigáveis ao Git, com ids persistentes |
| Exportação | **ZIP versionado** com manifesto; sem `..` nos caminhos; limites anti zip-bomb; escrita atômica |
| Cozimento | Remove clipes e buffers não usados; carga sob demanda; texturas |
| Migração | Explícita por versão; nunca renomear em silêncio |
| Procedência | `ATTRIBUTION.md` obrigatório; ativos de jogos comerciais ficam **fora do Git** |
| Petunia3D | Produz modelos, rigs e animações; ponte por GLB; **Live Link** depois |
| Reuso de código | Por **`petunia-libs`** (crates pequenas e versionadas) |

---

## 16. Qualidade de engenharia

| Regra | Detalhe |
|---|---|
| **Nunca travar** | Sem `unwrap`/`panic` em produção; último recurso com `catch_unwind` |
| **Dado hostil** | Limites de tamanho, profundidade e contagem; floats finitos |
| **Tudo é comando** | Com id, validação, transação e undo |
| **Pirâmide de 6 camadas** | Domínio → contratos → GUI sem tela → fuzz → regressão visual → **acessibilidade** |
| **Entrega verdadeira** | Cada item é `passou`, `falhou`, `não rodou` ou `bloqueado` |
| **Teste diferencial** | Duas versões, mesma semente, mesmo resultado |
| **Exemplos da doc rodam nos testes** | A doc não mente |
| **Recursos do host** | Orçamento de jobs de compilação e disco |

---

## 17. Plataformas

| | Plano |
|---|---|
| **Windows e Linux** | Alvo da v1 (`HERDADO`) |
| **Web** | Depois. Para manter a porta aberta: renderer no subconjunto **WebGL2/GLES3** e runtime sem Qt (`HIPÓTESE`) |
| Consoles, VR | Fora de escopo |

---

## 18. O ecossistema

```text
   Petunia3D ──GLB──► Petunia Engine ◄── Aipo (linguagem, MIT)
   (modelador)        (editor+runtime)        │
        ▲                    ▲                │
        └──── petunia-libs ──┴────────────────┘
```

**Provas de uso:** **Digi-Tactics** (auto-battler: jogabilidade, animação, UI, rede) e a **Ilha** (rios, estradas, clima, cordas).

---

## 19. Fora da v1
Consoles · VR · ray tracing · animação esquelética de **autoria** · veículos, ragdoll, destruição · FBX · loja, nuvem, contas · plugins nativos (DLL) · mundo infinito · MMO · voz · cinemáticas · grafo de materiais completo · *rollback* · exportação web · diálogos e quests · navmesh automática · CSG avançado.

---

## 20. Estado e o que falta decidir

**Estado:** só documentação. Nenhum código foi escrito.

| Decisão | Estado |
|---|---|
| Licença do código entre seus projetos | **Decidida** |
| Licença pública do runtime | Aberta (recomendo permissiva) |
| Objetivo principal (engine própria + bibliotecas) | **Decidido** em 2026-10-24 |
| Nome final e relação com o repositório `petunia-game-engine` e a "Poppy" | Pendente |
| Provas e regras dos ativos do Digimon (`DR-16`) | Pendente de aprovação |
| UI de jogo (`DR-17`) | Depende do experimento S9 |

**Cinco maiores riscos:** escopo · uma pessoa só · Aipo em tempo real · custo do Qt/C++ na sua máquina · direitos dos ativos de terceiros.

**Antes de codar:** experimentos **S1 a S4** (Aipo, Luau, OpenGL, Qt).

---

## 21. Ordem de construção

| Fase | Entrega |
|---|---|
| 1 | Esqueleto: `check` e `run`, sala de brushes, flycam |
| 2 | Editor Qt e Level Builder |
| 3 | Regras, Script Host, Gameplay IR, `grid`, núcleo do Digi-Tactics em Aipo |
| 4 | Personagens, UI de jogo, toon, 2.5D (**Prova 1**) |
| 5 | Mundo natural (**Prova 2**) |
| 6 | Física, cordas, navegação e IA |
| 7 | Online |
| 8 | Intercâmbio, MCP, exportação |
| 9 | Acabamento e teste com pessoas |

Cada fase termina com uma **demo** que roda.
