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
| `project.save` | — | Grava a cena (atômico) |
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
| `script.list` · `script.read` · `script.new` | — / `name` | Scripts da pasta `scripts/`; criar a partir do modelo (nunca sobrescreve) |
| `script.write` | `name`, `text` | Grava; a versão anterior vai para `scripts/.backup/` |
| `script.check` · `script.format` | `name`, `text` | Erros com linha e coluna; formatar (só texto que passa na sintaxe) |
| `script.run` | `name`, `text`?, `ticks`? | Executa numa **cópia** do mundo; devolve console, falhas e quantos objetos mudariam |
| `script.api` · `script.attach` | — / `name` | API para sugestões e ajuda; liga o script ao objeto selecionado (um desfazer) |
| `create.cave` | `seed` (opcional) | Bloco de rocha com um túnel; com `seed`, com salas e túneis **gerados** (um desfazer) |
| `sculpt.begin` · `sculpt.drag` · `sculpt.end` · `sculpt.cancel` | pixels, `invert` | Esculpe a caverna sob o mouse; o arrasto vira **um** `world.batch` |
| `sculpt.mode` | `mode` (`carve`, `add`, `inflate`, `flatten`, `smooth`), `force` (0 a 1) | Modo e força do pincel |
| `sculpt.radius` | `meters` (0,25 a 16) | Raio do pincel |
| `path.point` · `path.pop` · `path.commit` · `path.cancel` | pixels, `invert` | Túnel por caminho: pontos na rocha, `Enter` cava (um desfazer) |
| `sculpt.generate` | `seed`, `rooms`, `worms`, `radius` | Soma salas e túneis gerados à caverna selecionada |
| `sculpt.consolidate` | — | Funde traços em cadeia (um desfazer) |
| `sculpt.faceted` | `value` | Triângulos chapados (low poly) na caverna selecionada |
| `brush.carve` · `brush.hollow` | — / `thickness` | **Cavar** (a seleção corta as caixas que toca) e **Tornar oco** |
| `floor.step` · `floor.height` | `delta` / `meters` | Sobe ou desce o **andar** (grade e criação); além do último, **cria** o andar (um desfazer) |
| `floor.select` · `floor.above` | `entity` / `mode` (`ghost`, `hide`, `show`) | Vai a um andar da lista; como aparecem os objetos acima dele |
| `viewport.overlay` | `width`, `height` | Modelo em pixels do gizmo, guias, marcadores, painel do gesto e eixos da câmera |
| `camera.view_direction` · `camera.yaw` | `x`, `y`, `z` / `degrees` | Usados pelo cubo de navegação |

## Layout da janela

Inspirado no redesenho do Unity que você mandou como referência visual ([DR-26](../06-decisoes/registro-de-decisoes.md)). Nada de código foi copiado.

```text
┌ Arquivo Editar Criar … │ ferramentas │ Global|Local  Pivô ⊞ ……………… ↶ ↷ │ 💾 ┐
├ Hierarquia ─────┬ Snap 0,5 m 15° 0,1× Inteligente …  ⌄ Térreo ⌃ ┬ Inspetor ────┤
│ + Criar  Buscar │ (opções da ferramenta de esculpir, só com B)  │ ◇ Caixa  #9  │
│ ▾ Sala          │                                               │ ⌄ Name    ⋯  │
│   ◇ Piso     #1 │              viewport 3D                      │ ⌄ Transform ⋯│
│   ◇ Caixa    #9 │                                               │ + Adicionar  │
│                 ├ Diagnósticos  Histórico  API  Código ─────────┤   tabela     │
│                 │ painel da aba                                 │              │
└─────────────────┴───────────────────────────────────────────────┴──────────────┘
```

| Parte | O que tem |
|---|---|
| **Barra de cima** (uma só) | Menus; as 8 ferramentas em ícones (transformar · faces · esculpir) e o menu de primitivas; eixos **Global/Local**, pivô e editar pivô; desfazer, refazer e salvar |
| **Faixa do viewport** | Snap, os três passos (as unidades m, ° e × dizem qual é qual), alvo do snap, inclinação e o andar |
| **Opções da ferramenta** | Só aparece com Esculpir ou Túnel; quando falta espaço, o resto vai para o botão `⋯` |
| **Hierarquia** | Árvore com o projeto na raiz, ícone por tipo de objeto, número em cinza; **Criar** ao lado da busca |
| **Inspetor** | Cabeçalho com ícone, nome e número; uma seção por tabela, que **recolhe** (o estado vale para os próximos objetos); o `⋯` da seção tem *Remover tabela*; **Adicionar tabela** no fim |
| **Painéis de baixo** | Abas em cima (Diagnósticos, Histórico, API, Código), só sob o viewport; os painéis laterais vão até embaixo |

Os painéis laterais têm um título compacto com **soltar** e **fechar**; **Exibir** reabre qualquer painel. Os temas **claro** (padrão), escuro e alto contraste usam o mesmo layout.

**Não há botão Jogar** na barra: o modo Jogar ainda não existe, e um botão sem efeito engana.

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
| Mover, Girar, Escalar | `G`, `R`, `S` (mapa padrão do Petunia3D; ou os ícones da barra de cima) |
| Menu radial de primitivas | `Shift+A` (aponte e clique, ou números 1–8) |
| Modo Livre (o mouse muda o valor; `X` `Y` `Z` restringem; digite um número) | **duplo toque** em `G`, `R` ou `S`; `Enter`/clique confirma, `Esc`/botão direito cancela |
| Empurrar/puxar e Extrudar uma face | `P` e `E`, depois arraste a face |
| Cavar e Tornar oco | `Ctrl+Alt+C` e `Ctrl+Alt+H` |
| Esculpir caverna (modo nas opções da ferramenta, sob a faixa do viewport; `Ctrl` faz o oposto) | `B`; `[` e `]` mudam o raio; `Esc` cancela o arrasto |
| Editor de código | `F4`; `F5` executa numa cópia; `Ctrl+S` dentro do editor salva o **script**; `Ctrl+Shift+F` formata; `Ctrl+Espaço` sugestões; **`Esc` e depois `Tab` sai do editor** |
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
| Sem câmera ortográfica | Construir em planta exige a vista de topo em perspectiva |
| Traços de caverna não aparecem na Hierarquia nem se editam um a um | A caverna inteira se move, copia e apaga; editar um traço fica para depois |
| Abrir uma caverna enorme e desfazer nela recalculam o campo todo | Pausa de centenas de milissegundos com mais de mil traços; ver [cavernas](14-workspaces-e-prioridades.md) |
| **Cavar** só funciona em caixas sem rotação | Recusa com mensagem; rampas e cilindros não são cortados |
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
