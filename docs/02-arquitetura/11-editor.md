# Editor (fase 1): sessão, operações e interface Qt

**Revisão:** 2026-10-24 · Estado: **primeira versão funcionando** (veja [o que foi verificado](../05-roadmap/03-estado-da-implementacao.md))

## Resumo (TL;DR)

O editor é uma **janela Qt fina** (com o tema, os ícones e os campos do Petunia3D, veja [paridade](12-paridade-ui-petunia3d.md)) sobre uma **sessão em Rust**. A janela só envia pedidos em JSON e desenha o que volta. A sessão guarda o projeto, o histórico de desfazer, a seleção e a câmera. Toda mudança na cena passa por **comandos validados**, então tudo dá para desfazer.

Como abrir:

```bash
scripts/editor.sh
```

## Exemplo

A janela pede a criação de uma caixa. O pedido é só dado:

```json
{ "op": "create.brush", "shape": "box" }
```

A sessão responde:

```json
{ "ok": true, "result": { "entity": 13 } }
```

Se algo falha, a resposta explica o que fazer:

```json
{ "ok": false, "error": { "code": "EDITOR_NO_PROJECT",
  "message": "nenhum projeto está aberto",
  "action": "Use Arquivo > Novo projeto de exemplo, ou Arquivo > Abrir projeto." } }
```

## Como funciona

```text
 Qt (C++)                    Rust
┌──────────────────┐        ┌────────────────────────────────────────────┐
│ MainWindow       │        │ petunia_editor_ffi   (C, único com unsafe) │
│  Hierarquia      │  JSON  │   └─ petunia_editor  (EditorSession)       │
│  Inspetor        │ ─────► │        ├─ Dispatcher  (comandos + desfazer)│
│  Diagnósticos    │ ◄───── │        ├─ World       (tabelas de dados)   │
│  Histórico · API │        │        └─ câmera, seleção, picking         │
│ ViewportWidget ──┼─ GL ──►│ petunia_render_gl    (OpenGL 3.3)          │
└──────────────────┘        └────────────────────────────────────────────┘
```

- **A janela não tem regras do jogo.** Ela monta campos a partir dos tipos das colunas que o motor informa. Uma tabela nova aparece no inspetor sem mudar a interface.
- **Seleção e câmera são estado do editor**, não do mundo. Criar, mudar e remover são **comandos**.
- **A mesma sessão serve** a janela, a automação e, no futuro, agentes (MCP). Não há segundo caminho.
- **A janela nunca recebe um mundo mutável.** Ela só lê cópias (snapshot).

## Operações (`{ "op": "...", ... }`)

| Operação | Campos | O que faz |
|---|---|---|
| `project.new_sample` | `path`, `name` | Cria a sala de exemplo na pasta e abre |
| `project.open` | `path` | Abre um projeto (ou explica o que está errado) |
| `project.save` | — | Grava a cena (atômico). Recusado com `EDITOR_BUSY` enquanto um arrasto altera o mundo; solte o mouse ou `Esc` antes |
| `editor.snapshot` | — | Resumo: objetos, seleção, desfazer/refazer, diagnósticos |
| `entity.describe` | `entity` | Tabelas, tipos e valores de um objeto |
| `diagnostics.get` · `history.get` · `schema.get` | — | Project Doctor, histórico, API |
| `command.execute` | `command`, `payload` | Roda um comando do motor (`world.set`, `world.create`…) |
| `query.run` | `query`, `payload` | Roda uma consulta (`world.get`, `world.find`) |
| `edit.undo` · `edit.redo` | — | Desfaz e refaz |
| `edit.duplicate` · `edit.delete` | — | Copia ou remove a seleção |
| `create.brush` | `shape` (`box`, `ramp`, `cylinder`, `cone`, `sphere`, `stairs`) | Cria na frente da câmera, no chão, alinhado a 0,5 m |
| `selection.set` | `entity` (ou nada) | Define a seleção |
| `selection.pick` | `x`, `y`, `aspect` | Seleciona o que está sob o ponto da tela |
| `camera.look` · `camera.advance` | movimento | Gira e anda |
| `camera.get` · `camera.reset` · `camera.focus` | — | Lê, volta ao início, aproxima da seleção |
| `camera.view` | `name` (`front`, `back`, `left`, `right`, `top`, `bottom`) | Olha para a seleção ou a cena nessa direção |
| `tool.set` | `tool` (`select`, `move`, `rotate`, `scale`, `push_pull`, `extrude`, `sculpt`, `sculpt_path`) | Escolhe a ferramenta |
| `snap.set` | `enabled`, `translate`, `rotate_degrees`, `scale`, `target` (`smart`, `grid`, `increment`, `point`, `face`), `align_to_slope` | Liga o snap, muda os passos e o alvo |
| `gizmo.hover` · `gizmo.begin` | `x`, `y`, `width`, `height` (pixels) | Qual alça está sob o mouse; começa o arrasto |
| `gizmo.drag` · `gizmo.end` · `gizmo.cancel` | idem (`invert_snap` opcional) | Move ao vivo; **um** passo no histórico ao soltar |
| `gesture.preview` · `gesture.commit` · `gesture.cancel` | `entity`, `table`, `column`, `value` | O mesmo mecanismo, usado pelos campos de número; várias colunas fecham em **um** `world.batch` |
| `pointer.hover` | `x`, `y`, `width`, `height` | Alça do gizmo ou objeto sob o mouse (pré-seleção) |
| `transform.set` | `orientation` (`global`, `local`), `pivot` (`object`, `bounds_center`, `base`), `edit_pivot` | Eixos do gizmo, ponto de pivô, modo **Editar pivô** |
| `free.begin` · `free.axis` · `free.type` | `tool`, pixels / `axis` / `text` | **Modo Livre** (duplo toque em G, R ou S) |
| `face.begin` · `face.drag` · `face.end` · `face.cancel` | pixels | **Empurrar/puxar** e **Extrudar** uma face |
| `script.list` · `script.read` · `script.new` | — / `name` | Scripts da pasta `scripts/`; criar a partir do modelo (nunca sobrescreve). `script.list` também devolve `missing`: nomes pedidos por uma tabela `Script` cujo arquivo ainda não existe |
| `programming.list` | — | Scripts `.aipo` e grafos `.graph.json`, com **os objetos que usam cada um**; nomes pedidos por uma tabela `Script` sem arquivo aparecem como `missing` |
| `script.write` | `name`, `text` | Grava; a versão anterior vai para `scripts/.backup/` |
| `script.rename` | `name`, `new`, `kind`? | Renomeia o `.aipo`/grafo (e o `.layout.json`) e atualiza os anexos; recusa nome ocupado |
| `script.delete` | `name`, `kind`?, `force`? | Move o arquivo para `scripts/.backup/` e desliga os anexos com `force`; sem `force`, recusa se estiver ligado |
| `script.check` · `script.format` | `name`, `text` | Erros com linha e coluna; formatar (só texto que passa na sintaxe) |
| `script.run` | `name`, `text`?, `ticks`? | Executa numa **cópia** do mundo; devolve console, falhas e quantos objetos mudariam |
| `script.api` · `script.attach` | — / `name` | API para sugestões e ajuda; **liga mais um** script ao objeto selecionado (um desfazer). O primeiro fica na linha legada do objeto; o segundo vira uma entidade-filha com `owner` e `order`, e os dois rodam |
| `script.to_graph` | `name`, `graph`, `text`? | Cria um grafo novo (schema v2) a partir do subconjunto Aipo suportado (DR-34/DR-35), incluindo funções auxiliares, globais, variáveis locais, `while`, `loop`, `repeat`, `each`, `break`/`continue` e `return`. `text` permite usar o editor com alterações não salvas; o script original **não** muda e um grafo existente **nunca** é sobrescrito |
| `create.cave` | `seed` (opcional) | Bloco de rocha com um túnel; com `seed`, com salas e túneis **gerados** (um desfazer) |
| `create.light` | `kind` (`point` ou `sun`) | Luz na frente da câmera (um desfazer); com uma luz, só as luzes da cena valem (DR-29) |
| `asset.list` | — | Modelos `.glb`/`.gltf` em `assets/` (caminhos do projeto; não segue links simbólicos) |
| `asset.import` | `path` | Importa um modelo como **malhas editáveis**, um objeto por nó; copia o arquivo para `assets/` (um desfazer). Devolve `imported`, `names`, `asset`, `warnings` |
| `prefab.list` | — | Os prefabs embutidos: `door` (Porta), `pickup` (Item coletável), `spawn` (Ponto de nascimento) (DR-31) |
| `prefab.spawn` | `id` | Cria o prefab na frente da câmera, no andar atual, em **um** desfazer; grava os grafos de que precisa em `scripts/` sem sobrescrever os que já existem. Devolve `entity`, `graphs_written`, `graphs_kept` |
| `graph.list` / `graph.new` `name` / `graph.open` `name` / `graph.close` | — | Grafos de nós (`scripts/<nome>.graph.json`, DR-30). Um grafo aberto por vez; cada edição **grava na hora** (atômico) e a primeira de uma abertura guarda o arquivo em `scripts/.backup/` |
| `graph.view` | — | O grafo aberto como **contorno** (eventos com suas cadeias, entradas com a origem de cada valor, ramos, soltos e diagnósticos) **e** como **canvas** (`canvas`: caixas com posição, pinos, problemas e fios); `layout_problem` quando o `.layout.json` foi ignorado; `can_undo` / `can_redo` |
| `graph.add` | `kind`, `after`? `{node,pin}`, `into`? `{node,pin}` | Põe um nó (na cadeia, depois de `after`; ou ligando o resultado a uma entrada `into`) |
| `graph.remove` | `node` | Tira o nó e fecha a cadeia sobre o buraco |
| `graph.set` | `node`, `name`, `value` | Constante de uma entrada ou opção do nó (`null` limpa); uma constante substitui o fio |
| `graph.connect` / `graph.disconnect` | `from`, `to` / `pin` | Liga ou desliga; recusa com a razão (tipos, ordem, volta de execução) |
| `graph.undo` / `graph.redo` | — | Histórico **do grafo** (até 100 passos); a cena tem o dela |
| `graph.search` | `query`, `want` (`chain`, `event`, `value`, `any`), `for`? | A paleta: nós que combinam (sem acento, qualquer ordem) |
| `graph.sources` | `for` `{node,pin}` | O que uma entrada pode receber: valores que já existem quando o nó roda |
| `graph.code` | — | **Ver como código**: o texto Aipo e o nó de cada linha, ou os problemas |
| `graph.set_layout` | `node`, `x`, `y` | Onde a caixa fica em `scripts/<nome>.layout.json` (DR-32). Não entra no desfazer: posição não é lógica |
| `graph.set_layouts` | `nodes`: `[{node,x,y}]` | Move várias caixas numa gravação só (arrastar um grupo) |
| `graph.group` / `graph.ungroup` | `label`, `nodes` / `group` | Agrupa caixas num quadro visual salvo no layout; desagrupar não move os nós |

No canvas (**Caixas e fios**): arraste uma caixa para mover; arraste de uma **saída** até uma **entrada** para ligar (clicar saída e depois entrada também funciona); arraste o fundo ou use o botão do meio para mover o canvas; `Ctrl`+roda, `+`, `-` e os botões dão zoom; `Home`, `F` ou **Centralizar** enquadram os nós; `Ctrl+clique` soma caixas à seleção e `Ctrl+A` escolhe todas; **Agrupar** cria um quadro que move as caixas juntas. Um pino de objeto (`entity`, `other`, `target`) tem **Escolher objeto…** com busca por id e nome.
| `sculpt.begin` · `sculpt.drag` · `sculpt.end` · `sculpt.cancel` | pixels, `invert` | Esculpe a caverna sob o mouse; o arrasto vira **um** `world.batch` |
| `sculpt.mode` | `mode` (`carve`, `add`, `inflate`, `flatten`, `smooth`), `force` (0 a 1) | Modo e força do pincel |
| `sculpt.radius` | `meters` (0,25 a 16) | Raio do pincel |
| `path.point` · `path.pop` · `path.commit` · `path.cancel` | pixels, `invert` | Túnel por caminho: pontos na rocha, `Enter` cava (um desfazer) |
| `sculpt.generate` | `seed`, `rooms`, `worms`, `radius` | Soma salas e túneis gerados à caverna selecionada |
| `sculpt.consolidate` | — | Funde traços em cadeia (um desfazer) |
| `sculpt.faceted` | `value` | Triângulos chapados (low poly) na caverna selecionada |
| `brush.carve` · `brush.hollow` | — / `thickness` | **Cavar** (a seleção corta as caixas que toca) e **Tornar oco** |
| `mesh.convert` | `entity`? | **Tornar editável**: o brush vira malha (`Mesh`), mesma entidade, mesma aparência (um desfazer) |
| `mesh.boolean` | `operation` (`union`, `subtract`, `intersect`), `tool`, `entity`?, `keep_tool`? | **Fundir, cortar, interseção** com outro brush ou malha, em qualquer rotação e escala; o alvo vira malha; a ferramenta some (a menos que `keep_tool`). Um desfazer |
| `mesh.pick` | `x`, `y`, `width`, `height`, `entity`? | Face, aresta (com a fração ao longo dela), vértice e ponto sob o mouse |
| `mesh.select` | `faces`, `edges` (`[[a, b]]`), `vertices`, `mode` (`replace`, `add`, `toggle`, `clear`) | Escolhe partes da malha. A escolha some quando a malha muda (desfazer, outra operação) |
| `mesh.info` | `entity`?, `geometry`? | Vértices, faces, arestas, triângulos, se é **sólido fechado**, a escolha; com `geometry`, a malha inteira |
| `mesh.inset` | `faces`?, `distance` (0,2), `grouping` (`region`, `individual`) | **Inset**: a face encolhe e ganha um anel; depois, a escolha é a face de dentro |
| `mesh.extrude` | `faces`?, `distance` (0,5; negativo afunda), `grouping` | **Extrudar** faces da malha |
| `mesh.loop_cut` | `edge`?, `cuts` (1 a 32), `slide` (-1 a 1) | **Corte em laço** no anel de quadriláteros da aresta; termina limpo em triângulos e n-gons |
| `mesh.bevel` | `edges`?, `width` (0,1), `segments` (1 = chanfro, até 16 = arredondado) | **Bevel**: tudo ou nada; arestas que se encontram num canto são recusadas juntas (limite do Petunia3D) |
| `mesh.subdivide` | `faces`?, `cuts` (1 a 32) | Subdivide triângulos e quadriláteros; os vizinhos ganham os pontos novos (sem rachaduras) |
| `mesh.dissolve` | `edges`? | Tira arestas e junta as duas faces de cada lado |
| `mesh.merge` · `mesh.merge_by_distance` | `vertices`? / `distance` (0,001) | Funde vértices no centro; funde os que estão perto demais |
| `mesh.delete_faces` · `mesh.flip` | `faces`? | Apaga faces (a malha fica aberta); vira faces do avesso (sem `faces`: todas) |
| `mesh.knife` | `start`, `end`: `{ "edge": [a, b], "fraction": 0.5 }` | **Faca**: corta uma face em duas entre dois pontos das suas arestas |
| `mesh.preview` | `operation` (`inset`, `extrude`, `bevel`, `loop_cut`) + os valores dela (`distance`, `width`, `slide`…) | **Valor ao vivo** enquanto o mouse arrasta: mostra o resultado sem gravar; cada passo parte da malha de antes do arrasto. Um valor que ainda não serve (inset de 0) mostra a malha como estava e responde `valid: false` com o motivo |
| `mesh.preview_end` | `commit` (padrão `true`) | Solta o arrasto: **um** passo de desfazer, ou nada quando `commit: false`. Outra operação de malha também encerra o arrasto (mantendo-o) |
| `floor.step` · `floor.height` | `delta` / `meters` | Sobe ou desce o **andar** (grade e criação); além do último, **cria** o andar (um desfazer) |
| `floor.select` · `floor.above` | `entity` / `mode` (`ghost`, `hide`, `show`) | Vai a um andar da lista; como aparecem os objetos acima dele |
| `viewport.overlay` | `width`, `height` | Modelo em pixels do gizmo, guias, marcadores, painel do gesto e eixos da câmera |
| `viewport.outline_style` | `color` `{r,g,b}` (0 a 1), `width` (1 a 8 px) | Cor e espessura do contorno da seleção (preferência do editor, não é comando) |
| `collider.add` | `shape` (`box`, `sphere`, `capsule`, `convex`) | Colisão com nome no objeto selecionado, ajustada a ele; nome livre ("colisão", "colisão 2"…); um passo de desfazer |
| `collider.fit` | — | A colisão selecionada volta a envolver o objeto |
| `play.start` | — | **Jogar**: roda a cena numa **cópia** com scripts, física e o personagem do jogador (cria um herói de reserva no `PlayerStart` se não houver). Devolve `problems`, `has_player` |
| `play.tick` | `seconds`, `forward`, `right`, `jump`, `look_x`, `look_y` | Um quadro do jogo; devolve o `log` dos scripts, as `faults` e o `debug` do quadro |
| `play.debug` | — | O que o último quadro reportou (`tick`, `marks` com `ran` / `check` / `value`) e `playing`; responde também com o jogo parado (DR-32) |
| `play.stop` | — | Descarta a cópia e devolve a câmera do editor |
| `camera.view_direction` · `camera.yaw` · `camera.orbit` · `camera.projection` | `x`, `y`, `z` / `degrees` / `dx`, `dy` / `mode`, `height` | Usados pelo cubo de navegação e menus de projeção |

## Layout da janela

As regras de tamanho, cor e texto estão em [design da interface](15-design-da-interface.md) ([DR-26](../06-decisoes/registro-de-decisoes.md)).

```text
┌ ≡  Sala de exemplo        [ Cena | Código ]        🔍 Buscar comando  Ctrl K   ↶ ↷ 💾 ┐
├ Hierarquia ─────┬ Global|Local  ⊕Pivô ⊞ │ 🧲0,5 m ⌄          ⌄ Térreo ⌃ ┬ Inspetor ────┤
│ 🔍 Buscar     + │ ┌──┐                                                │ ◇ Caixa  #9  │
│ ▾ Sala          │ │⬚ │  ferramentas                                   │ ⌄ Nome    ⋯  │
│   ◇ Piso     #1 │ │✥ │  (coluna)          viewport 3D                 │ ⌄ Transf. ⋯  │
│   ◇ Caixa    #9 │ └──┘                                                │ + Adicionar  │
│                 ├ Diagnósticos  Histórico ────────────────────────────┤   tabela     │
└─────────────────┴─────────────────────────────────────────────────────┴──────────────┘
```

| Parte | O que tem |
|---|---|
| **Barra de cima** | `≡` com todos os menus (`F10`; `Alt+letra` abre um menu direto); o projeto (`•` = não salvo); os workspaces **Cena** (`F3`) e **Código** (`F4`); **Buscar comando** (`Ctrl+K`); desfazer, refazer, salvar |
| **Faixa da cena** | Eixos Global/Local, pivô (menu), editar pivô, **snap** (o botão mostra o passo; `⌄` abre passos, alvo e inclinação) e o andar |
| **Coluna de ferramentas** | No canto do viewport: transformar · faces · esculpir · primitivas (a lista abre ao lado) |
| **Opções da ferramenta** | Só com Esculpir ou Túnel, sob a faixa; quando falta espaço, o resto vai para `⋯` |
| **Hierarquia** | Árvore com o projeto na raiz, ícone por tipo, número em cinza; busca e `+` (criar) |
| **Inspetor** | Ícone, nome e número; uma seção por tabela, com nome em português, que **recolhe**; `⋯` remove; **Adicionar tabela** no fim |
| **Workspace Código** | Lista de scripts (`+` cria), editor, Problemas e Console, e a **Referência da API** (funções do script primeiro) |

**Paleta de comandos** (`Ctrl+K` ou `Ctrl+Shift+P`): digite parte do nome, em qualquer ordem de letras ("cvr" acha "Caverna"); `↑ ↓` escolhem, `Enter` executa, `Esc` fecha. Os comandos usados por último aparecem primeiro.

**Exibir > Menus por extenso na barra** troca o `≡` pelos nomes dos menus. Cada workspace lembra quais painéis estavam abertos e o tamanho deles.

**Jogar** (`F6`; botão roxo na barra, vira **Parar** em vermelho): `W A S D` ou setas andam, `Espaço` pula, botão direito do mouse gira a câmera, `Esc` para. A cena **não muda**: o jogo roda numa cópia. Enquanto joga, hierarquia, inspetor, ferramentas e a maioria dos comandos ficam desligados, e o motor recusa edições. A saída dos scripts vai para o Console do workspace Código.

**Preferências** (`Ctrl+,`, menu Editar): tema, densidade e o **contorno da seleção** (cor e espessura). O contorno gira com o objeto.

**Colisões** (DR-27): *Adicionar colisão* no inspetor ou em **Operações › Colisão** (e na paleta). Elas aparecem **sob o objeto** na Hierarquia e em **verde** no viewport (gatilhos em amarelo; a selecionada mais clara). Clique numa para editar centro, tamanho, raio, altura, gatilho, atrito e quique; `⋯` › *Ajustar ao objeto*. Duplicar e excluir o objeto levam as colisões junto. O corpo físico é a tabela **Corpo físico** (`RigidBody`): *Adicionar tabela*.

**Não há botão Jogar**: o modo Jogar ainda não existe, e um botão sem efeito engana.

## Controles do viewport

| Ação | Como |
|---|---|
| Andar | setas |
| Voar (com o **botão direito** pressionado) | `W A S D`, `E` sobe, `Q` desce |
| Subir e descer | `PageUp` e `PageDown` |
| Mais rápido | segure `Shift` |
| Girar a câmera | botão direito do mouse + arrastar |
| Aproximar e afastar | roda do mouse |
| Selecionar | clique esquerdo |
| Selecionar e transformar (gizmo universal) | `V` |
| Mover, Girar, Escalar | `G`, `R`, `S` (mapa padrão do Petunia3D; ou a coluna de ferramentas no viewport) |
| Menu radial de primitivas | `Shift+A` (aponte e clique, ou números 1–8) |
| Modo Livre (o mouse muda o valor; `X` `Y` `Z` restringem; digite um número) | **duplo toque** em `G`, `R` ou `S`; `Enter`/clique confirma, `Esc`/botão direito cancela |
| Empurrar/puxar e Extrudar uma face | `P` e `E`, depois arraste a face |
| Cavar e Tornar oco | `Ctrl+Alt+C` e `Ctrl+Alt+H` |
| Esculpir caverna (modo nas opções da ferramenta, sob a faixa do viewport; `Ctrl` faz o oposto) | `B`; `[` e `]` mudam o raio; `Esc` cancela o arrasto |
| Workspace Código | `F4` (e `F3` volta à cena); `F5` executa numa cópia; `Ctrl+S` dentro do editor salva o **script**; `Ctrl+Shift+F` formata; `Ctrl+Espaço` sugestões; **`Esc` e depois `Tab` sai do editor** |
| Túnel por caminho: clique pontos na rocha | `Shift+B`; `Enter` cava, `Backspace` tira o último ponto, `Esc` cancela |
| Editar o pivô do objeto | `Alt+P` |
| Subir e descer de andar (acima do último, cria um novo) | `Ctrl+PageUp` e `Ctrl+PageDown`, ou a lista de andares na barra |
| Objetos acima do andar: fantasma, ocultar ou mostrar | **Exibir > Objetos acima do andar** |
| Arrastar um gizmo | seta = eixo, quadrado colorido = plano, centro = plano da tela (mover) / uniforme (escalar) / livre (girar), anel externo = em volta da vista; `Esc` cancela |
| Inverter o snap durante o arrasto | segure `Ctrl` |
| Focar na seleção | `F` |
| Vistas | **cubo de navegação** (canto superior direito) ou `Alt+1` a `Alt+6` |
| Câmera no início | `Home` |

Os atalhos de edição ficam em **Ajuda > Atalhos**: `Ctrl+Z`, `Ctrl+Shift+Z`, `Ctrl+D`, `Delete`, `Esc`, `Ctrl+1/2/3`, `Ctrl+S`.

## Acessibilidade que já existe no código

**Aviso:** isto está escrito, mas **ainda não foi testado** com leitor de tela, teclado sem mouse ou alto contraste reais.

- Tudo tem **atalho de teclado** e fica nos menus. Nada depende só do mouse.
- Os campos têm **nome acessível** definido (por exemplo "Transform · position eixo X"), para leitores de tela.
- **Texto maior e menor** (`Ctrl+=`, `Ctrl+-`, `Ctrl+0`) e **alto contraste**, ambos lembrados entre sessões.
- Os erros dizem **o que aconteceu e o que fazer**, em português.
- O editor **pergunta antes de descartar** alterações não salvas.

**Ainda falta** (veja [acessibilidade do editor](../04-acessibilidade/01-editor.md)): teste com leitor de tela real, modo sem animações, movimento da câmera sem mouse além do teclado, foco visível reforçado e catálogo de idiomas.

## Limites conhecidos (para não esconder)

| Limite | Efeito |
|---|---|
| A lista de objetos vem inteira a cada mudança (corte em 10 mil) | Cenas enormes ficam lentas; falta paginação |
| Traços de caverna não aparecem na Hierarquia nem se editam um a um | A caverna inteira se move, copia e apaga; editar um traço fica para depois |
| Abrir uma caverna enorme e desfazer nela recalculam o campo todo | Pausa de centenas de milissegundos com mais de mil traços; ver [cavernas](14-workspaces-e-prioridades.md) |
| **Cavar** só funciona em caixas sem rotação | Recusa com mensagem; para outras formas e rotações, use `mesh.boolean` |
| Edição de malha ainda **não tem botões nem atalhos** no Qt | As operações `mesh.*` existem e têm teste; os ganchos da interface estão listados em [edição de malha](13-primitivas-e-edicao-de-malha.md#ganchos-da-interface-que-faltam) |
| Bevel recusa arestas que se encontram num canto | Limite herdado do Petunia3D: escolha arestas separadas, uma de cada vez |
| Malhas editáveis **não têm UV** nem material por face | Cor única por objeto; UV ficou fora da cópia (DR-22) |
| Tempo das operações no build do editor (dev, com `opt-level = 3` em `petunia_mesh` e `manifold-rust`) | Malha de 6,5 mil vértices: booleano 74 ms, inset em todas as faces 29 ms, subdividir 5 a 30 ms, checar sólido 3 ms (medido em 2026-10-25). Malhas maiores ainda não foram medidas |
| Testar "acima do andar" monta a malha de cada objeto a cada quadro | Bom para centenas de objetos; cenas grandes pedem caixas guardadas |
| Inteiros no inspetor vão só até 32 bits | O motor aceita até 2^53 |
| Desfazer não restaura a seleção | A entidade recriada precisa ser selecionada de novo |
| Sem salvamento automático | Falta; é um recurso de produto planejado |
| Picking testa todos os triângulos | Bom para centenas de objetos; precisa de estrutura espacial depois |
| Só a cena padrão do projeto abre | Falta troca de cenas |

## OpenGL e plataformas gráficas

O editor pede OpenGL 3.3. Como os drivers aceitam formatos diferentes, ele **testa do melhor ao mais simples** (3.3 core com antisserrilhado 4x, depois sem antisserrilhado, depois sem profundidade na janela, depois compatibilidade) e usa o primeiro que o driver cria.

| Situação | O que o editor faz |
|---|---|
| Wayland nativo recusa tudo (visto com NVIDIA + Qt 6.12) | Reinicia sozinho via X11 (XWayland) |
| Nenhum formato funciona | Mostra uma mensagem e sai, sem travar |

`petunia-editor --gl-info` mostra o resultado sem abrir janela.

## Segurança

- **Nenhuma execução arbitrária.** A janela só envia as operações da tabela acima.
- **JSON inválido ou inesperado vira erro com código**, nunca falha da janela. Um pânico no Rust também é capturado na fronteira.
- Os caminhos dentro do projeto continuam presos à pasta do projeto (`resolve_inside`).

## Como testar

| O quê | Comando | Observação |
|---|---|---|
| Lógica do editor | `cargo test -p petunia_editor -- --test-threads=2` | Sem janela |
| Janela de ponta a ponta | `scripts/editor-smoke.sh` | **Display virtual (Xvfb)**: nada aparece na sua tela e suas preferências ficam intactas |

O teste de fumaça clica, duplica, cria, anda, gira, salva e tira uma imagem. Ele diz **se roda e desenha**. Não é regressão visual pixel a pixel (isso é o `cargo xtask test-visual`, *planejado*).
