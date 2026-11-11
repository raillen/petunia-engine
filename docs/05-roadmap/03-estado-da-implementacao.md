# Estado da implementação

**Revisão:** 2026-11-10 (prefabs do próprio projeto; linhas consolidadas: cenas, animação, vegetação, outliner, grid, prefabs, interface de malha e 2D) · **Fase atual:** 1 (esqueleto) concluída no código; **fase 2 (editor e construção de mapas) avançou**

## Resumo (TL;DR)

O motor agora também faz **2D do jeito que o 3D faz**: um sprite é uma **entidade comum** com `Transform` e a linha `Sprite` (imagem `.png`, tamanho, cor, camada), então **gizmo, inspetor, snap, andares, desfazer e gravação** valem sem código novo — a câmera ortográfica já existia. Uma **folha de sprites** é só mais uma linha (`SpriteAnimation`): o quadro vem do relógio, nunca da cena. Decisões: [DR-49](../06-decisoes/registro-de-decisoes.md) e [DR-50](../06-decisoes/registro-de-decisoes.md).

O editor agora tem a **cara e as ferramentas de base do Petunia3D**: tema (escuro, claro, alto contraste), ícones próprios, campos numéricos com expressões, uma barra principal com as ferramentas, **gizmo de mover, girar e escalar com snap**, vistas predefinidas e seis primitivas (caixa, rampa, cilindro, cone, esfera, escada).

O essencial foi verificado **em display virtual** (nada apareceu na sua tela), com imagens conferidas; a tabela abaixo diz o que **não** foi exercitado. **Ainda não vi a janela na sua sessão real.** A sonda de OpenGL (`--gl-info`) passou na sua máquina; falta você abrir o editor.

Três análises novas pedem a sua decisão: [paridade da interface](../02-arquitetura/12-paridade-ui-petunia3d.md), [edição de malha](../02-arquitetura/13-primitivas-e-edicao-de-malha.md) e [pintura e animação](../01-pesquisa/07-pintura-e-animacao.md).

**Consolidação de 2026-10-27:** os PRs #2 a #7 foram mesclados na `main` (personagem e Jogar, luzes e GLB, grafo de nós N1–N5, prefabs). A `main` tinha **397 testes** antes destas correções; com elas, tem **441 testes, 0 falhas** (2026-10-28), e o editor compila no Qt 6.4.2.

## Etapa atual: prefabs do próprio projeto (2026-11-10)

Fecha o item que a fase 2 listava desde o `DR-31` (“prefabs do próprio projeto”). Decisão: [`DR-51`](../06-decisoes/registro-de-decisoes.md).

| Entrega | Evidência |
|---|---|
| `prefab.save`: o objeto selecionado vira `prefabs/<nome>.prefab.json` — as linhas do objeto mais as **colisões**, os **scripts extras** e os **traços** que apontam para ele; a posição gravada vira zero, então o molde não guarda o lugar onde foi feito | 3 testes: a caixa com colisor volta com o colisor, `prefab.list` vê 5 e criar é **um** desfazer; a cópia nasce na frente da câmera, não onde foi salva; salvar de novo sobe a revisão e arquivo quebrado, nome desconhecido e falta de seleção são recusados com o motivo |
| `prefab.spawn_project` e `prefab.spawn` com `id: "project:<nome>"`: cria na frente da câmera, no andar atual, num `world.batch` (um desfazer), já selecionado | os mesmos 3 testes |
| Janela: **Criar ▸ Salvar seleção como prefab…** (nome no diálogo) e **Criar ▸ Prefab do projeto** (submenu recriado ao abrir, com “(nenhum prefab salvo)” quando vazio) | visto no display virtual; `scripts/editor-smoke.sh` **18/18**: o passo novo salva a malha selecionada como `smoke`, cria a cópia pelo submenu e confere o arquivo (posição zero, revisão 1) e as duas malhas na cena salva |

**Limites declarados:** o molde guarda **um** objeto por arquivo (o formato já aceita `objects` com vários, mas o salvar grava a seleção); sem seleção, não salva. Salvar o mesmo nome sobe a revisão, mas **cópias já colocadas nunca são reescritas** — atualizar o que já está na cena precisa de decisão própria. O prefab guarda o **anexo** de script, não o arquivo: apagar `scripts/<nome>.aipo` deixa o anexo apontando para um arquivo que não existe (o Project Doctor avisa, como para qualquer anexo).

**Não verificado:** a janela na sua sessão real; leitor de tela; um prefab com vários objetos (o formato aceita, o salvar atual grava um); a cópia criada dentro de uma parede ou rocha (nada impede).

**Reabrir se:** o molde precisar de vários objetos, se a revisão tiver de alcançar as cópias já colocadas ou se um prefab precisar levar os arquivos de script junto.

## Antes: módulo `grid` e A\* (2026-11-09)

Decisão: [DR-46](../06-decisoes/registro-de-decisoes.md). O desenho existia desde 2026-10-24 ([personagens, UI e grade](../02-arquitetura/09-personagens-ui-e-grade.md) §5) com uma API ilustrativa e nenhuma linha de código; agora as oito funções existem e rodam em script.

| Entrega | Evidência |
|---|---|
| Crate `petunia_grid` de domínio puro: grade **quadrada** (`\|Δq\| + \|Δr\|`, quatro vizinhas) e **hexagonal** (`s = -q - r`, fórmula cúbica, seis vizinhas); `range` em ordem fixa; `path` por A\* com a distância exata de heurística; conversões célula↔metros | 8 testes: distância, 13/19/91 células de alcance, vizinhas nas bordas, ida e volta mundo↔célula (quadrada em 121 células, hexagonal em 169), caminho mais curto e contínuo com e sem parede, o **mesmo** caminho em duas chamadas, `none` para começo/destino bloqueado |
| `grid.square`, `grid.hex`, `grid.distance`, `grid.neighbors`, `grid.range`, `grid.path`, `grid.to_world`, `grid.from_world` na tabela única do `api.rs` e nos nativos | 3 testes com script real: `distancia 3`, `vizinhas 6`, `alcance 19`, desvio de uma célula bloqueada em 4 passos, `sem caminho true`, célula↔metros (`celula 2 -1`), e recusa tratada com `attempt`/`failed` sem virar falha do script |
| **Limites antes da conta**: coordenada ±1.000.000, célula de 0,001 a 10.000 m, raio 256, 65.536 bloqueadas, 65.536 expandidas (erro, não travamento), ponto não finito recusado | 1 teste hostil com todas essas fronteiras, inclusive o estouro do orçamento de A\* numa busca sem saída |

**Limites declarados:** a grade e a célula são **dado** transformável (`{"kind","size"}` e `{"q","r"}`); não há tabela `Grid` na cena nem visão da grade no viewport. `grid.path` não tem custo por célula (lama, água) e a grade não tem andares.

**Não verificado:** a janela (não há interface de grade ainda — a API só existe para scripts) e um mapa real grande: os testes usam raios de até 5 e uma busca de 400 células.

## Antes: painel de assets (2026-11-09)

A aba **Assets** fica ao lado de **Hierarquia** e mostra os modelos da pasta `assets/` do projeto. Antes só existia o comando `asset.list`; agora a pasta aparece na janela e o modelo entra na cena com um duplo clique.

| Entrega | Evidência |
|---|---|
| Aba **Assets**: nome e tamanho de cada `.glb`/`.gltf` (até 4 níveis de pasta); com a pasta vazia, explica o que fazer; o botão **Importar modelo…** abre o mesmo seletor de *Arquivo > Importar modelo 3D…* | visto no display virtual |
| **Duplo clique** importa um modelo que já está em `assets/` como malhas editáveis, sem copiá-lo de novo; importar de fora continua copiando (nome repetido vira `-2`) e é **um** desfazer | triângulo de teste (744 bytes): 12 objetos, `Mesh` com 3 vértices e 1 face, cor `#D97333`; **um** `Ctrl+Z` voltou a 11; cena salva com a tabela `Mesh` (entidade 13) |
| A lista se atualiza ao abrir a aba e depois de importar (um arquivo copiado para `assets/` com o editor aberto aparece) | visto: o arquivo criado depois de abrir o editor apareceu na aba; o segundo import listou `triangulo-2.glb` |

**Não verificado:** a janela na sua sessão real; leitor de tela; importar um modelo real de um programa de modelagem (só o triângulo montado à mão).

## Antes: cortar, arranjar e espelhar brushes (2026-11-08)

Fecha o que faltava do **nível 2** da [edição de brushes](../02-arquitetura/13-primitivas-e-edicao-de-malha.md#nível-2-edição-de-brushes-feito-em-2026-11-08), sem trazer edição de malha para o caminho. Todas são **operações de dados** (comando + registro no histórico), não código novo de interface além das caixas de diálogo.

| Entrega | Evidência |
|---|---|
| **Cortar com plano** (`brush.cut_plane`): divide a caixa em duas com um plano perpendicular a um eixo **dela**; as metades continuam caixas, com a mesma rotação, escala e cor; volume preservado | teste com caixa girada 35°, volume igual antes e depois; corte fora da caixa recusado com `EDITOR_CUT_PLANE_OUTSIDE` |
| **Arranjo em linha** (`brush.arrange_line`): `count` de 2 a 64 contando o original e passo `{x,y,z}` | teste: 4 itens criam 3 cópias a 2 m, tudo em **um** desfazer |
| **Arranjo em círculo** (`brush.arrange_circle`): centro explícito, eixo, cópias giram junto com o círculo | teste: 4 itens num anel de raio 5 m com giros de 45°, 135°, 225° e 315° |
| **Espelhar** (`brush.mirror`): plano de eixo do mundo, cópia por padrão ou mover no lugar; rotação refletida (`M·R·M`) | teste: cópia em `x = 195` com giro `-30°` a partir de `x = 205`/`+30°`; mover no lugar e recusa para cilindro |
| Editor Qt: **Operações > Cortar com plano… / Arranjo em linha… / Arranjo em círculo… / Espelhar…**, ligadas só com seleção | `scripts/editor.sh build` ok |
| Atalhos | `Ctrl+Alt+P`, `Ctrl+Alt+L`, `Ctrl+Alt+O`, `Ctrl+Alt+M` |
| **Visto no display virtual (2026-11-08):** cortar a caixa de 1 m em duas de 0,5 m com giro 25°; linha com cópias a 1 m; círculo de 8 itens em passos de 45°; espelho em X=0 com giro `-25°`; **um** `Ctrl+Z` desfez cada operação inteira (as 7 cópias do círculo caíram juntas) | imagens da janela e cena salva conferida (`room.scene.json`: posições, giros e tamanhos) |
| Testes do motor | `cargo test -p petunia_editor brush_ops -- --test-threads=2`: **11 passaram, 0 falharam** |

**Limites declarados:** as quatro operações aceitam **caixas**; cilindros, escadas e malhas recebem recusa com motivo. O arranjo não copia colisores, scripts nem traços; o espelho com cópia copia. Não há prévia ao vivo: o resultado aparece ao confirmar.

**Não verificado:** a janela na sua sessão real; leitor de tela; cilindros/escadas/malhas nessas operações (recusados por projeto).

**Próximo da lista:** material por face. (O `grid`/A\\* foi feito em 2026-11-09, acima; o Event Sheet foi descartado pelo DR-30, que o trocou pelo sistema de nós; os prefabs do próprio projeto foram feitos em 2026-11-10, no topo; as alças no viewport para redimensionar colisões foram feitas em 2026-11-08, abaixo.)

## Antes: rios, estradas, água e terreno no editor (2026-10-29)
## Etapa atual: valor ao vivo e faca por clique (2026-10-30, mesma rodada)

Decisão: [`DR-48`](../06-decisoes/registro-de-decisoes.md). Fecha os ganchos de interface que a
[edição de malha](../02-arquitetura/13-primitivas-e-edicao-de-malha.md#ganchos-da-interface)
declarava — valor ao vivo, faca por clique e as contagens da malha no inspetor. Nada mudou no
motor: `mesh.preview`, `mesh.preview_end`, `mesh.pick`, `mesh.knife` e `mesh.info` já existiam e
tinham teste; o que faltava era o gesto e o resumo na janela.

| Entrega | Evidência |
|---|---|
| **Valor ao vivo:** segurar `I`, `Ctrl+E`, `Ctrl+B` ou `Ctrl+R` mostra a operação e o mouse muda o valor (1 cm por pixel, 0,01 a 10 m; no corte em laço, 40 px por corte); soltar a tecla, clicar ou `Enter` aplica como **um** desfazer; `Esc` cancela | na janela, cada uma das quatro: `I` segurado e arrastado 100 px aplicou **um** inset de **1,20 m** (a barra tinha 0,20 m); `Ctrl+E` deu **10 faces e 12 vértices**; `Ctrl+B` deu **7 faces e 10 vértices**; `Ctrl+R` deu **14 faces e 16 vértices**. `Esc` no meio da prévia deixou a malha **idêntica** (6 faces, 8 vértices) com o rodapé "Prévia cancelada; a malha não mudou." |
| **Uma batida continua uma batida:** a prévia começa com o valor da barra, então soltar sem mover aplica o mesmo valor de antes | o gesto de prévia é o mesmo caminho; a confirmação espera 120 ms para não confundir a repetição automática do teclado (o X11 manda pares pressionar/soltar) |
| **Faca por clique:** `K` arma a faca; cada clique escolhe o ponto na aresta mais próxima (`mesh.pick` dá aresta e fração); o segundo clique corta; `Esc` cancela | na janela: `K` + dois cliques numa face cortaram a malha de **6 para 7 faces** e de **8 para 10 vértices**, e o arquivo da cena guardou o corte. Um clique em arestas que não dividem a mesma face mostra o erro do motor no rodapé e não muda a malha |
| **Faca por números** continua: **Malha ▸ Faca por números…** | o diálogo antigo foi mantido (caminho sem mouse e única opção com o ponteiro fora do viewport) |
| **Foco no viewport ao entrar no modo de edição** (`Tab`) | sem isso, escolher o objeto na Hierarquia e apertar `K` não fazia nada: os atalhos de malha valem só com o foco no viewport, e é assim que deve ser para não roubar letras da edição de código |
| **`Enter` e sair do viewport também aplicam** o valor mostrado | `Enter` aplica sem soltar a tecla; trocar o foco (escolher outro objeto na Hierarquia) aplica, para a prévia nunca ficar presa bloqueando o salvar |
| **Contagens no inspetor:** a seção `Mesh` mostra `8 vértices · 12 arestas · 6 faces` em vez de "malha editável" | medido na janela com um corte de faca: a seção passou de `8 vértices · 12 arestas · 6 faces` para `10 vértices · 15 arestas · 7 faces` (os mesmos números do arquivo salvo) |
| **Corte em laço ao vivo** (`Ctrl+R`): a tecla abre a prévia, o arrasto muda o número de cortes (1 a 32) e o campo **Cortes** acompanha | na janela: `Ctrl+R` segurado e arrastado cortou a caixa de **6 para 14 faces** (16 vértices) e **um** `Ctrl+Z` voltou para 6 faces e 8 vértices |
| **Ajuda ▸ Atalhos** e a documentação do editor ganharam os gestos novos | texto do diálogo e `docs/02-arquitetura/11-editor.md` + `13-primitivas-e-edicao-de-malha.md` |

**Gates medidos agora (fim da rodada):** `cargo test --workspace --no-default-features -j 2` →
**543 testes, 0 falhas**; clippy e fmt por pacote **limpos**; editor Qt compila; `editor-smoke.sh`
**12/12**; `natural-smoke.sh` **16/16** (medidos na build final).

**Não verificado:** a janela na **sua sessão real**; leitor de tela; mesa digitalizadora; o botão
**Faca por números…** pela janela (o caminho é o mesmo diálogo de antes, compilado).

## Etapa atual: várias cenas, salvamento automático e painel de assets (2026-10-30)

Decisão: [`DR-47`](../06-decisoes/registro-de-decisoes.md). Fecha os dois limites que a tabela do
editor declarava desde a primeira versão — **"só a cena padrão do projeto abre"** e **"sem salvamento
automático"** — e o painel de assets que faltava. A janela foi exercitada no **display virtual**;
nada apareceu na sua tela.

| Entrega | Evidência |
|---|---|
| **Várias cenas:** `project.scenes` (lista), `project.open_scene` (troca sem fechar o projeto) e `project.new_scene` (cena vazia; nome ocupado é recusado) | 2 testes de fluxo no `petunia_editor`: criar/listar/trocar, cena inexistente recusada com a aberta intacta, nome ocupado recusado, desfazer não atravessa cenas; 3 testes do `petunia_project` (lista só de `*.scene.json`, ordenada e sem seguir link; abrir cena com e sem entidades; caminho fora do projeto recusado) |
| **Troca de cena** substitui o mundo, a seleção e o **histórico de desfazer**; manifesto, scripts, `assets/` e câmera ficam | o teste confere as entidades depois de voltar à cena original e que `edit.undo` responde `false` na cena nova |
| **Salvamento automático** a cada 90 s, só com mudanças e fora de arrasto, em `scenes/<nome>.scene.autosave.json` (gravação atômica; **nunca** escreve a cena) | 1 teste de fluxo: sem mudança não grava; com caixa grava e a cena no disco continua sem a caixa; sessão nova vê `newer: true`; `project.recover` traz a caixa e deixa a sessão **suja**; `Ctrl+S` grava e **apaga** a cópia; descartar sem cópia responde `removed: false` |
| **Recuperação oferecida, nunca automática:** a janela pergunta **Recuperar / Descartar / Agora não**; a linha de comando e os testes nunca abrem diálogo | **visto no display virtual:** abrir o projeto pelo menu fez a pergunta "O editor guardou trabalho não salvo desta cena em 30/10/2026 às 01:51…"; **Recuperar** trocou o mundo (a Hierarquia passou a mostrar `Recuperado`), o título ficou `minha_sala · room*` e o rodapé disse "Trabalho recuperado. Salve (Ctrl+S) para manter no arquivo da cena."; a cena no disco não mudou |
| **Painel de assets** (aba embaixo, ao lado de Diagnósticos e Histórico): lista `assets/`, **Importar…**, **Inserir na cena** e **Atualizar**; clique duplo insere | visto no display virtual: a lista mostrou `tri.glb — 672 B`, o clique duplo criou **Triangulo #29** com tabela `Mesh`, o inspetor abriu nele e o rodapé disse "1 objeto(s) do modelo tri.glb entraram na cena — Ctrl+Z desfaz" |
| **O título da janela mostra a cena aberta** (`projeto · cena`), não só a cena padrão do manifesto; a barra de cima mostra o projeto | visto: `minha_sala · room*` antes de salvar, `minha_sala · outra` depois de trocar de cena |
| **Documentação corrigida:** o editor dizia que "não há botão Jogar" (há, `F6`) e que a edição de malha "não tem botões nem atalhos" (tem barra, menu e atalhos desde 2026-10-27/29) | `docs/02-arquitetura/11-editor.md` e `13-primitivas-e-edicao-de-malha.md` atualizados com o que falta de verdade: **valor ao vivo**, **faca por clique** e as **contagens no inspetor** |

**Como a janela foi verificada (2026-10-30, displays virtuais `:94` e `:95`):** um projeto de teste foi aberto
com `--project` (sem diálogo, como a linha de comando faz), uma caixa foi criada com `Ctrl+1`
(título ficou `*`), a cópia automática apareceu depois do intervalo de 90 s e **sumiu** depois do
`Ctrl+S` (a cena salva tinha a caixa); o menu **Arquivo ▸ Abrir outra cena…** trocou para
`outra.scene.json` e o título mudou para `minha_sala · outra`; a aba **Assets** listou o modelo e o
clique duplo o inseriu na cena; o log do editor ficou **vazio** (nenhum aviso de Qt, nenhum
`nullptr`). O roteiro foi um script descartável em `/tmp`, não um gate permanente: o que fica
automatizado são os testes Rust acima e os dois testes de fumaça.

**Gates medidos agora:** `cargo test --workspace --no-default-features -j 2` → **543 testes, 0
falhas** (eram 536; +7 desta etapa); `cargo clippy --workspace --all-targets --no-default-features
-- -D warnings` → **limpo**; `cargo fmt` por pacote → **limpo**; editor Qt compila (`Qt 6.4.2`);
`scripts/editor-smoke.sh` → **12/12**; `scripts/natural-smoke.sh` → **16/16**.

**Uma rodada do `editor-smoke.sh` morreu com SIGSEGV no encerramento (2026-10-30).** Os 12 passos
já tinham rodado; a rodada seguinte, sozinha, passou **12/12** e o log ficou limpo. Não se repetiu
em duas rodadas seguintes e não havia núcleo acessível (`ulimit -c` é 0), então a causa não foi
isolada; a regra de rodar **um teste de janela por vez** continua valendo.

**Não verificado:** a janela na **sua sessão real**; leitor de tela; os botões **Descartar** e
**Agora não** do diálogo de recuperação (só o **Recuperar** foi clicado); abrir uma cena **enquanto
joga** (o despacho recusa, sem teste de janela); nome de cena com acentos (o saneamento aceita letras
acentuadas; só a unidade cobre).

**Continua faltando no editor (declarado):** material por face/UV (exige mudança de schema),
valor ao vivo da malha e faca por clique, miniatura no autosave, duplicar/renomear cenas pela
janela, workspaces **Pintura**, **Animação** e **Efeitos** (`DR-24`) e UI de jogo (`DR-17`).

## Etapa atual: rios, estradas, água e terreno no editor (2026-10-29)
## Etapa atual: 2D como modo da mesma engine (2026-10-29)

Decisões: [DR-49](../06-decisoes/registro-de-decisoes.md) e [DR-50](../06-decisoes/registro-de-decisoes.md). O 2D **não** é um motor separado: é a mesma cena, com câmera ortográfica e sprites. A folha de sprites também: uma imagem cortada em quadros, com o quadro escolhido pelo relógio do anfitrião.

| Entrega | Evidência |
|---|---|
| **Tabela `Sprite`** (`asset` Text, `size` Vec3 com padrão 1×1×0, `color`, `layer` Int, `flip_x`, `flip_y`) em entidade comum | 5 testes de quadro do `petunia_render` (ordem `(layer, entity)`, tamanho e espelhamento, sprite sem tamanho ignorado, pixels ligados por alça, seleção marca o sprite) |
| **PNG como entrada hostil:** `decode_png` com limite de 32 MiB de arquivo, 4096 por lado e 8 Mi pixels, sempre RGBA8 e linha de cima primeiro; `read_png_file` confere o tamanho **antes** de ler | 7 testes do `petunia_import` (RGBA byte a byte, RGB/cinza opacos, alfa preservado, arquivo que não é PNG, largura acima do limite, arquivo acima do limite, leitura do disco com ausente e vazio) |
| **Pixels viajam no quadro:** `RenderFrame.textures` + `resolve_textures`; o `petunia_render` continua sem OpenGL | 7 testes de `TextureStore`/`TextureLoader` (alça estável por asset, tamanho errado recusado, asset lido **uma** vez e a falha lembrada, sprite sem asset não pede nada, limpar recomeça as alças) |
| **Caminho do asset em um só lugar:** `resolve_inside` + `read_png_file` = `petunia_project::load_png_asset`; editor e `petunia run` usam a mesma função | 1 teste do `petunia_project` (lê `assets/…` e recusa caminho fora do projeto) |
| **Passe de sprites no OpenGL:** quad unitário com textura, mistura alfa, sem descarte de faces e sem escrita de profundidade, textura branca quando falta imagem, cache de 256, estado devolvido antes do overlay | `scripts/sprite-smoke.sh` **4/4** e **a captura foi olhada**: um sprite de borda transparente aparece sobre uma caixa 3D verde; o meio é `(0, 255, 255)` e a borda mostra a caixa `(36, 95, 38)` |
| **2D com as ferramentas do 3D:** clique e passar o mouse usam `pick_sprite` (mesmo teste de raio do 3D); a regra de andares vale para sprites (esconder/transparente), pela meia diagonal do quad | 1 teste de domínio (`pick_sprite`), 1 de fluxo no editor (clique e `pointer.hover` acham o sprite) e 1 de fluxo de andares (sprite acima do andar fica transparente, sai do alcance do mouse, some no modo Esconder e **nunca** some quando selecionado) |
| **Inspeção:** `editor.snapshot` informa `textures: {loaded, failed}` | teste do editor que cria a linha `Sprite`, confere largura/altura, camada e os pixels no quadro, e lê `loaded = 1` |
| **Criar um sprite pela janela:** `asset.import_sprite` copia o `.png` para `assets/` (nome livre, nunca sobrescreve), mantém a proporção da imagem (o lado maior mede 2), apoia a base no andar atual, vira **um** objeto com a linha `Sprite` e já fica selecionado — um passo de desfazer. `asset.list` passa a listar imagens (`kind: image`) e a hierarquia mostra a linha `Sprite` como **Imagem 2D**, com ícone próprio | 1 teste de fluxo no editor (cópia em `assets/dragao.png`, 4×2 pixels viram tamanho 2×1, +1 objeto, seleção, sprite no quadro **com** textura, `ASSET_INVALID`/`ASSET_UNSUPPORTED`/`ASSET_IO`, reimportar pelo caminho do projeto não duplica o arquivo, dois desfazeres) |
| **Menus** **Arquivo > Importar imagem 2D…** e **Criar > Importar imagem 2D…** | o editor Qt **compila** com a ação, o ícone e o filtro `*.png`; **não abri o seletor de arquivos** |
| **Folha de sprites e animação por quadro:** tabela `SpriteAnimation` (`frames`, `columns`, `fps`, `loop`, `playing`, `frame`) na **mesma** entidade; o quadro é função pura do relógio (`frame_at`), então nada é gravado na cena e dois sprites podem tocar a mesma folha em tempos diferentes. O `petunia_render_gl` recebe só um retângulo (`u_rect`): uma folha é uma textura, nenhum atlas e nenhum draw call a mais. O editor **congela** em edição e o modo **Jogar** anima; o `petunia run` anima sempre | 5 testes de domínio no `petunia_render` (laço e travamento no último quadro, parado mostra `frame`, `fps`/`seconds` hostis, grade 3×2 do recorte e espelho dentro do quadro, linha `SpriteAnimation` → quadro e `uv` no quadro extraído) e 1 de fluxo no editor (edição mostra `frame = 1`, três passos de 0,1 s a 8 fps mostram o quadro 2 e parar volta ao 1) |
| **Prova visual do 2D:** `scripts/sprite-smoke.sh` monta um projeto (caixa 3D + sprite de uma folha 2×1: magenta | ciano) e lê quatro pixels da captura do viewport | **4/4**: meio `(0, 255, 255)` e um quarto à esquerda também ciano (a folha escolheu o quadro 1; o quadro 0 seria magenta), borda transparente `(36, 95, 38)` e fora do quadrado `(36, 95, 37)`. A mesma rodada mostrou a hierarquia com o objeto `Folha` e a linha `Sprite` como **Imagem 2D** |
| **Janela (rodada de 2026-10-29, repetida depois do DR-50):** `scripts/editor-smoke.sh` e `scripts/natural-smoke.sh` | **11/11** e **16/16** no display virtual, depois de acertar a contagem de itens do menu **Criar** (o item novo entrou antes de *Caverna*) |

**Fora desta etapa (declarado):** animação por **estado** (máquina de estados, “parado/andando/pulando”), atlas com vários PNGs, tiles/autotile, física 2D, HUD, texto, ordem por y, luz 2D, encaixe em pixel e limites de câmera.

**Não verificado:** o seletor de arquivos da imagem 2D, **a animação correndo numa janela** (o relógio do modo Jogar e o do `petunia run` existem e são testados, mas nenhuma captura mostra dois quadros diferentes no tempo), a janela na **sessão real** (todas as capturas são de display virtual com OpenGL por software) e o desempenho com muitas centenas de sprites.

## Antes: rios, estradas, água e terreno no editor (2026-10-29)

Decisão: [DR-36](../06-decisoes/registro-de-decisoes.md) e o complemento dela ([DR-37](../06-decisoes/registro-de-decisoes.md)). A interface existe (barra do terreno/rio/estrada no viewport) e o teste de fumaça a exercita no display virtual; nada foi visto na sua sessão real.

| Entrega | Evidência |
|---|---|
| Spline Bézier com amostragem por comprimento de arco, quadros com parâmetro e largura/profundidade por ponto | testes do `petunia_math` (espaçamento uniforme em curva desigual, quadros ortonormais, curva fechada sem costura duplicada) |
| Perfis varridos de rio/estrada, fluxo e máscara de margem nos vértices, orçamento e recusas | testes do `petunia_sculpt` (malha, largura por ponto, perfil vazio) |
| Mapa de altura em blocos, pincel com falloff, aplainamento e continuidade entre blocos | testes do `petunia_world` e do `petunia_sculpt` (caminho cruza a borda do bloco) |
| Rios: validação de descida, baixar leito, confluências, lago, queda, volume de água | testes do `petunia_sculpt` |
| Estradas: interseção, divisão por de Casteljau, polígono de cruzamento, aviso de ângulo agudo, ponte/bueiro por diferença de altura | testes do `petunia_sculpt` |
| Navegação: grafo por arestas, pontas soltas, ilhas, arestas sem dois extremos | 4 testes do `petunia_graph` |
| **Comandos e consultas:** `terrain.create`, `terrain.paint`, `path.create`, `path.set`, `road.detect_structures`; `terrain.sample`, `path.report` | testes do `petunia_editor` pelo JSON, com desfazer, refazer e gravação na cena |
| **Gravação na cena:** `Terrain`, `TerrainChunk`, `River`, `Road` e `Bridge` salvam e reabrem | teste que salva, fecha e reabre o projeto |
| **Pincel de erosão:** uma passada de erosão térmica (cada célula cede material ao vizinho mais baixo, com falloff e cópia do início) | 3 testes do `petunia_world` + 1 de fluxo no editor (`terrain.mode` e `terrain.paint` com `erode`) |
| **Pincel de ruído:** ruído de valor e fractal próprio (`petunia_math::noise`, retículo de 1,5 m, 3 oitavas) somado com falloff e **centrado na média**, então o carimbo detalha sem mudar o volume | 5 testes do `petunia_math`, 3 do `petunia_world` (detalhe, mesma semente, segunda pincelada aprofunda, entrada hostil) + 1 de fluxo no editor (`noise`) |
| **Passo por teclado (acessibilidade):** `terrain.step` aplica **um** carimbo com o modo, o raio e a força da barra, sem arrastar — a barra de espaços no viewport; `times` repete no mesmo ponto e tudo continua sendo **um** desfazer. Sem ponteiro sobre o viewport o carimbo cai no centro da tela | 1 teste de fluxo no `petunia_editor` (três passos somam altura, `mode`/`radius`/`strength` e `height` na resposta, **um** desfazer devolve o terreno plano) + 2 verificações no teste de fumaça (subiu e o desfazer devolveu) |
| **Carimbos de terreno:** formas nomeadas (`crater`, `mound`, `plateau`, `ridge`) em `petunia_world::stamp`, cada uma uma função fechada do afastamento dentro do disco e **zero na borda**; `terrain.paint` aceita `mode: stamp` com `stamp: <forma>` e o combo **Forma** aparece no modo Carimbo | 7 testes do `stamp` (nomes, limites e borda, cratera, monte, platô, crista, entrada hostil) + 4 do `heightfield` (cratera com meio fundo e borda alta, monte que não afunda, mesma forma dá o mesmo terreno, entrada hostil) + 1 de fluxo no editor (cratera com meio e borda, **um** desfazer) + 3 verificações no teste de fumaça (cratera cavou, borda subiu, monte só subiu) |
| **Giro do carimbo (`DR-37`):** `rotation` em graus por carimbo, girado antes de avaliar a forma; campo **Girar** ao lado de **Forma** (15° por seta, `Ctrl` = 1°), e o passo por teclado lê o mesmo ângulo | 2 testes novos do `stamp` (a crista vira 90° e troca os eixos; 0°/360° iguais e ângulo hostil ignorado) + 1 do `heightfield` (crista girada levanta em z sem mudar o pico) + 1 de fluxo no editor (duas medidas com um desfazer entre elas) + 2 verificações no teste de fumaça (a mesma crista reta e girada 90°) |
| **Desenho:** terreno com LOD e saia, água, estrada com acostamento, defensas, **convés de ponte** e fluxo/espuma no sombreador | testes do `petunia_render` (convés abaixo da estrada, faces de baixo não iluminadas como céu) + teste do editor que acha o convés no quadro entregue à janela |
| **Ponte e bueiro:** a ponte ganha convés e defensas; o bueiro/corte fica só como dado (o terreno é a estrutura) | teste do editor com estrada acima do terreno (ponte) e dentro do corte (bueiro) |
| **Física:** cada bloco de terreno entra como malha sólida, então o leito do rio é chão andável; a água continua meio (empuxo, nado, correnteza) | 2 testes do `petunia_physics` (bloco andável, bloco quebrado reportado e deixado de fora) |
| Editor Qt | compila (`scripts/editor.sh build`); `scripts/natural-smoke.sh` **16/16** no display virtual: cria o terreno, pinta alturas, **erode** e aplica **ruído** (cada um muda o material e conserva o volume), aplica **um passo por teclado** e **desfaz** (o terreno volta ao medido), grava um **carimbo de cratera** (meio afundou, borda subiu) e um **de monte** (só subiu), carimba a **mesma crista reta e girada 90°** (a medida longa troca de eixo), cria rio e estrada, cava sob a estrada e **detecta a ponte** (entidade `Ponte`, vão 0..2) |
| Merge na `main` (2026-10-29): `feat/mundo-natural-rios-estradas` mesclada; as decisões daquela linha renumeradas para **`DR-36`/`DR-37`** porque a `main` já usava `DR-33`, `DR-34` e `DR-35` | conflitos resolvidos guardando os dois lados (verificação de `PlayerStart` da `main` **e** as checagens de terreno/caminho; `_virtual-display.sh` **e** a limpeza de saídas antigas); 535 testes depois do merge |
| Editor Qt na `main` mesclada | `scripts/editor-smoke.sh` **12/12** (122 s) e `scripts/natural-smoke.sh` **16/16** (77 s), no display virtual |
| Workspace inteiro | **536 testes, 0 falhas** (`main` com esta linha mesclada, 2026-10-29, 2 threads); `cargo clippy --workspace --all-targets -- -D warnings` ok; `cargo fmt` ok **por pacote** (ver a nota do `fmt` abaixo); `cargo deny` **não rodou** (ferramenta não instalada) e nenhuma dependência nova entrou |

**Corrigido ao ligar o teste de fumaça à janela (2026-10-28):** a barra de opções mostrava todos os controles de uma vez, não cabia na largura da janela e o botão **Pontes e bueiros** caía fora da área visível (o canto `⋯` da barra não alcança widgets, só ações); e a checagem de "há estrada selecionada?" comparava `selected_road` — que é o **número** do id — com booleano, então o botão e a caixa **Defensas** nunca habilitavam. Agora cada ferramenta mostra só os seus controles e o teste é `isDouble()`.

**Outro teste de janela:** `scripts/editor-smoke.sh` voltou a **11/11** depois de acertar quantas teclas `Down` escolhem `PlayerStart` no menu **Adicionar tabela**. O menu lista as tabelas que faltam em ordem alfabética e o branch trouxe `Bridge`, `River`, `Road`, `Terrain` e `TerrainChunk` — todos antes de `PlayerStart`, que virou o **sexto** item. O teste reprovava antes de qualquer mudança de produto.

**Uma reprovação intermitente no `editor-smoke.sh` (2026-10-29):** a checagem da malha editável (Tab) reprovou na primeira de duas execuções seguidas, sem nenhuma mudança no caminho do Tab — a segunda passou **11/11**. O passo clica a caixa em (990, 431) depois de andar e girar a câmera; um clique que não acerta a caixa deixa o Tab sem o que converter. Se voltar a acontecer, esse clique tem de seguir a posição da caixa em vez de um pixel fixo.

**Duas falhas de janela com causa comum, consertadas no merge (2026-10-29):** na `main` mesclada o `editor-smoke.sh` reprovou tudo numa rodada (o clique na caixa grande não selecionava nada) e depois morreu com `rc=139`, e o `natural-smoke.sh` parou com "a cena não foi regravada". Não era o produto: (1) o teste dormia **3 s** fixos e clicava antes de a janela existir — logo depois de uma compilação o primeiro quadro em OpenGL por software demora mais; (2) o `--screenshot-delay 62000` é também o **tempo total da rodada** (o editor tira as fotos e sai ali), e os passos passaram a levar ~65 s, então o editor fechava no meio, às vezes durante um evento em voo (daí o segfault). Agora `_virtual-display.sh` tem `wait_for_window`, que espera a janela aparecer (e aborta com o log se o editor morrer antes), o timer subiu para 120 s e `PETUNIA_SMOKE_KEEP=1` guarda a pasta da rodada para pós-mortem. Medido depois disso: **11/11** e **16/16**.

**Pasta de build da janela (2026-10-29):** a pasta do CMake se chamava `/tmp/petunia-editor-build` e estava configurada para o checkout irmão `petunia-engine-mundo`. O CMake guarda a pasta de origem de quem configurou primeiro, então `scripts/editor.sh build` compilava o editor **daquele** checkout e o teste de fumaça rodava um binário com telas que não existem aqui (o status bar dizia `TERRAIN_NO_HIT`, texto que não está neste código). Agora cada cópia de trabalho tem a sua pasta (`/tmp/petunia-editor-<nome do checkout>`), `scripts/editor.sh` recusa uma pasta configurada para outra origem e os dois testes de fumaça derivam o mesmo nome, sem `PETUNIA_EDITOR_BIN` no comando.

**Tela virtual reaproveitada por um teste de fumaça (2026-10-29):** uma execução que morre no meio pode deixar o display `:98` ocupado e um **editor daquela execução** em cima do novo. Todo clique e tecla vão para a janela errada e o script continua "rodando": as pinceladas caíram em outro projeto, o arquivo da cena nunca foi escrito e as verificações começaram a reprovar como se a **erosão**, o **ruído** e o **passo** estivessem quebrados — o que me custou uma hora de diagnóstico errado. `scripts/_virtual-display.sh` (compartilhado pelos dois testes) escolhe um display livre (e recusa, em vez de mudar, quando `PETUNIA_EDITOR_DISPLAY` foi fixado) e aborta na hora se o editor morre ao abrir, mostrando o log dele.

**Salvamento esperado, não adivinhado (2026-10-29):** o teste de fumaça fazia `ctrl+s` e copiava o arquivo da cena depois de um `sleep` fixo. A janela entrega o salvamento ao motor e o arquivo aparece um instante depois, então o `sleep` curto demais copiava o estado da ação **anterior**: cada verificação comparava o par errado de arquivos e quatro checks (erosão, passo, desfazer e monte) reprovaram sobre um produto que estava certo. A função `save_scene` do `scripts/natural-smoke.sh` anota o `mtime` antes do `ctrl+s`, espera até 6 s a cena ser regravada e **para o teste** com mensagem se o arquivo não mudar; nenhuma verificação recebe mais arquivo velho.

**Navegação do combo por posição (2026-10-29):** o mesmo teste escolhia o modo com `End` (último item) e `End`+`Up` (penúltimo). Ao entrar **Carimbo** no fim da lista, os dois atalhos passaram a apontar para outros modos e a erosão virou carimbo sem nenhum aviso. Agora um auxiliar `pick_mode` conta `Home` + N × `Down`, e a ordem dos modos está escrita ao lado para conferência.

**`editor-smoke.sh` quebrado por um comentário (2026-10-30):** o comentário sobre o
`--screenshot-delay` foi inserido **entre** as linhas de continuação (`\`) e o comando `env`.
A barra invertida deixou de continuar a linha, o `env` passou a imprimir o ambiente e a **nunca
iniciar o editor**, e o teste morria em `a janela "Petunia Editor" não apareceu em 25 s` — sem
que o produto tivesse defeito (aberto à mão, o editor mostra a janela em ~1 s). O comentário
subiu para cima do `env` e o arquivo tem agora um aviso para não repetir. Depois da correção:
`editor-smoke.sh` **11/11** e `natural-smoke.sh` **16/16**, no display virtual.

**`Aplicar código`: promessa retirada (2026-10-30, feito):** o painel Lógica dizia ao usuário
*"edite e use \"Aplicar código\" para voltar ao grafo"*, mas o botão **nunca foi criado**
(`applyCodeButton_` era só declarado e ligado a `GraphPanel::applyCode`; nada o instanciava) e o campo
de código é `setReadOnly(true)`. O `QObject::connect(...invalid nullptr parameter)` do log era esse
`connect`. O `DR-40` tornou a ida e volta desnecessária — um projeto usa **um** método —, então a
frase, o `connect` órfão, o slot `applyCode` e o membro `applyCodeButton_` foram **removidos** (não
bastava consertar o botão: o fluxo deixou de existir). O texto do painel agora diz só o que é verdade:
*"Só leitura: editar o grafo muda este texto, e editar o texto não muda o grafo."* O comando
`graph.from_code` continua no protocolo, sem porta na janela. **Verificado:** o editor compila sem o
aviso de `nullptr` e o log da janela no display virtual não traz mais nenhuma linha de `QObject::connect`.

**Um método de programação por projeto (`DR-40`, 2026-10-30, implementado):** o manifesto ganhou
`programming_method` (`aipo` | `luau` | `nodes`); arquivos antigos com `scripting_language` continuam
abrindo (`serde(alias)`) e quem não tem o campo assume `aipo`. A janela **esconde a aba do outro
método** (Cena · Código em Aipo/Luau; Cena · Lógica em pNodes) e `F4`/`F7` não abrem a aba oculta. O
projeto novo pergunta o método ao criar (`--method` também na linha de comando). **Prefabs** seguem o
método: em `aipo` compilam o grafo para `<nome>.aipo`, em `nodes` gravam `<nome>.graph.json`; um
arquivo existente de qualquer extensão nunca é sobrescrito. O Project Doctor avisa quando os dois
tipos convivem em `scripts/` (`project.method_script` / `project.method_graph`); e o **carregador
passou a obedecer ao método** (`ScriptHost::load_directory`, `PlayerConfig` e o Jogar do editor), sem
o que o aviso do Doctor seria falso — ver a correção no `DR-40`. Evidência: **7 testes novos**
(5 de manifesto com a migração do campo antigo, 1 de prefab por método, 1 de carregamento por
método), dois quadros do display virtual (Cena · Código num projeto Aipo; Cena · Lógica num pNodes)
e uma execução manual num projeto real: `metodo=aipo carregados=[]`, `metodo=nodes
carregados=["sobra"]`.

**`../aipo-lang` é pré-requisito de build, e faltava aqui (2026-10-30):** `petunia_script`
depende de `../../../aipo-lang/crates/...` e o build desta cópia de trabalho falhava antes de
compilar qualquer coisa (`failed to load manifest ... /aipo-lang/crates/aipo-ast/Cargo.toml`).
Um link simbólico `aipo-lang` ao lado do checkout resolveu nesta sessão e **não** é versionado,
então quem clonar de novo repete o passo. O `docs/dependencies.md` diz que, para distribuir, isso
vira dependência git fixada por commit; enquanto for de caminho, o primeiro build numa cópia nova
falha com uma mensagem que não diz que o problema é a pasta irmã.

**Ainda falta (declarado):** decals de faixa (bloqueado: `DR-12` é só proposta), LOD em quadtree, tubo visível de bueiro e partículas na queda.

## Auditoria de 2026-10-29 (implementação × documentação)

Auditoria feita sobre a `main` @ `84a44ad`, com os gates rodados de verdade nesta máquina (não lidos de
registros antigos). Os números da tabela "O que existe" e o estado abaixo foram **reconferidos**: batem
com o código (`63.428` linhas de Rust em `src/`, `14.100` de legado em `petunia_mesh`, `10.918` linhas
próprias de C++, `3.589` copiadas no `ui_kit`).

**Gates medidos agora:** `cargo test --workspace --no-default-features -j 2` → **536 testes, 0 falhas**;
`cargo clippy --workspace --all-targets -- -D warnings` → **limpo**; `cargo fmt` por pacote → **limpo**
(`--all` continua tocando a dependência `../aipo-lang`); `cargo deny` / `cargo nextest` **não rodaram**
(não instalados); `cargo xtask …` **não existe** ainda. Editor Qt compila no Qt 6.4.2 e os dois testes de
fumaça passam no display virtual: `editor-smoke.sh` **12/12** e `natural-smoke.sh` **16/16**.

### Defeitos encontrados e corrigidos nesta auditoria

| Defeito | Como apareceu | Correção |
|---|---|---|
| **O teste de fumaça abria o editor na sessão real do usuário** | Reproduzido: um comentário bash estava **dentro** da continuação com `\`, então a linha lógica terminava no comentário; `env` rodava sozinho e o editor começava como comando separado, herdando o `DISPLAY` real. Ele abriu na tela do usuário e regravou `~/.config/Petunia/Petunia Editor.conf` (tema, geometria de janela). O teste então falhava com "a janela não apareceu", porque procurava a janela no display virtual | Comentário movido para fora da continuação; `require_virtual_display` novo em `_virtual-display.sh` lê `/proc/<pid>/environ` e **para o teste** se o `DISPLAY` ou o `XDG_CONFIG_HOME` não forem os de automação. O `AGENTS.md` pede exatamente isso ("nunca abrir o editor na sessão do usuário a partir de automação") |
| **`graph.from_code` não gravava o arquivo** | O painel mostrava o texto aplicado e o `.graph.json` continuava com a lógica antiga, então **o Jogar rodava o código velho**. `graph_edit` grava, mas o caminho de aplicar código só trocava o documento em memória | Passa a gravar como as outras edições: converte primeiro (texto fora do subconjunto deixa o arquivo intacto), copia para `scripts/.backup/`, grava atômico, apaga o `.layout.json` velho (os ids mudaram) e empilha o desfazer. Teste `applied_code_reaches_the_file_and_a_refused_text_keeps_it` falha sem a correção |
| **O botão "Aplicar código" não existia** | O `graph_panel.cpp` conectava `applyCodeButton_`, que era `nullptr`: o Qt imprimia `QObject::connect(QAbstractButton, GraphPanel): invalid nullptr parameter` a cada abertura e o botão documentado **nunca apareceu**. O campo de código era `setReadOnly(true)`, então a "edição contínua das duas vistas" declarada no `DR-34` não tinha como ser usada pela janela | Botão criado no cabeçalho do código (com ícone e nome falado); campo editável; `showCode` não sobrescreve mais o que a pessoa digitou; um `writeCode` interno separa a escrita do programa da digitação |
| **O painel de código ficava vazio para sempre** | `code_->clear()` ao trocar de grafo disparava `textChanged` e marcava o texto como "editado", então a atualização seguinte era bloqueada. Sintoma: "2 nó(s), nenhum erro" com a área em branco | As duas chamadas passaram a usar `writeCode(QString())`, que limpa a marca |
| **O teste de fumaça não alcançava "Ver como código"** | O botão fica no fim de uma barra que se desloca conforme a largura da lista de scripts; o clique em `x=1153` acertava outro lugar e a vista não trocava. Era a causa do meu próprio "apply não funciona" | Atalhos novos, que também servem à acessibilidade: **`F8`** alterna a vista de código e **`Ctrl+Enter`** aplica. O teste usa os atalhos, sem pixel fixo |

### O que a auditoria confirmou como correto

Spline e perfis varridos, terreno em blocos com erosão/ruído/carimbos girados, rios com descida e
confluências, estradas com cruzamento e ponte/bueiro, grafo de navegação, os comandos `terrain.*`,
`path.*` e `road.detect_structures` com desfazer, as tabelas `Terrain`, `TerrainChunk`, `River`, `Road` e
`Bridge` salvando e reabrindo, o convés de ponte no quadro, os blocos de terreno na física e o teste de
isolamento de VM por anexo. O aviso de `nullptr` no log do editor **sumiu** depois da correção do botão.

**Não verificado:** a janela na sua sessão real (a evidência é o display virtual); leitor de tela (o passo por teclado escreve no rodapé, mas **não foi testado** se o leitor de tela anuncia); o mesmo passo no pincel de esculpir caverna; campo numérico para `times` na barra; terreno grande (desempenho e memória); a forma **platô** só tem teste no domínio — na janela foram vistos a cratera, o monte e a crista (reta e girada); e a silhueta girada **não** é previsualizada antes do clique.

## Etapa atual: vegetação, clima e partículas (2026-10-31)

Trabalho feito **sem tocar na Aipo**, que outra sessão deixou quebrada (`aipo-formatter` não compila).
Tudo aqui é domínio e render puros, com teste.

| Entrega | Evidência |
|---|---|
| **Leitor de GLB com esqueleto** chega ao arquivo: `Skeleton`, `Clip`, `Skin`, `SkeletonLink` (feito nesta rodada) | 1 teste |
| **`petunia_flora`:** vegetação por regras — espaçamento, faixa de altura, inclinação máxima, escala, cor com variação, semente e **orçamento**; determinística (hash de célula, nunca `rand()`) | **7 testes**: espaçamento, determinismo em 7 execuções, faixa/inclinação, escala e rumo, regra inválida pelo nome, faixa invertida, orçamento |
| **Impostores no render:** cada planta vira duas faces viradas para a câmera; planta quebrada é pulada, não gera triângulo degenerado | **5 testes**: duas faces por planta, base no chão, o quad gira com a câmera, entrada hostil, tinta dentro da faixa |
| **Clima e hora do dia:** sol, ambiente, horizonte, névoa, nublado e precipitação como **funções puras** da hora; **alto contraste** levanta o piso de luz e afina a névoa (`DR-13`) | **7 testes**: meio-dia/meia-noite, determinismo, noite mais escura, chuva traz névoa e granizo não, alto contraste, direção unitária, hora hostil embrulhada |
| **Partículas:** pool limitado, chuva e neve, retorno ao topo da caixa, passo limitado a 0,1 s para um quadro longo não teletransportar nada | **7 testes**: teto do pool, mesma semente = mesma chuva, queda e caixa, neve mais lenta, quadro longo, passo inválido, presets |
| **Operações do editor:** `flora.add` / `flora.list` / `flora.count`, `weather.set` / `weather.get`; o quadro recebe vegetação, névoa e chuva | **5 testes**: regra adicionada/listada/recusada, clima não vira desfazer, tempo limpo sem névoa, contagem, projeto sem regra não desenha |

**Janela (2026-11-02):** menu **Mundo** — Clima (limpo, nublado, chuva, neblina, neve), **Hora do dia**
e **Plantar regra**. O clima vale nesta sessão (não entra no arquivo nem no desfazer). A regra de
planta grava uma linha e o `Ctrl+Z` desfaz. Chuva e neve ligam o mesmo relógio da animação, senão
as gotas ficavam paradas. O `petunia_render_gl` já desenhava os impostores e a precipitação; o que
faltava era a porta. Os **decals** continuam bloqueados pelo `DR-12`.

**Corte para montar um jogo (2026-11-02, `DR-41`, `DR-42`, primeiro corte do `DR-13`).**
Parentesco: `Transform.parent`, pose composta até 32 níveis, física e desenho usam essa pose.
Arrastar na hierarquia ou **Tornar filho de…** / **Soltar da hierarquia**. Câmera fixa: tabela
`Shot`; no Jogar, a de maior prioridade cuja caixa contém o jogador substitui a perseguição
(menu da hierarquia: **Câmera fixa neste objeto**). Pós: a cena passa por um framebuffer com
contraste, vinheta, grão e teto de brilho; **Exibir › Conforto visual** reduz isso e limita a
subida do brilho. Chuva: traços mais longos e pool de 4.800 (era 12.000). Água ganha fresnel
quando corre; a estrada ficou asfalto mais escuro. `play.start` informa quantos scripts estão
ligados a objetos. O clima **ainda não** entra no arquivo. Bloom, AgX, SMAA, refração da água e
decals **não** entraram. A janela destes itens **não foi clicada** nesta rodada.

**API e jogador (2026-11-02, `DR-43`).** `Vec3` e `Color` entram em todo script como structs; `transform.position` / `rotation` / `scale` devolvem `.x` `.y` `.z`, e `set_*` / `move` / `rotate` recebem um `Vec3`. Os eixos (`rotate_y` e afins) ficam. A tabela `Camera` no personagem escolhe `fixed`, `third`, `first` ou `isometric`. O herói criado na hora ganha essa linha e a cápsula `corpo`. Testes de `petunia_script`, `petunia_project`, `petunia_graph` e `petunia_editor` (lib) passaram; a janela **não** foi aberta de novo com este binário.

**Interagir e vida (2026-11-02, `DR-44`).** Tabela `Interact` (raio 2 m, texto, ligada). No Jogar, **E** avisa o objeto mais perto (`on_event` `"interact"`) e a barra mostra o texto. Tabela `Health`; `health.damage` / `health.heal`; `"died"` uma vez quando a vida chega a zero. Som, slot de partida e animação de três estados continuam de fora. A tecla **E** na janela está ligada no código; o clique nesta rodada **não** foi feito.

**Modo edição e inspetor (2026-11-02, `DR-45`).** Empurrar e extrudar só no modo edição, e o clique nesse modo chega na face. O arame da malha fica claro, com marca no meio da aresta ou da face. `Camera.mode` é uma lista (`custom` deixa a câmera quieta). Interagir com um `Shot` olha por essa câmera. Adicionar tabela e adicionar script buscam pelo nome. Clima como entidade, vento, raio, skybox e campos do script no inspetor **não** entraram.

## Etapa atual: personagem animado na cena (2026-10-31)

Continuação da fase 4. O motor já calculava pose e skinning; o que faltava era **o dado chegar ao
arquivo e o objeto da cena saber tocar um clipe**.

| Entrega | Evidência |
|---|---|
| **Import grava o rig:** tabelas `Skeleton` (juntas), `Clip` (canais) e `Skin` (`owner` + pesos por vértice) e `SkeletonLink`; texto em formato legível, no mesmo lote dos meshes, então **um desfazer** leva o personagem inteiro | **1 teste**: GLB com esqueleto importado grava as quatro tabelas, e as três voltam a ser lidas (2 juntas, clipe de 1 s, 3 pesos) |
| **`petunia_editor::rig`:** serializa e lê esqueleto, clipes e pesos em ida e volta; uma linha malformada invalida o bloco inteiro em vez de meia-leitura | **4 testes**: esqueleto, clipe com três tipos de canal, pesos, e entrada quebrada recusada |
| **Tocar no viewport:** `anim.list`, `anim.play` (tempo inicial, velocidade, laço), `anim.stop`, `anim.advance`; o tempo vem da janela, nunca do relógio do sistema (determinismo) | **2 testes**: tocar move o mesh e avançar o tempo continua; parar volta à primeira pose |

**Janela:** **Criar › Tocar animação…** escolhe o clipe do objeto selecionado e o relógio da janela
chama `anim.advance`. **Parar animação** volta à pose de repouso.

## Etapa atual: paridade do outliner e sombreamento (2026-10-31)

Pedido: "prossiga com tudo que ainda está faltando, não pare até a paridade estar 1 para 1".
**1 para 1 não é alcançável sem violar decisões já tomadas**, e isso está escrito na
[matriz de paridade](../02-arquitetura/12-paridade-ui-petunia3d.md): três das 20 linhas são **fora de
escopo por decisão registrada** (QT-03 cursor 3D, QT-04 Poly Pen, QT-10 operador de desenho — as duas
últimas de modelagem de malha, que o `DR-22` recusou copiar). O que era **"planejado" chegou a zero**.

| Entrega | Evidência |
|---|---|
| **Outliner: ocultar, travar e "ver só este"** (`QT-12`): tabela `EditorState` (objeto sem linha é visível e destravado), operações `object.hide` / `object.lock` / `object.solo`, olho e cadeado por linha, menu de contexto, e o estado viajando com o projeto | **6 testes**: domínio (2) e fluxo (4: oculto sai do quadro e um desfazer volta, travado é desenhado mas recusa seleção, solo mostra um e devolve a cena intacta, e o estado sobrevive a salvar e reabrir) |
| **Sombreamento: sólido, aramado e raio-X** (`QT-09`): `view.shading`, menu **Exibir › Sombreamento**, o GL desenhando em linhas ou com o teste de profundidade desligado | **2 testes**: modo muda o quadro sem virar passo de desfazer; modo desconhecido recusado pelo nome |

**Defeito que eu mesmo criei e consertei no caminho:** ao alargar a hierarquia para caber quatro
colunas, **o viewport deslocou e os dois testes de fumaça caíram** (13→10 e 16→13). O layout voltou e o
id foi para a dica; a largura volta a ser a de antes. Foi também a **segunda vez** que uma tabela nova
(`EditorState`) empurrou `PlayerStart` no menu do inspetor: o teste de fumaça agora desce 8 itens, com
a ordem escrita ao lado.

**Ainda parcial, com o que falta listado:** QT-12 sem cor por linha, multisseleção e arrastar; QT-15
sem painel de referências visuais; QT-07 sem layout nomeado em arquivo; QT-00 com a borda dos botões
(a seu gosto); QT-01 e QT-11 sem passo de teste de fumaça para "apagar tudo" e "cena vazia".

## Etapa atual: animação esquelética (2026-10-31)

Pedido: "A e depois C" — animação esquelética primeiro, depois uma revisão do que foi feito.
O **núcleo de reprodução** e a **leitura de GLB com esqueleto** estão prontos e testados.

`DR-23` propunha copiar ~3,8 mil linhas do Petunia3D. **O Petunia3D não está nesta máquina**, então
o caminho foi o que a própria pesquisa do `DR-23` mandava fazer primeiro: importar o GLB e escrever
o núcleo. O resultado é código novo, sem legado para migrar.

| Entrega | Evidência |
|---|---|
| **Crate `petunia_anim`:** `Mat4` (coluna-maior) e `Quat` (slerp pelo caminho curto), esqueleto que recusa ciclo e limite, clipes com canais de translação/rotação/escala, amostragem **degrau** e **linear**, **um passo** para a pose (pais antes dos filhos, garantido na construção), matrizes de skinning, mistura com a pose de repouso | **10 testes**: o pai move o filho, degrau segura e linear interpola, 9 execuções dão a mesma pose, duração é a última chave, mistura 0 = repouso, bind pose não move o vértice, ciclo recusado, junta inexistente recusada, chaves fora de ordem recusadas, esqueleto acima do limite recusado |
| **Leitura de GLB com esqueleto** (`petunia_import/src/animation.rs`): primeiro *skin*, matrizes de bind, todas as animações; **leitor de accessor próprio** (a `gltf` 1.4 não expõe um), com stride, checagem de truncamento, números não finitos viram 0 e interpolação cúbica **recusada pelo nome** | **1 teste com um GLB montado byte a byte**: 2 juntas, 1 clipe, o clipe move a raiz e o filho acompanha; mais 1 teste que prova que um arquivo sem esqueleto continua importando igual |

**Regressão de fase 1 consertada no caminho:** a tabela nova `FaceColor` ordena antes de
`PlayerStart` no menu **Adicionar tabela**, então o passo que escolhia a 6ª linha passou a escolher a
errada. O teste agora conta 7, com a ordem escrita ao lado para conferir. O script também passou a
**reportar as duas fases** mesmo quando uma reprova: antes o `set -e` matava a execução no primeiro
`python3` que saísse com erro e a fase pNodes nunca rodava — por isso uma queda de 13 para 9
verificações parecia "fase 2 quebrada" quando era só a fase 1 escondendo o resto.

**Skinning até o desenho (2026-10-31):** a cadeia está completa — o GLB traz os pesos, o import
guarda um peso por vértice, o `DrawItem` carrega as matrizes da pose, e o renderizador aplica o
deslocamento antes de subir a malha.

| Entrega | Evidência |
|---|---|
| **Pesos por vértice no import:** `ImportSkin` com até 4 juntas por vértice, normalizados; soldar posições que têm pesos diferentes é **costura** e vira aviso, com o primeiro peso vencendo | teste com o GLB montado: os pesos chegam e movem o vértice |
| **`DrawItem` com skin:** `skin` (os pesos) e `skin_matrices` (a pose do quadro); `skin_mesh` devolve uma **cópia** deslocada e nunca toca a malha em cache | 6 testes: todo vértice se move, a origem fica intacta, item sem skin não copia, dois pesos dão a média, peso zero fica parado, matrizes diferentes dão impressão digital diferente |
| **GL:** `SkinCache` com buffers próprios (escritos de novo quando a pose muda, teto de 256) e um caminho de desenho separado; o layout de vértice é o mesmo do caminho estático, então o sombreador é compartilhado | compila; o cache é liberado junto com as outras malhas |

**Decisão de custo:** o skinning roda na **CPU**, não numa paleta de ossos no sombreador. O vértice
não tem atributo de junta nem de peso, e acrescentar dois atributos a **toda** malha por causa das
poucas que são animadas custaria memória em todas. Fica local em `petunia_render_gl/src/skin.rs`;
uma paleta na GPU passa a valer a pena quando uma cena tiver muitos personagens animados ao mesmo
tempo.

**O que ainda não existe (declarado):** IK, interpolação cúbica e a autoria de esqueleto (fora de
escopo pelo `DR-23`); e não há ainda um caminho do **editor** que ligue um objeto da cena a um
esqueleto importado e a um clipe — o motor sabe desenhar um personagem animado, mas o editor ainda
não oferece "toque este clipe" numa cena salva.

## Etapa atual: fechamento da fase 2 e o módulo `grid` (2026-10-30)

Pedido: "feche tudo que precisa da fase 1 a fase 4". A fase 2 está **fechada**; da fase 3 entrou
o módulo `grid`. O que resta das fases 3 e 4 está listado abaixo, com o motivo.

| Entrega | Evidência |
|---|---|
| **Autosave:** `editor.autosave` grava quando há trabalho não salvo e **espera** quando um arrasto ao vivo muda o mundo sem comando; um `QTimer` de 60 s na janela pede, o motor decide | 1 teste (limpo → grava → limpo de novo → `busy` com prévia ao vivo, e o arquivo não recebe a prévia) + timer compilado |
| **Painel de assets:** lista `assets/` com tamanho legível, total, botão **Importar modelo…** e estado vazio explicado | compila; `asset.list`/`asset.import` já tinham teste; o painel é a porta |
| **Material por face:** tabela `FaceColor` (`owner`, `face`, `color`) e `mesh.paint_face`; a face sem linha usa a cor do objeto; índice de face obsoleto é ignorado | 3 testes do domínio (`petunia_world`) + 1 de fluxo no editor (pintar duas vezes deixa **uma** linha, um desfazer por pincelada) |
| **Alças de colisão no viewport:** `collider.resize_begin/drag/end/cancel` — prévia ao vivo e **um** desfazer ao soltar; cancelar devolve a linha exata | 2 testes (arrasto commita em um passo; cancelar restaura) |
| **`petunia_grid`** (fase 3): quadrado 4/8 direções e hexagonal axial, A\* **próprio** com custo inteiro (1000 reto, 1414 diagonal), heurística admissível, desempate por célula, orçamento e recusas nomeadas | **10 testes**: linha reta, desvio de parede, determinismo (9 execuções iguais), diagonal que não corta esquina, inalcançável, fora dos limites, seis vizinhos do hex, losango do hex, célula cara evitada, orçamento hostil |

**Correção de documento:** a página do editor afirmava que as colisões aparecem **em verde** no
viewport; o desenho existe (`collider_overlay`), e o que faltava era **arrastar** as alças, agora
implementado.

## Antes: intercâmbio Aipo ⇄ IR e isolamento por objeto (2026-10-28)

| O quê | Verificação |
|---|---|
| **Uma VM por anexo** (objeto + script): o módulo compilado é compartilhado, o estado de topo não vaza entre objetos | teste: o mesmo script em dois objetos move os dois para a mesma posição, não soma o estado |
| **Leitor Aipo → IR** para o subconjunto explícito do DR-34 | 6 testes: ida e volta, atribuição composta/`while`, `loop`/`each`, recusa com linha e leitura do código gerado por grafo |
| **Escritor IR → Aipo** canônico e determinístico | o IR lido da saída é igual ao primeiro |
| **Fora do subconjunto** (`struct`, closures, `attempt`, etc.) | `IR_UNSUPPORTED`, sem conversão silenciosa |
| **IR → grafo** (evento, funções auxiliares, globais, variáveis locais, `while`, `loop`, `repeat`, `each`, `break`/`continue`, `return`, chamadas, `if`/`elif` e expressões) | 15 testes: execução ponta a ponta, condição/`elif`, `repeat` com índice, `while`, `loop` com `break`/`continue`, `each` com uma e duas variáveis, função auxiliar, global/local, chamada de valor, variáveis e escopo por handler, e recusas de `let` reatribuído/`break` fora de laço |
| **Editor:** `script.to_graph` e menu **Transformar em grafo…** no script | teste de operação cria o grafo, abre, compila e converte função auxiliar; recusa lista como valor e nome ocupado. O menu não foi exercitado no smoke |
| **Schema v2 e migração** | **implementado:** grafos v1 são lidos e migrados para v2; `flow.while`, `flow.loop`, `flow.break`, `flow.continue` e `flow.each.with_index`; 14 testes de modelo e 21 testes públicos de grafo |
| **Um método por projeto (`DR-40`)** | **implementado:** `programming_method` no manifesto; a janela esconde a aba do outro método; prefabs gravam `.aipo` ou `.graph.json`; o Doctor avisa a mistura. `graph.from_code` continua no protocolo, mas deixou de ser o fluxo normal |
| **Edição contínua das duas vistas** | **implementado no recorte suportado:** `graph.code` gera Aipo e `graph.from_code` recria o grafo; botão **Aplicar código** (`Ctrl+Enter`) no painel Lógica, com o texto **editável**; **Ver como código** também no `F8`. Fora do subconjunto, diagnóstico e o arquivo não muda. Aplicar grava o `.graph.json` na hora, como as outras edições. Um teste de 2026-10-29 prova que o texto aplicado chega ao arquivo e que um texto recusado o deixa intacto; o teste de fumaça digita, aplica e lê o `Graus = 45` do arquivo |

## Antes: renomear e apagar scripts/grafos (2026-10-27)

| O quê | Verificação |
|---|---|
| **Renomear** script ou grafo: move o arquivo (e o `.layout.json` do grafo), atualiza todos os anexos e o grafo aberto; recusa nome ocupado | 1 teste: `a` → `c`, arquivo e anexo acompanham; nome `b` existente é recusado |
| **Excluir**: sem `force` recusa se houver anexos (`SCRIPT_IN_USE`); com `force`, move para `scripts/.backup/` e desliga os anexos | teste: recusa, exclusão com backup, lista limpa |
| **Menu de contexto** na lista de scripts (workspace Código) e na lista de scripts/grafos (workspace Lógica): Renomear…/Excluir… com confirmação | compilado; não exercitado no teste de fumaça |
| Workspace inteiro (2026-10-28) | **441 testes, 0 falhas**; fmt dos crates alterados e clippy ok; editor compila; teste de fumaça **11/11** |

**Não verificado:** renomear/excluir pela janela; leitor de tela.

## Antes: vários scripts por objeto (2026-10-27)

| O quê | Verificação |
|---|---|
| **Primeiro script** continua na linha `Script` do objeto; o **segundo** vira entidade-filha com `owner` e `order`; anexar de novo o mesmo arquivo não duplica | 1 teste de ponta a ponta: dois scripts no mesmo objeto, os dois rodam no mesmo quadro |
| **Ordem e isolamento**: objeto → `order` → arquivo; uma falha desliga só aquele anexo | testado pelo host; o teste confere os dois logs |
| **Salvar, reabrir, duplicar e excluir**: os anexos viajam com o objeto e não contam como objetos na Hierarquia | teste: fecha e abre, duplica, exclui e confere as contagens |
| Janela: **Anexar** acrescenta scripts; os anexos aparecem sob o objeto; `owner` é somente leitura, `order`/`enabled` editáveis | compilado; não exercitado no teste de fumaça |
| Workspace inteiro | **413 testes, 0 falhas** (naquela etapa); fmt/clippy ok; editor compila; teste de fumaça **11/11** |

**Não verificado:** vários scripts pela janela (só pelo protocolo/teste); reordenar por arrastar; leitor de tela.

## Antes: Luau, duas vistas e idioma (2026-10-27)

| O quê | Verificação |
|---|---|
| **As duas vistas de nós existem**: **Contorno** (lista/sheet, acessível por teclado) e **Caixas e fios** (canvas), ambas sobre o mesmo `graph.json` e as mesmas operações | **visto** no display virtual; o teste de fumaça edita e arrasta nas duas |
| **Não há Event Sheet/Gameplay IR separado**: o Contorno é a vista em lista do grafo; o grafo compila direto para Aipo (decisão no DR-30) | documentado; nenhuma camada IR separada |
| **Luau**: será implementado depois da integração da Aipo; uma linguagem por projeto. Até lá, `"luau"` no manifesto é **válido com aviso** (`project.manifest.luau_pending`) e não bloqueia abrir o projeto | 2 testes: validação do manifesto e carregamento com aviso |
| **Idioma de nós/sheets**: plano registrado; arquivos guardam ids, catálogo concentra os textos, Contorno e canvas compartilham a tradução; o Aipo gerado não é traduzido | DR-19 (revisão 2026-10-27) |
| Workspace inteiro | **412 testes, 0 falhas** (naquela etapa); fmt/clippy ok; editor compila; teste de fumaça **11/11** |

**Não verificado:** a tradução em si ainda não foi feita; o Luau ainda não roda.

## Antes: edição de malha na janela (2026-10-27)

| O quê | Verificação |
|---|---|
| **Vértices visíveis no modo de edição**: todos os vértices da malha aparecem como pontos; os escolhidos ficam maiores e brancos | **visto (display virtual)**; teste: um marcador por vértice no modo vértice e nenhum no modo face |
| **Barra Malha**: inserir, extrudar, chanfro, corte em laço, subdividir, dissolver, fundir, fundir por distância, apagar faces e virar normais | **visto**: inset aplicado num cubo; bevel inválido mostrou a razão; barra com distância, cortes e segmentos |
| Atalhos de modelagem (`I`, `Ctrl+E`, `Ctrl+B`, `Ctrl+R`, `Ctrl+X`, `M`, `X`) e menu **Malha** na barra superior | compilado; os atalhos só valem com o viewport em foco e no modo de edição |
| Modo de edição avisa o motor (`mesh.edit_mode`, `mesh.component_mode`); ao sair ou trocar de projeto, os marcadores somem | teste do overlay e chamadas no caminho da janela |
| Workspace inteiro | **410 testes, 0 falhas** (naquela etapa); fmt/clippy ok; editor compila; teste de fumaça **11/11** |

**Não verificado:** bevel/loop cut/merge em todas as combinações de seleção pela janela; leitor de tela; malhas grandes.

## Antes: colisão do jogador e fluxo de código (2026-10-27)

| O quê | Verificação |
|---|---|
| Prefab **Personagem jogável**: cilindro com `Character.player` e cápsula sólida | 1 teste de ponta a ponta (nasce, fica no chão, anda com o teclado) |
| **`PlayerStart` no Piso não vence o marcador**: câmera e herói usam o ponto de início sem forma | 1 teste: o marcador em y = 1,7 vence o piso em y = −0,25 e o herói nasce a 0,95 |
| Project Doctor avisa quando há vários inícios e um está num objeto desenhado | aviso novo em `check` |
| **Personagem com `RigidBody` dinâmico** passa a ser corrigido para cinemático, em vez de ficar parado | teste: o corpo é corrigido, o personagem fica de pé e o aviso aparece |
| **Personagem com apenas um gatilho** recebe a cápsula automática de caminhada | teste: o personagem fica de pé e anda |
| **Fluxo código ↔ nós decidido:** grafo é a fonte; código é somente leitura; “Transformar em script” separa o texto; `.aipo` não vira grafo automaticamente | registrado no DR-30 (revisão 2026-10-27) |
| Workspace inteiro | **409 testes, 0 falhas** (naquela etapa); fmt/clippy ok; editor compila; teste de fumaça **11/11** |

**Não verificado:** o prefab novo e a correção do corpo dinâmico na janela real (cobertos por testes de domínio/editor); leitor de tela.

## Antes: área de programação e canvas (2026-10-27)

| O quê | Verificação |
|---|---|
| Canvas: arrastar caixas, **puxar fios** de uma saída até uma entrada, clique-clique, seleção múltipla (`Ctrl+clique`, `Ctrl+A`), **agrupar/desagrupar** e mover o grupo | **visto (display virtual)**: fio criado e gravado, posições salvas, grupo criado e movido; teste do layout/grupos |
| Canvas: pan (arrastar o fundo/botão do meio), zoom 25–400 % (`Ctrl`+roda, `+`/`-`, botões), **Centralizar** (`Home`/botão) e canvas amplo | **visto**: zoom 110 → 149 %, pan e centralizar |
| Pinos de objeto: **Escolher objeto…** com busca por id e nome | **visto**: #9 escolhido e gravado no grafo |
| Workspace Lógica lista scripts `.aipo` e grafos, marca os ligados ao objeto e mostra arquivos pedidos mas ausentes; o workspace Código também lista os ausentes e cria com dois cliques | 1 teste; **visto** na lista |
| **Transformar em script** copia o código gerado para um `.aipo` independente; **Ver como código** mapeia linha → nó | compilado; não exercitado no teste de fumaça |
| Workspace inteiro | **404 testes, 0 falhas**; `cargo fmt` e `clippy -D warnings` ok; editor compila; teste de fumaça **11/11** |

**Não verificado:** leitor de tela no canvas; grafo grande; o clique em **Transformar em script** e a lista de scripts ausentes pela janela.

## Antes: integridade de dados (2026-10-27)

| O quê | Verificação |
|---|---|
| `i64::MIN` não estoura mais a validação; inteiros fora de ±(2^53−1) são recusados sem panic | 2 testes (`petunia_world`, `petunia_script`) |
| Salvar, desfazer e refazer são recusados enquanto um arrasto altera o mundo sem comando (`EDITOR_BUSY`); o arquivo não muda | 3 testes (`petunia_editor`) |
| Abrir um projeto descarta o gesto preso ao mundo antigo; ele não atinge entidades do projeto novo | 1 teste (`petunia_editor`) |
| Editor cancela arrasto, escultura e caminho ao perder o foco e antes de salvar/abrir/fechar; pede para salvar script pendente ao trocar de projeto | **compilado** com Qt 6.4.2; o caminho novo não é exercitado pelo teste de fumaça |
| Inspetor de colisores não desloca mais os valores após o primeiro reload; os campos são esvaziados depois de esconder os widgets | **compilado**; não há passo de colisão no teste de fumaça |
| Editor Qt compila e o teste de fumaça passa | **passou (Qt 6.4.2, display virtual, 11/11)** |
| Workspace inteiro | **402 testes, 0 falhas**; `cargo fmt` e `clippy -D warnings` ok |

**Não verificado:** a sua sessão real; o caminho de salvar durante um arrasto e o inspetor de colisores não têm passo no teste de fumaça; leitor de tela e navegação só por teclado.

## Etapa atual: sistema de nós, passo N4 — prefabs e o nó Esperar (2026-10-26)

Decisão: [DR-31](../06-decisoes/registro-de-decisoes.md).

| Entrega | Evidência |
|---|---|
| Nó **Esperar**: compila para uma função por espera, contagem por objeto, reinício, aviso em repetição, só o objeto depois da espera | 4 testes de ponta a ponta no Script Host (a cadeia continua 1 s depois, duas esperas seguidas, não repete) |
| **Prefabs** Porta, Item coletável e Ponto de nascimento (`prefab.list`, `prefab.spawn`), em **Criar** e na hierarquia | 4 testes: os grafos compilam sem aviso; **o jogador pega o item**; **a porta sobe, espera ~3 s e desce, uma vez por visita**; criar = um desfazer e grafo editado nunca é sobrescrito. Menu **visto** no display virtual |
| **Defeito do motor corrigido:** personagem × sensor/parede estática nunca gerava `on_collision` | teste novo no `petunia_physics` (falha sem a correção); 18 testes do crate |

**Não verificado:** a porta e o item **jogando de verdade na janela** com o teclado (só pelo motor de scripts com física e no teste do editor); uma execução isolada do teste de fumaça mostrou 9 de 10 e não vi qual (três repetições deram 10/10); leitor de tela.

**Ainda falta:** porta animada. (Os prefabs do próprio projeto foram feitos em 2026-11-10, no topo.)

## Etapa atual: sistema de nós, passos N3 e N5 (2026-10-26)

A **tela de caixas e fios** (workspace **Lógica**, alternador Contorno | Caixas e fios) e a **depuração**
(destaque do nó que rodou e "por que esta condição falhou?"). Decisão: [DR-32](../06-decisoes/registro-de-decisoes.md).

| Entrega | Evidência |
|---|---|
| `petunia_graph`: `scripts/<nome>.layout.json` (ler, gravar, podar, recusar arquivo hostil) e `canvas` com caixas, pinos, fios e posições calculadas quando não há layout | 5 testes novos; o mesmo grafo sempre dá as mesmas posições |
| Marcas de depuração no compilador (`compile_with`): `graph_debug.node/check/value`, com o texto **idêntico** ao normal quando desligadas | teste que compara os dois textos byte a byte, e um que roda o grafo e vê a condição falsa com os valores usados |
| `play.debug` + marcas ligadas ao jogar no Script Host | teste do host e teste do editor (monta um `Se` falso pelo protocolo, joga um quadro e lê as marcas) |
| Editor: `graph.view` traz `canvas` e `layout_problem`; `graph.set_layout` grava só o layout, fora do desfazer | teste do editor que move uma caixa, confere a posição e que `gira.graph.json` continua sem coordenadas |
| Janela: `QGraphicsView` com caixas, fios, arrastar (grava o layout), clicar (seleciona, o contorno acompanha), clicar saída → entrada (liga), Tab/setas/Enter/Delete/`+`/`-`/`0`, zoom com Ctrl+roda, destaque do que rodou e lista das condições falsas | **visto no display virtual**: quadro do canvas e quadro **jogando** com o nó do evento destacado; teste de fumaça **11/11**, incluindo o arrastar que muda só o `.layout.json` |

**Corrigido ao conferir no display virtual:** depois de arrastar uma caixa, os fios e os pontos de clique dos pinos ficavam no lugar antigo; agora o canvas se redesenha ao soltar a caixa. A captura do jogo com o grafo ligado a um objeto mostra as duas caixas destacadas como "rodou".

**Não verificado:** leitor de tela no canvas (o contorno continua sendo a interface acessível; **não ouvi** nenhum dos dois); grafo grande no canvas; o `F7` com teclado real.

**Ainda falta:** porta animada, spawn de criaturas, pontuação do item (declarados fora do DR-31); os prefabs do próprio projeto vieram depois, em 2026-11-10 (topo).

## Etapa atual: sistema de nós, passo N2 (2026-10-26)

Comandos `graph.*` e a **vista em contorno** no editor (workspace **Lógica**, `F7`). Decisão: [DR-30](../06-decisoes/registro-de-decisoes.md).

| Entrega | Evidência |
|---|---|
| `petunia_graph`: editar (adicionar na cadeia, remover fechando o buraco, ligar, desligar, constante), contorno, busca sem acento, fontes de valor | 9 testes novos de ponta a ponta do crate |
| 12 operações `graph.*` no editor, com desfazer próprio, gravação na hora, cópia em `scripts/.backup/` e recusas com a razão | 2 testes do editor, incluindo montar um grafo, desfazer, refazer e **jogá-lo** |
| Workspace **Lógica**: lista de grafos, paleta de nós por busca, contorno `QUANDO … FAÇA`, menu de cada entrada (valor fixo, usar o valor de…, novo nó, esvaziar), Ver como código, Ligar ao objeto, Insert / Enter / Delete / Ctrl+Z | **visto no display virtual**, vários quadros; teste de fumaça **10/10** (cria um grafo pelo mouse e teclado e confere o arquivo) |

**Não verificado:** leitor de tela (as linhas têm texto e descrição falados prontos, mas **não ouvi**); o `F7` com teclado real (no teste de fumaça o `xdotool` desmapeia a tecla depois de digitar, então ele clica na aba); grafos grandes.

**Ainda falta:** os prefabs como grafos (N4).

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

**Ainda falta da fase 2:** material por face; interface da grade no viewport (o `grid`/A\* existe para scripts desde 2026-11-09).

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

**Não verificado:** a sua sessão real; o jogo no `petunia run` numa janela; teclado e mouse reais no modo Jogar (só `xdotool` no display virtual); leitor de tela. (As alças para redimensionar colisões foram vistas e exercidas no display virtual: caixa de 1 m redimensionada para 2 m num arrasto, com centro movendo 0,5 m; `Ctrl+Z` reverteu.)

**Ainda falta do bloco:** material por face. (O Event Sheet foi descartado pelo DR-30, que o trocou pelo sistema de nós; o módulo `grid`/A\\* foi feito em 2026-11-09; Porta, Item coletável e Ponto de nascimento foram feitos no passo N4, em 2026-10-26; os prefabs do próprio projeto foram feitos em 2026-11-10, no topo; as alças no viewport para redimensionar colisões foram feitas em 2026-11-08: as quatro faces de uma colisão caixa viram quadrados brancos, arrastar uma delas muda `size` e `center` juntos com o desfazer de um passo; colisões esféricas, em cápsula e convexas ficam para depois.)

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

**Não verificado:** a sua sessão real; o jogo com física numa janela. (As alças no viewport para redimensionar colisões foram feitas em 2026-11-08, para colisões em caixa com rotação zero.)

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
| **Ganchos da interface**: menu **Malha**, `Tab`, atalhos, faca (`K`), **Booleano…**, valor ao vivo e contagens no inspetor | **visto (display virtual)**: união 8/12/6 → 16/24/12; faca 8/12/6 → 10/15/7; inset ao vivo 8/12/6 → 12/20/10; `Esc` cancela a prévia |
| Desempenho no build do editor (6,5 mil vértices) | **medido**: booleano 74 ms, inset em todas as faces 29 ms, checar sólido 3 ms |
| `cargo fmt`, `clippy -D warnings`, **310 testes** (2 threads), `cargo deny` | **passou**, recompilado do zero numa pasta de build **isolada** (`/tmp/petunia-x.verify`); o `cargo deny` rodou com a base de avisos local (`--offline`) |
| **Falso verde encontrado:** a cópia `petunia-engine-system-modules` usa a **mesma** pasta de build (`/tmp/petunia-engine-target`) | Uma cópia sobrescreve os binários da outra, e o `cargo test` chegou a rodar o `petunia_math` da cópia (5 testes em vez de 9) dizendo "ok". Até a cópia ter outra pasta de build, **verifique com `CARGO_TARGET_DIR` próprio** |

**Não verificado:** a **sua sessão real**; malhas acima de 6,5 mil vértices. O teste de fumaça passou **11/11** em display virtual (inclui converter uma caixa em malha editável).

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
| Crate **`petunia_script`**: compila e roda scripts **Aipo**, uma VM por anexo (objeto + script), `on_start` / `on_update` / `on_event`, orçamento de 100 mil instruções por chamada ([doc](../03-linguagens-e-api/05-script-host.md), [DR-25](../06-decisoes/registro-de-decisoes.md)) | 17 testes de host (ciclo de vida, orçamento, falha tratada e não tratada, sandbox, eventos, isolamento por objeto, determinismo, simulação) + 5 testes do leitor/escritor IR + 11 testes de IR → grafo |
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

**65,7 mil linhas de Rust** em 18 crates (14,1 mil delas copiadas do Petunia3D em `petunia_mesh/src/legacy/`), **11,0 mil linhas de C++** próprias no editor e **3,6 mil linhas de C++ copiadas** do Petunia3D (`editor/src/ui_kit/`; o tema foi reescrito, o resto está igual), medidas em 2026-10-29, depois do 2D e da folha de sprites. A tabela conta só `src/` e não inclui testes.

| Parte | Linhas | O que faz |
|---|---:|---|
| `petunia_math` | 1496 | `Vec2`, `Vec3`, `Color`, `Transform`, **`PolygonMesh`**, **spline Bézier** por comprimento de arco, ruído próprio |
| `petunia_world` | 2798 | Entidades, tabelas, tags, comandos adiados, RNG; `f32` salvo em decimal curto; tipo de valor **`mesh`**; **mapa de altura em blocos**, carimbos e rede de caminhos |
| `petunia_schema` | 466 | API como dado |
| `petunia_commands` | 1392 | Despachante com desfazer/refazer; histórico com **teto de memória** |
| `petunia_project` | 1801 | Formato de texto, salvar (recusa cena grande demais para abrir), diagnósticos, projeto de exemplo; tabelas **`Mesh`**, **`Sprite`**, **`SpriteAnimation`**, `Terrain`, `TerrainChunk`, `River`, `Road`, `Bridge`; **`load_png_asset`** (caminho dentro do projeto + decodificação) |
| `petunia_input` | 166 | Entrada por ações nomeadas |
| `petunia_import` | 877 | GLB/glTF → malhas editáveis; **PNG → RGBA8** para sprites; arquivo tratado como hostil, com limites e avisos |
| `petunia_render` | 3817 | Câmera, **seis formas**, **malhas editáveis**, **sprites, folha de sprites e pixels de textura**, picking (objetos, sprites e superfície), **overlay para gizmos**, volumes esculpidos, **terreno, água e estrada** no quadro |
| `petunia_sculpt` | 3745 | **Cavernas** (campo em blocos, *surface nets*, gerador, caminho) e **perfis varridos** de rio/estrada, cruzamentos, ponte/bueiro |
| `petunia_script` | 6678 | **Script Host**: compila e roda Aipo, API como dado, Doctor de scripts, simulação numa cópia, Aipo ⇄ IR |
| `petunia_graph` | 5120 | **Grafo de nós** (DR-30): documento tipado, catálogo tirado da API, compilação para Aipo com mapa de linhas, validação, layout |
| `petunia_render_gl` | 1268 | OpenGL 3.3: contorno de seleção, **passe de sprites com textura e recorte de quadro**, **passe de overlay em pixels**, buffers de malhas editáveis |
| `petunia_player` | 461 | Janela simples com câmera livre; carrega as imagens do projeto (`project_root`) e anima as folhas de sprites |
| `petunia_cli` | 184 | `petunia new / check / run / schema / version` |
| `petunia_mesh` | 15479 | **Edição de malha**: booleanos, inset, loop cut, bevel… (14100 linhas de legado do Petunia3D + API validada) |
| `petunia_physics` | 2972 | Corpos rígidos, colisões e consultas (Rapier **só aqui dentro**), água, personagem; física determinística |
| `petunia_editor` | 16141 | Sessão, operações JSON, **gizmo**, **gestos**, snap a objetos, Modo Livre, **operações de brush**, andares, **esculpir**, **edição de malha**, **terreno/rio/estrada** |
| `petunia_editor` | 16717 | Sessão, operações JSON, **gizmo**, **gestos**, snap a objetos, Modo Livre, **operações de brush**, andares, **esculpir**, **edição de malha**, **terreno/rio/estrada**, **sprites e animação de sprite** |
| `petunia_editor_ffi` | 212 | Fronteira C |
| `editor/` (C++/Qt, próprio) | 10960 | Janela, barras de ferramenta, workspaces, paleta de comandos, docas, inspetor, editor de código, painel de grafo |
| `editor/src/ui_kit/` (copiado) | 3589 | Tema, campo numérico, campo XYZ, expressões, arrasto, ícones |

Detalhes: [editor](../02-arquitetura/11-editor.md), [paridade](../02-arquitetura/12-paridade-ui-petunia3d.md).

## Verificação (honesta)

| Item | Estado | Evidência |
|---|---|---|
| `cargo fmt`, `cargo clippy --workspace --all-targets -D warnings` | **passou (2026-10-30, refeito nesta revisão)** | limpo; o `fmt` foi conferido **por crate** (`cargo fmt -p <crate> -- --check`), como a nota abaixo explica |
| `cargo test --workspace` (2026-10-31) | **passou: 590, 0 falhas** | 541 da revisão anterior + 4 de fase 2 (autosave, pintura por face, 2 de alça de colisão) + 3 de domínio da cor por face + 10 do `petunia_grid` |
| `cargo fmt --all` (literal) | **não passa neste repositório** | `--all` também formata a dependência de caminho `../aipo-lang` (27 diferenças, já existentes, nenhuma nos crates `petunia_*`). O que se mede aqui é `cargo fmt -p <cada crate petunia> -- --check`, e esse passa. |
| Suíte completa, **590 testes** (2026-10-31, com personagem animado na cena) | **passou** | math 21, world 47 (45 + 2 do outliner), sculpt 41, graph 31, grid 10, anim 10, physics 22, render 27 (22 + 5 de skin), editor 88 (81 + 4 do rig + 2 da animação + 1 do import), script 83, mesh 181, project 10, commands 6, import 8, input 3, schema 2 |
| Suíte completa, **568 testes** (2026-10-29, com folha de sprites) | **passou** | math 21, world 42, sculpt 41, graph 31, physics 22, render 41, editor 75 (73 + 2 de integração), script 82, mesh 181, project 7, commands 6, import 14, input 3, schema 2; `render_gl` e `player` não têm teste (precisam de janela) |
| Suíte completa, **562 testes** (2026-10-29, com `asset.import_sprite`) | **passou** | 561 + o teste da imagem importada como sprite; a quebra por crate é a da linha de 561, com `editor` em 74 (72 + 2 de integração) |
| Suíte completa, **561 testes** (2026-10-29, depois do 2D) | **passou** | math 21, world 42, sculpt 41, graph 31, physics 22, render 36, editor 73 (71 + 2 de integração), script 82, mesh 181, project 7, commands 6, import 14, input 3, schema 2; `render_gl` e `player` não têm teste (precisam de janela) |
| Suíte completa, **535 testes** (2026-10-29, `main` com a linha dos rios mesclada) | **passou** | math 21, world 42, sculpt 41, graph 39, physics 29, render 22, editor 70, script 67, mesh 181, project 6, commands 6, import 6, input 3, schema 2. **Atenção:** a quebra por crate desta linha não fecha com o checkout de hoje — `petunia_graph` tem 31 e `petunia_physics` 22 (`--list` confirma 53 nos dois juntos, não 68) e `petunia_script` 82, não 67. O total de 535 foi medido naquele momento; a quebra não foi reauditada. |
| Suíte completa, **441 testes** (2026-10-28) | **passou** | inclui 6 de Aipo → IR, 14 de IR → grafo, 1 de `script.to_graph` no editor, 1 de isolamento por objeto e 14 de modelo de grafo com migração v1→v2 |
| Suíte completa, **414 testes** (2026-10-27) | **passou** | mesh 181 (164 do Petunia3D), script 55 (15 + 16 do host + 7 de física + 17 de grafo), editor 57, graph 22 (13 + 9 de edição), physics 18, sculpt 17, render 16, world 14, math 11, commands 6, import 6, input 3, project 6, schema 2 |
| Formas orientadas para fora (caixa, rampa, cilindro, cone, esfera, escada) | **passou** | teste por geometria, em vários níveis de detalhe |
| Gizmo: acertar a alça, mover só no eixo e no snap, girar em passos, escalar sem zerar | **passou** | 6 testes em `petunia_editor` |
| Um arrasto vira **um** passo de desfazer; sem mudança, nenhum | **passou** | teste de gesto |
| Editor Qt compila e liga | **passou (Qt 6.4.2, 2026-10-30)** | `scripts/editor.sh build`; `editor-smoke.sh` **13/13** (10 da fase Aipo + 3 da fase pNodes) e `natural-smoke.sh` **16/16**. O teste de fumaça **não** exercita os quatro itens novos da fase 2 (autosave, painel de assets, pintura, alças) |
| Janela: selecionar, **arrastar a seta X**, **adicionar tabela pelo combo**, duplicar, criar, andar, girar, salvar | **passou (display virtual)** | `scripts/editor-smoke.sh` **confere o arquivo salvo** (`x=4.5`, só no eixo X, na grade; tabela `PlayerStart` adicionada) |
| **Sprite desenhado sobre geometria 3D, com recorte de transparência** | **passou (display virtual, pixels lidos)** | `scripts/sprite-smoke.sh` **3/3**: meio magenta `(255, 0, 255)`, borda transparente mostrando a caixa `(36, 95, 38)`, fora do quadrado verde |
| Aparência: tema escuro, ícones, campos XYZ, trilho, barra do viewport | **visto (display virtual)** | imagens |
| Gizmos de girar e de escalar, cone, esfera e escada renderizados | **visto (display virtual)** | imagens |
| Realce da alça ao passar o mouse | **visto (display virtual)** | imagem do gizmo de escalar (alça Y em amarelo) |
| Vistas predefinidas (`Alt+1` a `Alt+6`), `Esc` cancela o arrasto | **não rodou** | código escrito |
| Arrastar um **anel** ou um **quadrado de escala** com o mouse | **não rodou** | só a matemática tem teste; a interface só foi exercitada com a seta de mover |
| Trocar tema, densidade e escala pelo menu; alto contraste | **não rodou** | código escrito; só o tema escuro foi visto |
| Digitar expressões nos campos (`2+3*4`) | **não rodou** | campo copiado e compilado; o caminho de confirmação tem teste |
| Editor na **sua sessão real** (RTX 3060) | **não rodou a janela** | `--gl-info` passou |
| Cliques sob **escala fracionária** (niri a 0,95) | **não se reproduziu** | reproduzido em niri aninhado a 0,95 (1920x1080 físico → 2021x1136 lógico), no Wayland e no XWayland: o clique chega em `físico ÷ 0,95`, exato, inclusive no canto direito; falta a sua sessão real, que o `--pointer-info` mede |
| Acessibilidade (leitor de tela, só teclado) | **não rodou** | |
| `cargo deny check` | **passou até 2026-10-25** (com avisos normais de duplicações transitivas) | `deny.toml` com `allow-wildcard-paths` e `RUSTSEC-2026-0192` justificado; `cargo-deny` não está instalado nesta revisão |
| `cargo nextest` | **não rodou** | `cargo-nextest` não está instalado; a suíte rodou com `cargo test` |
| `cargo xtask …`, regressão visual pixel a pixel | **não rodou / planejado** | |

## Como abrir (você roda)

```bash
cd ~/Documentos/Projetos/petunia-engine
scripts/editor.sh
```

Se o Wayland recusar o OpenGL, o editor reinicia sozinho via X11. Para diagnosticar: `scripts/editor.sh build` e depois `/tmp/petunia-editor-petunia-engine/petunia-editor --gl-info`.

Quando um **clique não responder**, rode o mesmo comando com `--pointer-info` e guarde o log (`2> ponteiro.log`): a cruz vermelha mostra onde o Qt recebeu o clique, e a linha `[escala]` mostra plataforma, telas e tamanho da janela ([detalhes](../02-arquitetura/11-editor.md#quando-um-clique-não-funciona---pointer-info)).

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
| **`i64::MIN` estourava** a validação de inteiro (panic em debug; aceito em release) | Revisão do código | Intervalo com `contains` em `petunia_world` e `petunia_script`; teste com os extremos |
| **Salvar durante um arrasto** gravava a prévia e deixava o arquivo "limpo" | Revisão do código | `save`, `undo` e `redo` recusam com `EDITOR_BUSY` enquanto há edição ao vivo; teste confere o arquivo |
| **Abrir outro projeto** mantinha gesto/drag presos ao mundo antigo | Revisão do código | `open_project` zera gesto, drag, escultura, componentes e andar; teste |
| **Inspetor de colisores** mostrava os valores da coluna anterior após o primeiro reload | Revisão do código | `refill` pula a coluna `owner` como o `rebuild` já fazia |
| Campos do inspetor podiam ser liberados antes de os widgets escondidos emitirem sinal | Revisão do código | Esconde primeiro, libera depois; `tearingDown_` bloqueia callbacks durante a troca |
| **Painel Lógica prometia um botão "Aplicar código" que nunca existiu** e o log soltava `QObject::connect(...invalid nullptr parameter)` a cada abertura | Leitura do código ao investigar o teste de fumaça (2026-10-30) | O `DR-40` aposentou a edição contínua: frase, `connect`, slot e membro removidos; o texto agora descreve só leitura |
| **`editor-smoke.sh` nunca abria o editor**: um comentário entre a barra de continuação e o `env` fazia o `env` imprimir o ambiente e sair | O teste reprovava com "a janela não apareceu em 25 s" | Comentário movido para cima do `env`, com aviso no arquivo; 13/13 |

## Limites conhecidos

Veja [editor](../02-arquitetura/11-editor.md). Os maiores: lista de objetos sem paginação, sem salvamento automático, cavar só em caixas sem rotação (use `mesh.boolean` para o resto), edição de malha com interface parcial (`Tab` e seleção de componentes; as operações ainda não têm botões).

## Linha paralela guardada: `archive/sistemas-de-mundo` (2026-10-29)

Existia uma **segunda implementação de terreno**, em outra cópia de trabalho, no branch
`feat/sistemas-de-mundo` (12 commits, nunca publicado). Ela **não** entra na `main` como está: são
dois terrenos que disputam os **mesmos nomes de comando** (`terrain.create`, `terrain.begin`,
`terrain.drag`, `terrain.end`, `terrain.cancel`, `terrain.paint`) com payloads diferentes, e o
princípio é **uma fonte de verdade por responsabilidade**. A `main` ficou com a linha dos rios
(terreno em `petunia_world`, `DR-36`); a outra virou a tag **`archive/sistemas-de-mundo`**
(commit `20975be`), publicada no remoto, e a cópia de trabalho dela ficou solta nesse commit para
consulta.

O que **só** a linha guardada tem, para portar item por item, cada um com a sua decisão:

| Capacidade | Onde está lá |
|---|---|
| Cinco biomas gerados da semente | `petunia_terrain` |
| Ler e gravar mapa de altura (`terrain.heightmap_import`, `heightmap_export`) | `petunia_terrain` + editor |
| Descarte de bloco fora da vista (corte por frustum) | `petunia_render/frustum.rs` |
| Terreno por camadas de textura, com grão de um atlas feito pelo motor | `petunia_render/texture.rs` |
| Varredura de perfil ao longo de uma spline (`spline.sweep`) com `petunia_curve` próprio | `petunia_curve` |
| Ver a caverna por raio X e cortar altura pelo cursor (`terrain.xray`, `terrain.section`) | `petunia_terrain` + editor |
| Erosão hídrica além da térmica (`terrain.erode_hydraulic`) | `petunia_terrain` |
| Carimbo como malha e buracos vindos das cavernas (`terrain.stamp_mesh`, `holes_from_caves`) | `petunia_terrain` |

Portar é **opcional e depois**: nada no caminho das fases 3 a 5 depende disso. O ganho maior está
em desempenho (frustum, textura) e em fluxo de trabalho (importar mapa de altura, biomas), não em
capacidade que falte à demo.

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
3. **Método não se troca** (decisão de 2026-10-30): escolhe-se ao criar e vale até o fim. Não há
   botão nem conversão entre métodos, e não há trabalho pendente nisso.
4. Rodar a **bateria restante** (`xtask`, regressão visual pixel a pixel quando implementadas).
