# Plano: mapas, materiais e interface

**Revisão:** 2026-11-09 · **Estado:** decisão de corte ([DR-53](../06-decisoes/registro-de-decisoes.md)). A frente 4 está provada no teste. O resto deste plano ainda não virou código novo. O que já existia antes está marcado `LOCAL`.

## Resumo (TL;DR)

O pedido de 2026-11-09 pede treze frentes. Este documento é a ordem e o corte de cada uma. A edição volta **sem um modo novo**: a seleção de face, aresta e vértice mora nas ferramentas que já estão na barra. O loop cut volta só na malha gravada. O booleano que já corta passa a abrir vão andável. Materiais são cinco tipos de dado, sem grafo de nós. O clima vira entidade e pode haver vários ao mesmo tempo. A interface de jogo **não entra neste corte**: a seção 13 é conversa.

Cada frente fecha com comando, teste e uma linha neste arquivo passando de "a fazer" para "feito", com o resultado do teste. Sem isso, o item continua plano.

## O que muda em relação ao que já foi decidido

| Decisão anterior | O que este plano faz |
|---|---|
| [DR-51](../06-decisoes/registro-de-decisoes.md) apagou modo Tab, faca, inset, bevel, loop cut, dissolver e fundir vértices | Continua apagado, menos três ferramentas de mapa: seleção de componente, loop cut com opções, e mover/girar/escalar o componente. Sem `Tab` e sem barra de modelagem |
| [DR-52](../06-decisoes/registro-de-decisoes.md) pinta uma face de caixa com um quadrado 2 mm para fora | A cor por face passa a ser um material `flat` na face, quando a frente 5 chegar. Até lá o quadrado fica |
| [DR-23](../06-decisoes/registro-de-decisoes.md) deixa pintura de textura fora | Continua fora. Textura aqui é arquivo em `assets/` ligado a um material, não pincel em UV |
| [DR-45](../06-decisoes/registro-de-decisoes.md) deixou clima único fora da cena | A frente 6 grava o clima na cena, como entidade |
| [DR-17](../06-decisoes/registro-de-decisoes.md) ainda é proposta de UI | A frente 13 não escolhe implementação |

Regras que não mudam: comando é dado e desfaz; o domínio não depende de Qt nem de OpenGL; produção não usa `unwrap`; biblioteca nova só com o checklist de [bibliotecas](../02-arquitetura/10-bibliotecas-auxiliares.md). Nenhuma frente deste plano adiciona dependência.

## Ordem

A numeração é a do pedido. A ordem de código respeita dependência.

| Corte | Frentes | Por quê nesta hora |
|---|---|---|
| A | 4, depois 1 e 3, depois 2 | O vão da janela é booleano. A seleção serve o booleano e o loop cut |
| B | 5 | A estrada e a face pintada passam a apontar para um material |
| C | 6 e 7 | O céu lê o clima misturado |
| D | 8 | A estrada ganha material e junção |
| E | 9 | Não depende do 3D |
| F | 10 | A parede com vão usa o booleano do corte A |
| G | 11 | O editor de código não trava o mapa |
| H | 12 | Não depende de nada; pode subir na fila se a tela vazia atrapalhar |
| I | 13 | Só conversa. Código depois de uma decisão nova |

## 1. Seleção de face, aresta e vértice, sem modo novo

**Hoje (`LOCAL`).** Não há modo de edição. Empurrar (`P`) e extrudar (`E`) acertam a face sob o mouse. A caixa tem alças de canto e de aresta. O clique escolhe o objeto.

**Corte.** Com a ferramenta Mover, Girar ou Escalar ativa, o clique escolhe o componente sob o mouse **do objeto já selecionado**. Um segundo clique no vazio volta a escolher objetos. Não existe tecla de modo, nem botões Vértice/Aresta/Face. A barra de status diz o que está escolhido ("face +X", "aresta 3", "vértice 2"). `1` `2` `3` trocam o tipo de componente sob o mouse, como atalho, sem mudar de modo.

A seleção é dado da sessão (não entra no arquivo da cena): entidade, tipo, índices. Vários componentes com `Shift`. `Esc` limpa a seleção de componente e deixa o objeto selecionado.

**Não entra.** Faca, inset, bevel, dissolver, fundir vértices, subdividir, prévia ao vivo dessas operações.

## 2. Loop cut com opções

**Hoje (`LOCAL`).** O código de loop cut foi apagado com o DR-51. O booleano ainda produz uma malha gravada (`Mesh`).

**Corte.** `mesh.loop_cut` volta para **malha gravada**, não para a caixa paramétrica. Caixa continua sendo brush: para cortar em laço, o booleano ou `mesh.convert` grava a malha antes. Opções no diálogo e no comando:

| Campo | Limite |
|---|---|
| `cuts` | 1 a 32 |
| `slide` | −1 a 1, a partir do meio |
| `smooth` | 0 a 1, só encosta nos vizinhos já existentes |

Um desfazer. `Esc` cancela o diálogo. Atalho `Ctrl+R` abre o diálogo; não há arrasto ao vivo neste corte (o valor ao vivo antigo dependia do modo que saiu).

O teste que volta com a função: uma caixa convertida, um corte, a malha continua sólida, um desfazer volta à malha anterior.

## 3. Mover, girar e escalar por face, aresta e vértice

**Hoje (`LOCAL`).** O gizmo move o objeto inteiro. A caixa redimensiona por canto e aresta, com o lado oposto parado.

**Corte.** Com um componente escolhido (frente 1), o mesmo gizmo age nele:

| Componente | Mover | Girar | Escalar |
|---|---|---|---|
| Face de caixa | empurra aquela face; a oposta fica | gira a caixa em torno do centro da face | escala a caixa a partir dessa face |
| Aresta de caixa | move a aresta no plano da ferramenta; a caixa continua convexa | gira em torno da aresta | afasta a aresta do centro |
| Vértice de caixa | move o vértice; se deixar de ser caixa, a operação **grava uma malha** e avisa na barra | gira os vértices escolhidos em torno do centro deles | escala a partir desse centro |

Malha gravada (depois do booleano ou de um GLB): mover, girar e escalar os vértices escolhidos, ou a face inteira, ou a aresta. Um gesto, um desfazer. Snap segue o da cena. `Esc` devolve o gesto.

Isso é o encaixe de mapa no estilo Quake: o bloco é a unidade, o vértice só sai do bloco quando o corte pede. Não é um modelador.

**Não entra.** Extrusão de malha, ponte entre buracos, slide de aresta além do loop cut.

## 4. Booleano que abre janela e portal

**Hoje (`LOCAL`).** `mesh.boolean` com `union`, `subtract` e `intersect` usa o núcleo Manifold. O menu é **Malha ▸ Booleano…** e já pede operação, ferramenta e "manter ferramenta". O import de GLB e a colisão de malha existem.

**Feito (2026-11-09).** O comando não mudou. Dois testes do `petunia_editor` provam o vão: `booleans_cut_a_door_and_one_undo_restores_both_brushes` e `a_rotated_cutter_still_opens_a_hole_and_a_miss_changes_nothing`. Os dois passaram.

O que o corte pedia:

1. Parede caixa menos caixa de vão: a malha tem o buraco, o volume cai o volume do vão, e o sólido da física acompanha o buraco (dá para atravessar).
2. O mesmo com a ferramenta girada.
3. `keep_tool: true` deixa o cortador no lugar, para repetir a janela ao lado.
4. O diálogo pede operação, objeto ferramenta e "manter ferramenta". A seleção é o alvo.
5. Falha (malha aberta, vão que não toca) explica o motivo e não muda a cena.

Portal, neste corte, é o vão: um volume que a física não bloqueia. Não é teleporte. Teleporte fica para um script em cima de `Interact`, fora daqui.

## 5. Materiais, sem grafo de nós

**Julgamento:** grafo de nós não é necessário. Cinco tipos cobrem o pedido, com campos no inspetor. Um nó a mais seria outro editor para a mesma tabela. Bibliotecas de shader graph ficam de fora. PNG já entra por `petunia_import`. Nenhuma dependência nova.

**Hoje (`LOCAL`).** Uma cor por objeto. `FaceColor` cobre uma face de caixa com um quadrado deslocado 2 mm. O GLB guarda a cor base e **avisa** que a textura foi ignorada. O sprite já amostra PNG com alfa. O sombreador 3D tem um `u_color` só.

**Corte.** Tabela `Material` no objeto (e, quando houver, na face via `FaceColor.material` no lugar da cor solta). Tipos:

| Tipo | O que desenha | Campos |
|---|---|---|
| `flat` | cor sem brilho de luz | `color`, `alpha` |
| `standard` | cor, metal e rugosidade; texturas opcionais | `color`, `metallic`, `roughness`, `base_texture`, `normal_texture` |
| `masked` | o mesmo que `standard`, pixel com alfa abaixo do corte some | `cutoff` de 0 a 1 |
| `blended` | o mesmo, com mistura; desenha depois dos opacos | `alpha` |
| `post` | um retângulo de tela inteira, um efeito nomeado | `effect` (`grade`, `fog`), intensidades já existentes no passe de cor |

Textura é caminho dentro de `assets/`, PNG, com o mesmo limite de tamanho do sprite. Sem o arquivo, vale a cor e a barra explica. O objeto sem `Material` continua com a cor de hoje.

`post` não é um shader livre. A lista de efeitos é a que o renderizador já tem (cor, vinheta, névoa). Efeito novo é uma decisão nova, não um campo de texto com GLSL.

**Não entra.** Pintura em UV, camadas de tinta, decal (o [DR-12](../06-decisoes/registro-de-decisoes.md) segue proposta), material toon (prova do Digi-Tactics, outro corte).

Quando `standard` desenhar a textura do GLB, o aviso "texturas ignoradas" sai desse arquivo.

## 6. Clima como entidade, mais de um ao mesmo tempo

**Hoje (`LOCAL`).** `Weather` é um valor da sessão: um preset (`clear`, `cloudy`, `rain`, `fog`, `snow`) e uma hora. `weather.set` não é desfazer. Não está na cena. O céu achata o flash quando a pessoa pediu menos movimento.

**Corte.** Tabela `Weather` em uma entidade:

| Coluna | Papel |
|---|---|
| `preset` | os cinco de hoje |
| `hours` | 0 a 24, finito |
| `weight` | 0 a 1, quanto entra na mistura |
| `volume` | vazio = o clima vale no mapa inteiro; caixa = só dentro |

Várias entidades misturam num estado só, por peso. Dentro de um volume, o volume ganha do global. O estado misturado é o que o sol, a névoa e a chuva já leem. Mudar peso ou preset é um desfazer. Hora pode ser animada depois pela trilha de propriedade; neste corte um campo no inspetor basta.

O limitador de flash continua no estado misturado, não em cada preset.

**Não entra.** Relâmpago, trovão, poça, neve acumulada no chão, áudio de chuva.

## 7. Skybox em camadas, com animação

**Hoje (`LOCAL`).** O céu é a cor do clima. Não há cubemap.

**Corte.** Tabela `SkyLayer`, várias por cena, cada uma numa entidade:

| Coluna | Papel |
|---|---|
| `order` | quem desenha por cima |
| `color` | cor lisa, se não houver imagem |
| `image` | PNG equiretangular em `assets/`, opcional |
| `alpha` | 0 a 1 |
| `scroll` | volta por segundo em x e em y; 0 fica parado |
| `hours_follow` | se verdadeiro, a cor acompanha a hora do clima misturado |

A rolagem usa o relógio do jogo (`anim.advance` / passo do Jogar), não o relógio da máquina. Em edição as camadas ficam no tempo zero, para a cena não mudar sozinha ao salvar.

Seis imagens soltas (um cubo) ficam para um corte posterior. Uma imagem equiretangular cobre o pedido de camadas.

## 8. Rios e estradas

**Hoje (`LOCAL`).** Rio e estrada são spline, largura, acostamento, defensas, ponte detectada, água com fluxo. O leito é chão. Decal de faixa, LOD em quadtree, tubo de bueiro e partículas de queda **não existem** e não entram aqui (decal continua no DR-12).

**Corte.**

1. `path.join`: duas pontas próximas viram um nó só (encruzilhada ou confluência). Um desfazer.
2. Alça de largura no viewport, no ponto da spline sob o mouse, um desfazer ao soltar.
3. Coluna `material` apontando para a frente 5. Sem material, a cor de hoje.
4. Rio que sobe continua recusado, como já é.

## 9. Plano de fundo 2D com rolagem infinita

**Hoje (`LOCAL`).** `Sprite` é uma entidade com imagem, tamanho, cor e camada. `SpriteAnimation.fps` é da folha, não do projeto. Não há rolagem infinita.

**Corte.** Tabela `ParallaxLayer` numa entidade com `Sprite` (ou só a imagem, se não houver sprite de jogo):

| Coluna | Papel |
|---|---|
| `speed` | tiles por segundo, x e y |
| `offset` | deslocamento inicial, em fração da imagem |
| `position` | onde o plano senta na vista, em fração da tela |
| `alpha` | 0 a 1, para sobrepor camadas |
| `order` | ordem do parallax; a camada do sprite continua valendo na frente |

A imagem repete. O retângulo da tela amostra o PNG com a origem andando `speed * segundos + offset`. Edição mostra o quadro parado. Jogar e `petunia run` rolam. Duas camadas com `alpha` e `speed` diferentes são o parallax.

Isto não é o pano 2.5D do Digi-Tactics (câmera calibrada em arte pintada). É fundo de tela.

## 10. Primitivas novas

**Hoje (`LOCAL`).** Caixa, rampa, cilindro, cone, esfera, escada. A escada usa o tamanho da caixa e `detail` como número de degraus; a pisada e o espelho saem dividindo esse tamanho. Não há espiral nem parede com vão.

**Corte.**

| Primitiva | Campos | Como nasce |
|---|---|---|
| Escada reta | `steps` (1 a 64), `tread` (profundidade do degrau, m), `riser` (altura do degrau, m), `width` | Continua brush `stairs`. Se `tread` e `riser` vierem, o tamanho do brush segue a conta. Se não vierem, vale o tamanho atual |
| Escada em espiral | `steps`, `turns`, `radius`, `width`, `height`, sentido | Brush novo `spiral_stairs`. Um sólido só, degraus em volta do eixo |
| Parede com vãos | comprimento, altura, espessura, lista de vãos `{kind: door\|window, x, width, height, sill}` | Gera a parede e subtrai cada vão com o booleano da frente 4, **um** desfazer para a parede inteira |
| Arco | largura, altura, espessura, raio do vão | O mesmo gerador: caixa menos a forma do vão |

**Não entra neste corte.** Cerca (o arranjo em linha já repete um poste), telhado, calha, escada em L. Entram se a parede e a espiral ficarem boas e o mapa pedir.

Janela e porta aqui são **vão na geometria**, com ombreira feita pela sobra da parede. Não é um objeto "janela" animada. A folha da porta continua sendo outra caixa com script.

## 11. Módulo de código

**Hoje (`LOCAL`).** O painel abre os `.aipo` do projeto, confere ao digitar, formata, salva com cópia, e tem console. O painel de API lista operações. Não há inserção no cursor, nem a lista de quem usa o script, nem busca de entidades.

**Corte.**

1. Ao abrir um script, a barra mostra as entidades cuja linha `Script` aponta para aquele arquivo (nome e id). Clique seleciona a entidade na cena.
2. Duplo clique numa operação da API insere o exemplo completo daquela operação na linha do cursor. O exemplo é o que o catálogo já guarda. Se a linha tiver texto, a inserção vai na linha de baixo e o cursor desce.
3. Aba **Entidades** ao lado da API: busca por nome e por id. Duplo clique insere, na linha do cursor, o id persistente na forma que o script já aceita (`entity` com esse número). O texto exato sai do catálogo na hora de implementar, não de um formato novo.
4. **Quadros por segundo do jogo** saem do script. Coluna do manifesto do projeto: `frames_per_second`, padrão 60, finito, de 15 a 240. O passo do Jogar e do `petunia run` lê esse valor. A pessoa jogando poderá baixar o número nas configurações do jogo (arquivo de configuração do usuário, não da cena). Isso fica desenhado aqui e entra quando existir tela de configuração; até lá o manifesto manda.
5. `SpriteAnimation.fps` **continua na folha**. É a velocidade da arte, não a do jogo.

**Também neste corte, porque o painel já tem o gancho.** Duplo clique num erro do console põe o cursor na linha. Busca no script aberto (`Ctrl+F`) se o Qt do painel ainda não tiver.

**Não entra.** Depurador passo a passo, vários cursores, LSP externo.

## 12. Tela de boas-vindas

**Hoje (`LOCAL`).** O editor abre direto na última sessão ou vazio. Não há lista de projetos recentes na partida.

**Corte.** Janela antes do editor, ou a área central enquanto nenhum projeto está aberto:

- nome da engine;
- projetos recentes (caminho, data), gravados na configuração do editor, no máximo 10;
- **Abrir projeto…** e **Novo projeto…**, os mesmos comandos de hoje;
- um retângulo vazio com o texto "QR e chave PIX" e outro com "Ko-fi". Sem imagem, sem rede, sem pagamento. Quando houver arte, ela entra em `ATTRIBUTION.md` e ocupa esses dois retângulos.

Fechar a boas-vindas sem escolher não cria projeto.

## 13. UI de jogo — conversa, sem código

Isto **não está aceito para implementar**. O [DR-17](../06-decisoes/registro-de-decisoes.md) continua proposta. O que segue é a recomendação para a conversa.

A UI de jogo deve ser **dado de layout**, no mesmo espírito de uma pilha e de uma grade, mais curta que CSS:

- Sem seletor, sem cascata, sem herança de estilo. O pai posiciona os filhos.
- Dois pais chegam: `stack` (em linha ou em coluna, com `gap`, `padding` e peso de crescimento) e `grid` (colunas e linhas com tamanho fixo ou `1fr`, criança nomeando a célula).
- Filho é texto, imagem, botão, barra ou painel. Evento de botão vira comando, como o resto da engine.
- Tema são os tokens que o editor já usa (cor, tamanho de texto, foco). Contraste mínimo 4,5:1.
- Teclado alcança todo botão. Arrastar tem caminho por teclado.
- A árvore é a árvore de acessibilidade. Não é HTML e não é um grafo de nós.

O que eu não recomendo: embutir um navegador, interpretar CSS de verdade, ou um editor visual de nós para a UI. Os três custam mais do que a grade e a pilha, e a acessibilidade fica atrás.

Perguntas para fechar antes de qualquer código de UI:

1. A UI vive na cena (entidades) ou num arquivo `ui/*.json` do projeto?
2. O primeiro jogo que precisa dela é HUD (vida, texto) ou janela (inventário, arrastar)?
3. Escala com a janela, com um controle do jogador, ou com os dois?

## Fora deste plano

Faca, inset, bevel, pintura de UV, grafo de materiais, teleporte, decal, relâmpago, cubemap de seis faces, cerca, depurador, e a implementação da UI de jogo.

## Decisões ligadas

[DR-51](../06-decisoes/registro-de-decisoes.md), [DR-52](../06-decisoes/registro-de-decisoes.md), [DR-53](../06-decisoes/registro-de-decisoes.md), [DR-12](../06-decisoes/registro-de-decisoes.md), [DR-17](../06-decisoes/registro-de-decisoes.md), [DR-23](../06-decisoes/registro-de-decisoes.md).
