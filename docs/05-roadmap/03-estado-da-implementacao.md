# Estado da implementação

**Revisão:** 2026-10-25 (edição de malha) · **Fase atual:** 1 (esqueleto) concluída no código; **fase 2 (editor e construção de mapas) começou**

## Resumo (TL;DR)

O editor agora tem a **cara e as ferramentas de base do Petunia3D**: tema (escuro, claro, alto contraste), ícones próprios, campos numéricos com expressões, uma barra principal com as ferramentas, **gizmo de mover, girar e escalar com snap**, vistas predefinidas e seis primitivas (caixa, rampa, cilindro, cone, esfera, escada).

O essencial foi verificado **em display virtual** (nada apareceu na sua tela), com imagens conferidas; a tabela abaixo diz o que **não** foi exercitado. **Ainda não vi a janela na sua sessão real.** A sonda de OpenGL (`--gl-info`) passou na sua máquina; falta você abrir o editor.

Três análises novas pedem a sua decisão: [paridade da interface](../02-arquitetura/12-paridade-ui-petunia3d.md), [edição de malha](../02-arquitetura/13-primitivas-e-edicao-de-malha.md) e [pintura e animação](../01-pesquisa/07-pintura-e-animacao.md).

## Etapa atual: sistema de nós, passo N4 — prefabs e o nó Esperar (2026-10-26)

Decisão: [DR-31](../06-decisoes/registro-de-decisoes.md).

| Entrega | Evidência |
|---|---|
| Nó **Esperar**: compila para uma função por espera, contagem por objeto, reinício, aviso em repetição, só o objeto depois da espera | 4 testes de ponta a ponta no Script Host (a cadeia continua 1 s depois, duas esperas seguidas, não repete) |
| **Prefabs** Porta, Item coletável e Ponto de nascimento (`prefab.list`, `prefab.spawn`), em **Criar** e na hierarquia | 4 testes: os grafos compilam sem aviso; **o jogador pega o item**; **a porta sobe, espera ~3 s e desce, uma vez por visita**; criar = um desfazer e grafo editado nunca é sobrescrito. Menu **visto** no display virtual |
| **Defeito do motor corrigido:** personagem × sensor/parede estática nunca gerava `on_collision` | teste novo no `petunia_physics` (falha sem a correção); 18 testes do crate |

**Não verificado:** a porta e o item **jogando de verdade na janela** com o teclado (só pelo motor de scripts com física e no teste do editor); uma execução isolada do teste de fumaça mostrou 9 de 10 e não vi qual (três repetições deram 10/10); leitor de tela.

**Ainda falta:** a tela de caixas e fios (N3), a depuração (N5), prefabs do próprio projeto, porta animada.

## Etapa atual: sistema de nós, passo N2 (2026-10-26)

Comandos `graph.*` e a **vista em contorno** no editor (workspace **Lógica**, `F7`). Decisão: [DR-30](../06-decisoes/registro-de-decisoes.md).

| Entrega | Evidência |
|---|---|
| `petunia_graph`: editar (adicionar na cadeia, remover fechando o buraco, ligar, desligar, constante), contorno, busca sem acento, fontes de valor | 9 testes novos de ponta a ponta do crate |
| 12 operações `graph.*` no editor, com desfazer próprio, gravação na hora, cópia em `scripts/.backup/` e recusas com a razão | 2 testes do editor, incluindo montar um grafo, desfazer, refazer e **jogá-lo** |
| Workspace **Lógica**: lista de grafos, paleta de nós por busca, contorno `QUANDO … FAÇA`, menu de cada entrada (valor fixo, usar o valor de…, novo nó, esvaziar), Ver como código, Ligar ao objeto, Insert / Enter / Delete / Ctrl+Z | **visto no display virtual**, vários quadros; teste de fumaça **10/10** (cria um grafo pelo mouse e teclado e confere o arquivo) |

**Não verificado:** leitor de tela (as linhas têm texto e descrição falados prontos, mas **não ouvi**); o `F7` com teclado real (no teste de fumaça o `xdotool` desmapeia a tecla depois de digitar, então ele clica na aba); grafos grandes.

**Ainda falta:** a tela de caixas e fios (N3), os prefabs como grafos (N4) e a depuração (N5).

## Etapa atual: sistema de nós, passo N1 (2026-10-26)

Fase 3, no lugar do Event Sheet. Decisão: [DR-30](../06-decisoes/registro-de-decisoes.md). Esta etapa é **só o modelo e o compilador**; ainda **não há tela de nós nem vista em contorno**.

| Entrega | Evidência |
|---|---|
| Crate `petunia_graph` (domínio puro): documento `.graph.json`, limites, catálogo gerado da API (um nó por função), validação em português com nó e pino, compilador para Aipo com mapa linha → nó | 6 testes do crate |
| Integração no Script Host: `load_graph`, `load_directory` lê `*.graph.json`, o Project Doctor lista erros de grafos, o **Jogar** do editor carrega grafos | 12 testes de ponta a ponta + 1 no editor (um grafo gira um objeto no Jogar) |
| Ponta a ponta: condição, conta, variável entre quadros, repetir, para cada, texto com aspas e quebra de linha | testes passam |
| Arquivos hostis: JSON inválido, campo extra, versão nova, ids repetidos, ligação para nó inexistente, 600 nós, lista escondida em parâmetro | recusados |
| Mesmo grafo, mesma saída, qualquer ordem dos nós | teste |

**Não verificado:** a leitura por leitor de tela (não há interface ainda); grafos grandes (só grafos de teste); o desempenho do código gerado.

**Próximo (N2):** comandos `graph.*` com desfazer e a **vista em contorno** `QUANDO … FAÇA`, a primeira interface acessível.

## Etapa atual: luz de cena e importação de modelos (2026-10-26)

Fase 2 do pedido "pendências e fases 1 a 6". Decisão: [DR-29](../06-decisoes/registro-de-decisoes.md).

| Entrega | Evidência |
|---|---|
| Tabela `Light` (sol e pontual); renderizador com até 8 luzes pontuais | 1 teste no editor + **visto (display virtual)** |
| Menus **Criar > Luz pontual / Sol** com marca no viewport | visto |
| Crate `petunia_import` (`gltf` 1.4.1): GLB vira malhas soldadas, com cor e transformação | 6 testes, incluindo arquivos hostis |
| `asset.list` e `asset.import`: cópia em `assets/`, um passo de desfazer, erros claros | 2 testes no editor |
| **Arquivo > Importar modelo 3D…** | compila; **não abri o seletor de arquivos** |
| Correção: título do inspetor ficava vazio sem a tabela `Collider` | teste que falha sem a correção |
| Teste de fumaça: menus ganharam itens, posições ajustadas | **9/9** |

**Não verificado:** importar um modelo real de um programa de modelagem (só GLBs montados nos testes); a sua sessão real; leitor de tela.

**Ainda falta da fase 2:** painel de assets (hoje só a lista por `asset.list`); alças de colisão no viewport; material por face.

## Última etapa: personagem, modo Jogar e solidez automática (2026-10-26)

Bloco 1 do pedido "pendências e fases 1 a 6": fecha o essencial da fase 3. Decisão: [DR-28](../06-decisoes/registro-de-decisoes.md).

| O quê | Verificação |
|---|---|
| **Corrigido (da revisão do PR #1):** apagar objeto por script leva as colisões; mudar `Collider`/`RigidBody` por script refaz a física | 1 teste pelo script |
| **Camadas e máscaras** de colisão (`layer`, `mask`) | teste: um piso em outra camada é atravessado |
| **Personagem** (`Character`): andar, degraus, rampas, pulo, **tempo de coiote**; API `character.*` | **6 testes** no adaptador + 2 pelo script; determinismo bit a bit |
| **Solidez automática**: tudo que é desenhado é sólido sem configurar; `SolidShape` (novo, em `petunia_math`, 2 testes) | teste: a sala de exemplo é andável, o herói pisa no chão e **para na parede** |
| **Modo Jogar** (`F6`) numa cópia da cena; herói de reserva no `PlayerStart` (`petunia_project::ensure_player`, 1 teste); câmera em terceira pessoa que **não entra na parede** | 2 testes pelo JSON + **visto (display virtual)**, 3 quadros: início, pulo, parar |
| `petunia run` usa o mesmo caminho | compila; **não rodou com janela** |
| Teste de fumaça ganhou Jogar/Parar e a prova de que a cena salva **não** guarda o herói | **9/9** |
| Workspace inteiro | **341 testes, 0 falhas** (build isolado); fmt e clippy `-D warnings` ok |

**Não verificado:** a sua sessão real; o jogo no `petunia run` numa janela; teclado e mouse reais no modo Jogar (só `xdotool` no display virtual); leitor de tela.

**Ainda falta do bloco:** alças no viewport para redimensionar colisões; prefabs (Porta, Pickup, Spawn); Event Sheet; módulo `grid`/A\*; material por face; painel de assets.

## Antes: contorno girado e colisão com física (2026-10-25)

| O quê | Verificação |
|---|---|
| **Contorno da seleção acompanha a rotação** (caixa no espaço do objeto); cor e espessura em **Preferências** (`Ctrl+,`) | 1 teste (arestas inclinadas, estilo aplicado, limites recusados) + **visto (display virtual)** |
| Crate **`petunia_physics`** (Rapier 0.36.1, determinístico) e leitor de cena ([DR-27](../06-decisoes/registro-de-decisoes.md)) | **10 testes** |
| Tabelas `Collider` (várias **com nome** por objeto) e `RigidBody` | leitor de cena testado |
| Script Host: física por quadro, `on_collision`, `collision.*`, `physics.*` (inclui `raycast_from`) | **4 testes** pelo script |
| Editor: adicionar (caixa, esfera, cápsula, convexa), ajustar, listar sob o objeto, desenhar em verde, duplicar e excluir com o objeto | 1 teste de fluxo + **visto (display virtual)** |
| `petunia run` liga a física quando há colisões | compila; **não rodou com janela** |
| Workspace inteiro | **326 testes, 0 falhas**; clippy `-D warnings` e fmt ok; teste de fumaça **8/8** |

**Atenção, verificação:** existe uma cópia do projeto (`~/Documentos/Projetos/petunia-engine-system-modules`) com a **mesma pasta de build**. Por isso os números acima saíram de pastas isoladas (`/tmp/petunia-ui-target`, `/tmp/petunia-ui-editor`).

**Não verificado:** a sua sessão real; o jogo com física numa janela; alças no viewport para redimensionar colisões (hoje pelo inspetor).

## Antes: edição de malha do Petunia3D (2026-10-25)

Booleanos (fundir, cortar, interseção), inset, extrudar, loop cut, bevel, subdividir, dissolver, fundir vértices, apagar e virar faces, faca. Decisão: [DR-22](../06-decisoes/registro-de-decisoes.md) (a malha pronta fica guardada na cena). Detalhes: [edição de malha](../02-arquitetura/13-primitivas-e-edicao-de-malha.md#nível-3-o-que-foi-feito-2026-10-25).

| O quê | Verificação |
|---|---|
| Crate `petunia_mesh`: código do Petunia3D (legado) + API validada | **181 testes passam**: 164 do próprio Petunia3D + 17 da API (volumes exatos, sólido fechado, entrada hostil, determinismo) |
| Tabela `Mesh`, tipo de valor `mesh`, uma linha por vértice no arquivo | testes de ida e volta, recusa de malha quebrada; **salvar e abrir de novo dá a malha bit a bit igual** |
| Operações `mesh.*` no editor (tornar editável, booleano, escolher partes, as operações) | **5 testes de ponta a ponta pelo JSON**: cada operação é **um** desfazer; booleano com ferramenta girada e esticada dá o volume exato |
| Malhas desenhadas, selecionáveis e com gizmo e snap (mesmo caminho dos brushes) | teste de picking em malha; **não vista em imagem** |
| Salvar recusa cena maior que o limite de abrir (antes: salvava e não abria mais) | teste: o arquivo antigo fica igual |
| Histórico de desfazer com teto de memória (256 MB) | teste: os passos antigos saem, o desfazer continua certo |
| Subdivisão sem rachaduras nos vizinhos (defeito do Petunia3D corrigido na API) | teste: o sólido continua fechado, também subdividindo metade de uma malha densa |
| **Valor ao vivo** ao arrastar (`mesh.preview`): inset, extrudar, bevel, loop cut | 2 testes: um arrasto = um desfazer; cancelar não deixa rastro; ir e voltar dá o mesmo resultado |
| Desempenho no build do editor (6,5 mil vértices) | **medido**: booleano 74 ms, inset em todas as faces 29 ms, checar sólido 3 ms |
| `cargo fmt`, `clippy -D warnings`, **310 testes** (2 threads), `cargo deny` | **passou**, recompilado do zero numa pasta de build **isolada** (`/tmp/petunia-x.verify`); o `cargo deny` rodou com a base de avisos local (`--offline`) |
| **Falso verde encontrado:** a cópia `petunia-engine-system-modules` usa a **mesma** pasta de build (`/tmp/petunia-engine-target`) | Uma cópia sobrescreve os binários da outra, e o `cargo test` chegou a rodar o `petunia_math` da cópia (5 testes em vez de 9) dizendo "ok". Até a cópia ter outra pasta de build, **verifique com `CARGO_TARGET_DIR` próprio** |

**Não verificado:** nada disso foi **visto** em janela (nem no display virtual), porque a interface Qt ainda não tem os botões. Os ganchos que faltam estão [listados aqui](../02-arquitetura/13-primitivas-e-edicao-de-malha.md#ganchos-da-interface-que-faltam). Malhas acima de 6,5 mil vértices ainda não foram medidas.

## Antes: design da interface, workspaces e paleta de comandos (2026-10-25)

Pesquisa e regras em [design da interface](../02-arquitetura/15-design-da-interface.md); decisão [DR-26](../06-decisoes/registro-de-decisoes.md) (revista).

| O quê | Verificação |
|---|---|
| Medidas novas: texto de 12 px, ícones de 16 px, controles de 24 px; neutros quentes e acento violeta nos temas claro e escuro | **visto (display virtual)**; contraste **calculado** (todos os textos ≥ 4,5:1) |
| Menus atrás de `≡`, `Alt+letra`, `F10`, "Menus por extenso" | **visto**; o teste de fumaça usa `Alt+C` |
| **Paleta de comandos** (`Ctrl+K`): busca tolerante, destaque das letras, recentes, atalhos | **vista** com "cvr" → Caverna; não tem teste automático |
| **Workspaces Cena e Código** (`F3`/`F4`), cada um com seu arranjo de painéis | **visto**; o teste de fumaça cria e salva um script no workspace Código |
| Coluna de ferramentas no viewport; faixa da cena com snap num botão e painel | **visto** (o painel do snap e o menu `≡` capturados na tela virtual) |
| Inspetor e hierarquia compactos, rótulos e tabelas em português | **visto** |
| Referência da API com as funções do script primeiro | **vista** |
| Ícone de busca quebrado, vindo do Petunia3D | corrigido; os outros 286 ícones conferidos |
| Teste de fumaça com as coordenadas novas | **7/7 ok** |

**Não verificado:** a sua sessão real; leitor de tela; navegação só por teclado nos painéis novos; a paleta sem mouse em todos os casos.

## Antes: layout da janela (2026-10-25)

Primeira versão do redesenho (uma barra no topo, hierarquia em árvore, inspetor em seções). Substituída pela etapa acima.

## Antes: Script Host e editor de código (2026-10-25)

| O quê | Verificação |
|---|---|
| Crate **`petunia_script`**: compila e roda scripts **Aipo**, uma VM por arquivo, `on_start` / `on_update` / `on_event`, orçamento de 100 mil instruções por chamada ([doc](../03-linguagens-e-api/05-script-host.md), [DR-25](../06-decisoes/registro-de-decisoes.md)) | 20 testes (ciclo de vida, orçamento, falha tratada e não tratada, sandbox, eventos, isolamento, determinismo, simulação) |
| API do script (`world`, `game`, `log`) descrita **uma vez** como dado: superfície do compilador, funções nativas e ajuda do editor | teste de coerência da tabela |
| Tabela `Script` (`file`, `enabled`); `petunia check` lista erros de script com linha e coluna | testado de ponta a ponta (sai com código 1) |
| `petunia run` executa os scripts todo quadro | **não rodou** (precisa de janela) |
| Operações `script.*` no editor; erros de script no painel Diagnósticos (`SCRIPT_MISSING` para objeto sem arquivo) | 1 teste de fluxo completo |
| **Editor de código** (`F4`): destaque, números de linha, erros ao digitar, sugestões, recuo, Executar numa cópia (`F5`), salvar com cópia de segurança | **visto (display virtual)** + **teste de fumaça** (cria, digita e salva um script) |
| Cavernas: primeira construção e desfazer com 1000 traços **459 ms → 189 ms** (replay paralelo em fatias) | medido em `--release`; teste de igualdade bit a bit com o serial |

**Não verificado:** nada disso foi visto na **sua** sessão real; `petunia run` com scripts não rodou com janela.

## Antes: cavernas rápidas e completas (2026-10-25)

| O quê | Verificação |
|---|---|
| **Campo em memória, blocos de 16³, malha por bloco**: uma pincelada custa ~1 ms em qualquer tamanho (antes 54 a 1063 ms) | 16 testes do crate; **medido** em `--release` (tabela no [doc 14](../02-arquitetura/14-workspaces-e-prioridades.md)) |
| Malha **uma vez só** para desenhar e clicar; o clique rejeita blocos por caixa | 3 testes no renderizador (acerto, movido e escalado, sem refazer) |
| Malha em até 4 threads, sem dependência | usado em todos os testes de blocos |
| **5 modos de pincel**: cavar, adicionar, inflar, nivelar, suavizar (`Ctrl` faz o oposto) | teste: cada modo muda a superfície e desfaz em **um** passo |
| **Túnel por caminho** (spline que passa pelos pontos), `Shift+B` | teste + **teste de fumaça pela janela** |
| **Gerador** de cavernas por semente; *Criar > Caverna aleatória*; *Gerar túneis* | 3 testes (determinismo, válido e oco, pequeno demais) + **visto (display virtual)** |
| **Fundir traços** (Douglas-Peucker em cadeias de cavar e adicionar) | 2 testes de forma + 1 de fluxo (um desfazer) |
| **Facetada** (low poly) por derivadas de tela | **visto (display virtual)**: triângulos chapados |
| Barra de opções (modo, raio, força, facetada, gerar, fundir, contador de traços) | **visto (display virtual)** |

**Não verificado:** nenhuma destas coisas foi vista na **sua** sessão real; a pausa de ~0,5 s ao abrir/desfazer uma caverna de 1000 traços **não foi corrigida** (só medida); inflar, nivelar e suavizar só foram vistos nos testes, não olhados em imagem.

## Etapa anterior: cavernas (2026-10-25)

| O quê | Verificação |
|---|---|
| Crate novo **`petunia_sculpt`**: distância com sinal, cápsulas que cavam ou somam, malha por *surface nets*, determinístico, sem dependências | 5 testes: esfera fechada e voltada para fora, cavidade voltada para dentro, túnel que atravessa, mesma malha duas vezes, números ruins recusados |
| Tabelas `Sculpt` e `SculptStroke`; desenho no OpenGL com malha refeita só quando o volume muda | **visto (display virtual)**: túnel com paredes suaves; bloco arredondado visto de fora |
| `world.batch` aceita `{"$entity_of": n}`: criar a caverna e seu túnel é **um** desfazer | teste (inclui referência inválida desfeita sem deixar nada) |
| Ferramenta **Esculpir** (`B`), anel do pincel, `Ctrl` adiciona, `[` `]` raio; **Criar > Caverna** | 2 testes; **teste de fumaça**: caverna criada e esculpida pela janela (8 traços) |
| Clique seleciona a caverna; objetos assentam no chão dela; duplicar e apagar levam os traços | teste |

## Etapa anterior: andares completos (2026-10-25)

| O quê | Verificação |
|---|---|
| Andares são **entidades** com `Name` e `Floor` (`elevation`, `height`): salvam, desfazem e aparecem na Hierarquia | 2 testes (subir, salvar e reabrir, desfazer; subsolo) |
| Subir além do último andar **cria** o próximo (do térreo implícito, cria Térreo e Andar 1 juntos); **um** `Ctrl+Z` desfaz | teste; **teste de fumaça**: `Ctrl+PageUp` criou os dois andares e a caixa nasceu em y = 4,0 |
| Objetos acima do andar: **fantasma** (padrão; translúcidos e fora do alcance do mouse), ocultar ou mostrar | teste; **visto (display virtual)**: o cilindro do andar 1 aparece translúcido |
| Passe translúcido no OpenGL (depois dos opacos, do mais longe ao mais perto, sem gravar profundidade) | visto (display virtual) |
| Lista de andares na barra; modo "acima" em **Exibir** | visto (display virtual) |

## Etapa anterior: snap a objetos, Modo Livre, pivô, operações e andares

| O quê | Origem | Verificação |
|---|---|---|
| **Alvos de snap**: Ponto (cantos de outros objetos) e Superfície (assenta em topos e rampas; **alinhar à inclinação**) | Porte de `SnapTarget` | testes Rust; **interface não exercitada** |
| **Modo Livre** por duplo toque em G, R ou S (350 ms) | Porte do manual de modelagem do Petunia3D | **interface não exercitada** |
| **Pivô por objeto** (`Transform.pivot`), **Editar pivô** (`Alt+P`); cursor 3D **removido** | Seu pedido | teste de girar em volta do pivô; edição pela interface **não exercitada** |
| **Empurrar/puxar** (`P`) e **Extrudar** (`E`) | Porte adaptado a brushes | **visto (display virtual)**: topo puxado +1,00 m, base ficou no chão |
| **Cavar** e **Tornar oco** | Novo (caixa menos caixa) | 2 testes de geometria; **interface não exercitada** |
| **Andares**: grade sobe e desce, objetos novos nascem no andar | Novo | teste de fumaça: rampa criada em y = 1,0 no andar 0 |
| Comando `world.batch` (tudo ou nada, **um** desfazer) | Novo, no despachante | teste |
| Script `scripts/clean-builds.sh` e regra no `AGENTS.md` | Seu pedido | rodou: liberou 879 MB na primeira vez |

**Não verificado na interface:** Modo Livre, snap Ponto e Superfície, cavar e tornar oco, andares acima de 0, editar pivô, cliques no cubo de navegação. Nada disso foi visto na **sua** sessão real.

## O que existe

**37,5 mil linhas de Rust** em 15 crates (14,1 mil delas copiadas do Petunia3D em `petunia_mesh/src/legacy/`), **6,1 mil linhas de C++** próprias e **3,6 mil linhas de C++ copiadas** do Petunia3D (`editor/src/ui_kit/`; o tema foi reescrito, o resto está igual).

| Parte | Linhas | O que faz |
|---|---:|---|
| `petunia_math` | 753 | `Vec2`, `Vec3`, `Color`, `Transform`, **`PolygonMesh`** (malha editável como dado, triângulos para desenhar) |
| `petunia_world` | 1444 | Entidades, tabelas, tags, comandos adiados, RNG; `f32` salvo em decimal curto; tipo de valor **`mesh`** |
| `petunia_schema` | 466 | API como dado |
| `petunia_commands` | 1245 | Despachante com desfazer/refazer; histórico com **teto de memória** |
| `petunia_project` | 1074 | Formato de texto, salvar (recusa cena grande demais para abrir), diagnósticos, projeto de exemplo; tabela **`Mesh`** |
| `petunia_input` | 166 | Entrada por ações nomeadas |
| `petunia_render` | 1831 | Câmera, **seis formas** e **malhas editáveis**, picking, **overlay para gizmos**, volumes esculpidos no quadro |
| `petunia_sculpt` | 2176 | **Cavernas**: campo em blocos, malha por *surface nets*, gerador, caminho, fundir traços |
| `petunia_script` | 3614 | **Script Host**: compila e roda Aipo, API como dado, Doctor de scripts, simulação numa cópia |
| `petunia_render_gl` | 820 | OpenGL 3.3: contorno de seleção, **passe de overlay em pixels**, buffers de malhas editáveis |
| `petunia_player` | 386 | Janela simples com câmera livre |
| `petunia_cli` | 177 | `petunia new / check / run / schema / version` |
| `petunia_mesh` | 15361 | **Edição de malha**: booleanos, inset, loop cut, bevel… (14100 linhas de legado do Petunia3D + API validada) |
| `petunia_editor` | 7801 | Sessão, operações JSON, **gizmo**, **gestos**, snap a objetos, Modo Livre, **operações de brush**, andares, **esculpir**, **edição de malha** |
| `petunia_editor_ffi` | 210 | Fronteira C |
| `editor/` (C++/Qt) | 6138 | Janela, barra principal, workspaces, paleta de comandos, faixa da cena, docas, inspetor, editor de código |
| `editor/src/ui_kit/` (copiado) | 3567 | Tema, campo numérico, campo XYZ, expressões, arrasto, ícones |

Detalhes: [editor](../02-arquitetura/11-editor.md), [paridade](../02-arquitetura/12-paridade-ui-petunia3d.md).

## Verificação (honesta)

| Item | Estado | Evidência |
|---|---|---|
| `cargo fmt`, `cargo clippy --workspace --all-targets -D warnings` | **passou** | limpo |
| Suíte completa, **301 testes** (2026-10-25, depois da edição de malha) | **passou** | mesh 179 (164 do Petunia3D), script 30 (14 + 16 do host), editor 25, sculpt 17, render 15, world 13, math 9, commands 6, input 3, project 2, schema 2 |
| Formas orientadas para fora (caixa, rampa, cilindro, cone, esfera, escada) | **passou** | teste por geometria, em vários níveis de detalhe |
| Gizmo: acertar a alça, mover só no eixo e no snap, girar em passos, escalar sem zerar | **passou** | 4 testes em `petunia_editor` |
| Um arrasto vira **um** passo de desfazer; sem mudança, nenhum | **passou** | teste de gesto |
| Editor Qt compila e liga | **passou** | `scripts/editor.sh build` |
| Janela: selecionar, **arrastar a seta X**, **adicionar tabela pelo combo**, duplicar, criar, andar, girar, salvar | **passou (display virtual)** | `scripts/editor-smoke.sh` **confere o arquivo salvo** (`x=4.5`, só no eixo X, na grade; tabela `PlayerStart` adicionada) |
| Aparência: tema escuro, ícones, campos XYZ, trilho, barra do viewport | **visto (display virtual)** | imagens |
| Gizmos de girar e de escalar, cone, esfera e escada renderizados | **visto (display virtual)** | imagens |
| Realce da alça ao passar o mouse | **visto (display virtual)** | imagem do gizmo de escalar (alça Y em amarelo) |
| Vistas predefinidas (`Alt+1` a `Alt+6`), `Esc` cancela o arrasto | **não rodou** | código escrito |
| Arrastar um **anel** ou um **quadrado de escala** com o mouse | **não rodou** | só a matemática tem teste; a interface só foi exercitada com a seta de mover |
| Trocar tema, densidade e escala pelo menu; alto contraste | **não rodou** | código escrito; só o tema escuro foi visto |
| Digitar expressões nos campos (`2+3*4`) | **não rodou** | campo copiado e compilado; o caminho de confirmação tem teste |
| Editor na **sua sessão real** (RTX 3060) | **não rodou a janela** | `--gl-info` passou |
| Acessibilidade (leitor de tela, só teclado) | **não rodou** | |
| `cargo deny check` | **passou** (com avisos normais de duplicações transitivas) | `deny.toml` com `allow-wildcard-paths` e `RUSTSEC-2026-0192` justificado |
| `cargo xtask …`, regressão visual pixel a pixel | **não rodou / planejado** | |

## Como abrir (você roda)

```bash
cd ~/Documentos/Projetos/petunia-engine
scripts/editor.sh
```

Se o Wayland recusar o OpenGL, o editor reinicia sozinho via X11. Para diagnosticar: `/tmp/petunia-editor-build/petunia-editor --gl-info`.

No editor:

1. **Arquivo > Novo projeto de exemplo**.
2. **Clique** num objeto, depois `G` (mover): arraste uma seta. `R` gira, `S` escala, `V` volta ao universal. **Duplo toque** em `G` entra no Modo Livre.
3. `Shift+A` abre o menu de primitivas; elas nascem na frente da câmera, no andar atual.
4. O botão **Snap** e os passos ficam acima do viewport; `Ctrl` inverte durante o arrasto.
5. `Ctrl+Z` desfaz o arrasto inteiro de uma vez. `Ctrl+S` salva.

## Decisões tomadas durante a implementação

| Decisão | Motivo |
|---|---|
| Qt Widgets + JSON sobre interface C ([DR-21](../06-decisoes/registro-de-decisoes.md)) | Mesma porta para janela, testes e agentes |
| **Brushes paramétricos** com gizmo e snap ([DR-22](../06-decisoes/registro-de-decisoes.md), nível 1) | O que mais pesa em construir mapa, sem puxar a edição de malha |
| **Gesto:** prévia ao vivo, um comando ao soltar | Um `Ctrl+Z` por arrasto; o histórico não se enche |
| **Snap absoluto** ao mover | O objeto cai na grade, não só anda em passos |
| **`WASD` só com o botão direito**; `Q W E R` são ferramentas | Convenção de Unity, Godot e Unreal; letras livres para atalhos |
| Gizmo de girar em eixos de **Euler** (gimbal) | Cada anel gira exatamente o componente que leva o nome |
| Cópia fotografada do kit de UI do Petunia3D, sem alteração ([ATTRIBUTION](../../ATTRIBUTION.md)) | Tema, campos e ícones são a parte mais cara de acertar |

## Defeitos encontrados e corrigidos

| Defeito | Como apareceu | Correção |
|---|---|---|
| Arquivos de cena com `0.6200000047683716` | Conferi o JSON salvo | `f32` sai como decimal curto; teste |
| **O editor não abria no Wayland** (erro EGL 3009) | Você rodou | Teste de formatos e reinício via X11; `--gl-info` |
| Ferramentas do trilho apareciam como **texto** | Imagem do teste | `icon` é propriedade nativa do `QAction`: eu a sobrescrevia; troquei o nome |
| Caixa de escala mostrava `0,05×` em vez de `0,1×` | Imagem do teste | `f32` chega como `0.10000000149`; comparação com tolerância |
| Teclas `W E Q` iam para o atalho de ferramenta em vez de voar | Revisão do desenho | `WASD` só com o botão direito; `ShortcutOverride` no viewport |
| Teste de normais reprovava cone e esfera de 3 segmentos | Teste novo | Com 3 segmentos a normal suave difere muito da plana: limite do teste ajustado (a orientação já passava) |
| **O editor caía (SIGSEGV) ao escolher "Adicionar tabela…"** no inspetor | Você rodou; o relatório da queda (`coredumpctl`) apontava um `QComboBox` | O inspetor se reconstruía e **apagava o próprio combo de dentro do sinal dele**. Agora esconde e usa `deleteLater()` (vale também para "Remover tabela"). Reproduzido no display virtual, corrigido e **incluído no teste de fumaça** |
| A roda do mouse sobre um combo **mudava o valor** (rolar o inspetor trocava a forma de um brush) | Revisão da imagem | `WheelGuard`: o combo só aceita a roda quando tem foco |
| **Anel Z apagado** quando visto de frente | Imagem | Todos os pontos com profundidade ~0 contavam como "de trás"; ~0 agora conta como frente |
| **O anel da vista roubava cliques** do anel de eixo | Arrasto simulado pegou o anel errado | Regra herdada do Petunia3D (testava o anel da vista primeiro; as faixas se sobrepõem entre 73 e 84 px). Agora vence o mais próximo; teste |
| Barra do viewport deixava a janela com 1.740 px | Imagem | Botões de vista saíram (o cubo faz isso) |
| Rótulos do cubo ilegíveis sobre o céu claro | Imagem | Disco translúcido escuro atrás do cubo |
| Altura do cursor 3D com ruído (`0,99999976`) | Arquivo salvo | Altura arredondada ao milímetro (o cursor depois saiu, a seu pedido) |
| **`sh.run` da Aipo roda programas do sistema sem checar permissão** | Leitura do código da Aipo | O host monta a VM por lista de permitidos: `sh`, `fs`, `env` e `io` são **removidos**; teste prova que não compilam |
| Orçamento estourado chegava como `AIPO_RT_OVERFLOW` | Teste | Traduzido para `SCRIPT_BUDGET_EXCEEDED`, com o limite e o que fazer |
| Uma `Failure` não tratada passava por sucesso | Teste | Agora é falha do script (`SCRIPT_FAILURE`); com `attempt` o script segue |
| O formatador da Aipo "formatava" texto quebrado | Teste | O texto precisa passar na sintaxe antes |
| O painel de código aumentava a altura mínima da doca inferior e **quebrou o teste de fumaça** | Teste de fumaça + imagem | Alturas mínimas explícitas pequenas; layout de volta ao de antes |
| Minha lista de palavras-chave estava errada (não existe `for`) | Leitura do analisador léxico | A lista agora é a do `aipo-lexer` |
| **O editor caía (SIGSEGV) ao fechar** com o cursor num campo de raio ou de força | Teste visual; `coredumpctl` + `addr2line` apontaram a função | Ao destruir a janela, o campo com foco emite `editingFinished` e o editor chamava o motor **já destruído**. Agora `closing_` bloqueia chamadas no encerramento |
| Vértices **repetidos no mesmo ponto** quando a superfície passa exatamente por uma amostra da grade | Teste de malha fechada reprovou | Valores a menos de 1 mm da superfície vão para o lado do ar; era degenerescência, não bloco |
| O `cargo` do atalho do `mise` tentou **atualizar o Rust** para 1.99.1 e falhou no meio | Saída do `cargo test` | O toolchain **estava intacto** (1.97.1). Passei a usar o `cargo` do toolchain direto; **não mexi na sua configuração do mise** |
| Malha da caverna **não fechava** nos cantos do bloco | Teste novo reprovou | A orientação de cada quadrilátero vinha de um teste geométrico, que vira nas dobras; agora vem só da ordem das células e do lado da rocha |
| Hierarquia dizia "22 objetos" e listava 12 | Imagem | Os traços contavam; o contador agora mostra objetos, não traços |
| Teste de fumaça "reprovou tudo" | Teste | Os passos novos passaram do limite de 21 s e o editor fechou **antes de salvar**; limite em 27 s. **Fragilidade conhecida:** o teste depende de um tempo fixo |
| Lista de andares cortada ("Térreo ·") | Imagem | A caixa media o próprio conteúdo **antes** de ser preenchida; largura mínima fixa |
| A barra deixou a janela com 1.511 px | Imagem | Altitude passou para a dica da lista e o modo "acima" foi para o menu **Exibir** (de volta a 1.360 px) |
| Teste de fumaça escolheu a tabela errada | O teste reprovou | A tabela nova `Floor` entrou antes de `PlayerStart` na lista; o teste agora desce dois itens |
| **Apaguei cinco funções do viewport por engano** ao substituir um trecho | Erro de ligação | Restauradas; o teste de fumaça voltou a passar |
| Empurrar/puxar **escorregava** a cada movimento | Revisão do código | A face de referência era atualizada durante o arrasto; agora guarda a face **inicial** |
| A barra do viewport deixou a janela com 1.951 px | Imagem | Barra em **duas linhas** (de volta a 1.360 px) |
| Teste abortou (SIGABRT) | Dois testes seguidos no mesmo display virtual | **Não era o editor**: o Qt não achou o display. Testes rodam um de cada vez |
| Dois números errados na minha análise (11 comandos de operação; UV 3,9 mil) | Releitura | Linha removida; 3.665 + 345 = ~4 mil |

## Limites conhecidos

Veja [editor](../02-arquitetura/11-editor.md). Os maiores: sem câmera ortográfica, lista de objetos sem paginação, sem salvamento automático, cavar só em caixas sem rotação (use `mesh.boolean` para o resto), edição de malha ainda sem interface.

## Pendências que não são código

| Item | Nota |
|---|---|
| `TODO(human)` em `petunia_schema/src/naming.rs` | Política dos verbos da API; hoje aceita qualquer verbo |
| **DR-23** e **DR-24** (workspaces e prioridades) | Aguardam sua decisão |
| Interface da edição de malha (menu, `Tab`, atalhos) | O motor está pronto; a janela está sendo redesenhada em outra sessão |
| `catch_unwind` no despachante | Planejado |

## Próximos passos

1. **Você abrir o editor** (`scripts/editor.sh`), `F4`, criar um script e executar com `F5`.
2. Rodar `petunia run` num projeto com scripts e conferir o console.
3. Event Sheet / Gameplay IR (regras sem código) sobre o mesmo Script Host.
4. Rodar a **bateria restante** (`xtask`, regressão visual pixel a pixel quando implementadas).
