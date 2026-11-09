# Paridade da interface com o Petunia3D

**Revisão:** 2026-10-24 · **Fonte:** `QT_REFACTOR_BACKLOG.md`, `PROJECT_STATE.md` e o código de `crates/ui-cxxqt` do Petunia3D, commit `dec467127a7f` (`LOCAL`). Atribuições em [ATTRIBUTION.md](../../ATTRIBUTION.md).

## Resumo (TL;DR)

**Atualização (2026-10-25):** o trilho à esquerda saiu; as ferramentas e as primitivas estão numa coluna dentro do viewport. Vieram do Petunia3D também o controle segmentado e a ideia da paleta de comandos ([design](15-design-da-interface.md)) ([DR-26](../06-decisoes/registro-de-decisoes.md), [layout](11-editor.md#layout-da-janela)). As linhas abaixo que falam do trilho valem para essa barra.

Da lista de decisões de interface do Petunia3D (`QT-00` a `QT-16`, 20 linhas), esta etapa trouxe a **base visual e de entrada**: tema, ícones, campos numéricos e XYZ, trilho de ferramentas, gizmo, snap e vistas. Das **20 linhas** da lista: **10 feitas, 7 parciais, 0 planejadas e 3 fora de escopo** (recontado em 2026-10-31).

**Por que não há "1 para 1":** três linhas são **fora de escopo por decisão registrada** — QT-03
(cursor 3D, retirado a seu pedido), QT-04 (Poly Pen) e QT-10 (operador e perfil de desenho), os dois
últimos de **modelagem de malha**, que esta engine faz pelo workspace de malha e não pelo outliner.
"1 para 1" com o Petunia3D incluiria copiar o editor de modelagem inteiro, o que o `DR-22` recusou.
As parciais restantes estão listadas com o que falta, uma por uma.

O que **não** vem é o que só existe por causa da modelagem de malha (Poly Pen, perfis de desenho, loop cut). Isso fica para a [edição de malha](13-primitivas-e-edicao-de-malha.md), se você aprovar.

## Como a paridade foi feita

| Escolha | Motivo |
|---|---|
| **Copiar** o que não depende da modelagem (tema, ícones, campos) | É a parte mais cara de acertar e já está pensada para acessibilidade |
| **Reescrever pequeno** o que dependia da ponte do Petunia3D (trilho, barra do viewport) | O trilho de lá usa um controlador de shell que não existe aqui |
| **Não copiar** o que é de malha ou de pintura | Fora do escopo da engine nesta fase |

Os módulos copiados estão em `editor/src/ui_kit/` e **sem alteração**. Quando o Petunia3D passar a consumir uma biblioteca compartilhada ([DR-18](../06-decisoes/registro-de-decisoes.md)), eles saem daqui.

## Matriz

Estados: **Feito** (rodou e foi visto), **Parcial**, **Planejado** (viável, ainda não feito), **Fora** (modelagem ou pintura).

| Item do Petunia3D | Estado | Na engine |
|---|---|---|
| QT-00 · Transforms em linha única, tags de eixo coloridas | **Feito** | `PetuniaVector3Field`: X vermelho, Y verde, Z azul, **sempre com a letra** (a cor não é o único sinal) |
| QT-00 · Botões sem caixas duras | **Parcial** | Tema do Petunia3D aplicado; os botões da barra ainda têm borda fina (**decisão de gosto, não trabalho pendente**) |
| QT-00 · Barra inferior contextual dinâmica | **Planejado** | Depende de ter operações por contexto (face, aresta) |
| QT-00 · Densidade compacta do Outliner | **Feito** | Densidade **Compacta por padrão** (campos e ícones menores: 16 px na barra, 18 px no trilho); Padrão e Confortável no menu |
| QT-00 · Barra superior unificada (menus, abas, busca) | **Planejado** | Hoje: menu + barra de ações |
| QT-01 · Trilho esquerdo enxuto | **Feito** | Selecionar, Mover, Girar, Escalar e Primitivas |
| QT-02 · Gizmo moderno (eixos RGB, setas sólidas) | **Feito** | **Porte** de `compute_gizmo`: um gizmo por ferramenta (seta no mover, quadrado no escalar, anéis com o arco da frente destacado no girar), planos YZ/XZ/XY, anel da vista, centro, e o **universal** com zona morta. Melhorado: o anel mais próximo vence (no original, o anel da vista roubava metade do anel de eixo) |
| QT-03 · Cursor 3D | **Fora** | Retirado a seu pedido: o **pivô por objeto**, editável e sempre visível, cobre o caso |
| QT-04 · Poly Pen em vistas ortográficas | **Fora** | Modelagem de malha |
| QT-05, QT-06 · Pivô e cursor no inspetor | **Feito** | Pivô **por objeto** (`Transform.pivot`, no inspetor), modo **Editar pivô** (`Alt+P`), marcador sempre visível; políticas centro da caixa e base |
| QT-07 · Docas e janelas flutuantes | **Parcial** | Docas do Qt (arrastar, abas, flutuar) com o arranjo guardado **por workspace** em `QSettings`. Falta um layout nomeado, versionado em arquivo |
| QT-08 · Primitivas num menu do trilho | **Feito** | Caixa, rampa, cilindro, cone, esfera e escada |
| QT-09 · Sombreamento na barra do viewport | **Feito** (2026-10-31) | Snap, eixos e pivô na barra; vistas no cubo; **Sólido · Aramado · Raio-X** no menu **Exibir › Sombreamento** (`view.shading`), como modo de visão: muda o quadro, nunca a cena nem o desfazer |
| QT-10 · Operador e perfil de desenho | **Fora** | Modelagem |
| QT-11 · Cena vazia | **Parcial** | O mundo aceita zero objetos; não testei apagar tudo na interface |
| QT-12 · Outliner: ocultar, travar, solo, cor, multisseleção, arrastar | **Parcial** (2026-10-31) | Lista, filtro, **ocultar**, **travar** (não selecionável) e **ver só este** por linha e por menu de contexto; estado sobrevive a salvar e reabrir. **Faltam:** cor por linha, multisseleção e arrastar para reordenar |
| QT-13 · Seletor Persp · Orto · 2:1 | **Feito** | `ProjectionMode` com perspectiva e ortográfica, menu **Exibir › Projeção** e o cubo de navegação; o tamanho no plano de foco é preservado |
| QT-14 · Sensibilidade de zoom, roda de cor, atalhos configuráveis | **Planejado** | `color_picker` e `keybinds` estão prontos para copiar |
| QT-15 · Painéis flutuantes (referências, paleta) | **Parcial** | As docas flutuam e a **paleta de comandos** (`Ctrl+K`) existe; um painel de **referências visuais** não existe |
| QT-16 · Menu radial `Shift+A` | **Feito** | `PetuniaPieMenu` copiado sem alteração: aponte e clique, números 1–8, setas e Enter |

## O que veio além da lista

| Item | Estado |
|---|---|
| Três temas do Petunia3D (escuro, claro, **alto contraste**) | **Feito** |
| Densidade, escala de texto e "reduzir animações" | **Feito** (no código; **não** testado com tecnologia assistiva) |
| 287 ícones próprios (`petunia-outline`, MIT) | **Feito** |
| Campo numérico com **expressões** (`2+3*4`, `1,5/2`, `pi`) e arrasto | **Feito**: `Enter` confirma, `Esc` restaura |
| Prévia ao vivo ao arrastar um valor, **um** `Ctrl+Z` por arrasto | **Feito**, testado |
| Atalhos do mapa padrão do Petunia3D: `V` selecionar e transformar, `G` mover, `R` girar, `S` escalar, `Shift+A` primitivas; `WASD` só com o botão direito | **Feito** |
| **Cubo de navegação** no canto superior direito (`PetuniaNavCube`, copiado): face, aresta, canto, bússola, casa | **Feito**; com um disco translúcido atrás, porque o desenho original pressupõe fundo escuro |
| **Grade do chão** com eixos X e Z (porte de `grid_plane_lines`): linha maior a cada 10 m, some com a distância | **Feito** |
| **Pré-seleção**: contorno ciano e leve tinta no objeto sob o mouse | **Feito** |
| **Contorno de seleção de largura constante** (2 px, qualquer distância) | **Feito** |
| **Painel do gesto** (o que está sendo feito, o valor ao vivo, estado do snap) e **guias de eixo** durante o arrasto | **Feito** |
| Marcador de **pivô** e letra em cada eixo (o eixo nunca depende só da cor) | **Feito** |
| **Alvos de snap** do Petunia3D: Inteligente, Grade, Incremento, Ponto (cantos de outros objetos), Superfície (assenta em topos, rampas e andares) e **alinhar à inclinação** | **Feito** |
| **Duplo toque** em G, R ou S: **Modo Livre** (mouse muda o valor, X/Y/Z restringem, digitar fixa o número) | **Feito** |
| **Empurrar/puxar** (`P`) e **Extrudar** (`E`), adaptados a brushes | **Feito** |
| Booleanas adaptadas: **Cavar** (`Ctrl+Alt+C`) e **Tornar oco** (`Ctrl+Alt+H`) | **Feito** (só caixas sem rotação para cavar) |
| **Andares** salvos na cena (tabela `Floor`): subir além do último cria um andar, objetos acima ficam **fantasma** ou ocultos | **Feito** |

## Próximos itens por custo e valor

| # | Item | Custo | Por quê |
|---|---|---|---|
| 1 | Câmera **ortográfica** e o menu de projeção do cubo | Médio | Construir mapa em planta exige vista de topo ortogonal |
| 2 | Paleta de comandos `Ctrl+K` | Médio | A API já é dado: a paleta lista os comandos do schema |
| 3 | Seletor de cor (`petunia_color_picker`, cópia) | Médio | Cor dos brushes |
| 4 | Pivô **por objeto** (mudar a origem) | Médio | Portas que giram na dobradiça |
| 5 | Outliner completo (ocultar, travar, agrupar) | Médio | Mapas grandes |
| 6 | Perfis de atalhos (Blender, Maya…) (`keybinds`, cópia) | Médio | Quem vem de outras ferramentas |

## Cuidados

- O Petunia3D descreve a própria interface como "em conformidade em revisão" e **sem acessibilidade certificada**. Copiamos componentes e desenho, **não** polimento pronto.
- A auditoria de acessibilidade (experimento S8) continua obrigatória antes de dizer que o editor é acessível.
