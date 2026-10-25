# Online

**Revisão:** 2026-10-24 · **Estado:** proposta; precisa do experimento S7

## Resumo (TL;DR)

Online deve ser **"criar sala, passar código, jogar"**. Quem cria o jogo **marca quais tabelas são compartilhadas**. A engine cuida do resto: conexão, réplica, interpolação e teste com vários jogadores com **um clique** no editor.

O desenho começa **simples e seguro** (servidor com autoridade) e deixa a porta aberta para *rollback* em modos competitivos.

## Níveis (cada um funciona sem o seguinte)

| Nível | O que é | Quando |
|---|---|---|
| **0 — Local** | Sem rede | Sempre |
| **1 — Sala** | Um jogador é o **anfitrião**; os outros entram por **código** | **Versão 1** |
| **2 — Servidor dedicado** | O mesmo jogo, executado **sem tela** (`--headless`) | Depois |
| **3 — Rollback** | Para modos rápidos e pequenos; exige simulação determinística | Só se houver demanda |

## O que o criador faz

```aipo
# Ilustrativo (não é contrato)
net.share(table = "Door", fields = ["open"])      # compartilhar esta tabela
net.share(table = "Player", fields = ["position", "health"], owner = "player")

let sala = net.host(max_players = 4)              # devolve um código curto
ui.say(f"Código da sala: {sala.code}")

net.join(code = "K7P2QX")                         # outro jogador
```

No editor existe o botão **"Testar online"**: abre **N clientes locais + 1 servidor** e um painel com **latência e perda de pacotes simuladas**.

## Como funciona (nível 1)

| Tema | Proposta |
|---|---|
| **Autoridade** | O servidor decide. Clientes mandam **entradas** (ações), recebem **estado** |
| **O que replica** | **Tabelas marcadas** como compartilhadas, só as colunas pedidas, **só as mudanças (delta)** |
| **Suavização** | Interpolação de outros jogadores; **predição + reconciliação** só do próprio personagem |
| **Interesse** | Cada cliente recebe só os **chunks** perto dele |
| **Identidade** | Sem conta obrigatória: identidade anônima por sessão; conta é opcional |
| **Código de sala** | Curto e sem ambiguidade (sem `0/O`, `1/I`) |
| **Segurança** | Servidor valida tudo. Scripts de clientes **não** têm autoridade |
| **Sala sem anfitrião** | Se o anfitrião sai, a sala encerra ou migra (decisão aberta) |

O Godot segue o mesmo caminho: cliente-servidor com autoridade, `MultiplayerSpawner` e `MultiplayerSynchronizer`, e a doc oficial avisa que isso **não torna o jogo seguro sozinho** (`WEB`).

## Transporte (`DR-09`)

| Opção | Pontos fortes | Pontos fracos (`WEB`) |
|---|---|---|
| **A. Réplica própria + `renet`** | `renet` 2.0 cuida de conexão e mensagens; réplica sob medida para tabelas | `renet` não replica ECS sozinho |
| **B. `naia`** | Replica campos alterados; UDP no nativo e WebRTC no navegador | Modelo próprio de entidades |
| **C. `matchbox` (WebRTC P2P)** | Navegador e nativo; tem soquete compatível com `ggrs` (rollback) | Peer-to-peer; sinalização precisa de servidor |

`lightyear` é feito para o Bevy e depende do ECS dele (`WEB`), então **não serve** a um armazenamento próprio.

**Recomendação:** **A** para o nível 1; **C** como transporte alternativo para web e P2P. Validar em S7.

### O problema do "relay"
Jogar com amigos atrás de roteadores exige **atravessar NAT**. Isso pede um **servidor intermediário** (relay/sinalização). Quem hospeda e paga é uma **decisão de produto**:

1. Serviço mantido por você (custo e manutenção).
2. Serviço do próprio criador do jogo (autohospedado).
3. Plataforma (Steam etc.), presa à loja.

Recomendação: começar com **autohospedado + um servidor de sinalização pequeno incluído**, sem login.

## Acessibilidade e segurança online

| Tema | Proposta |
|---|---|
| **Chat** | **Frases prontas e emotes por padrão** (chat por símbolos é recomendado, `WEB`). Texto livre é **opcional e desligado por padrão** |
| **Silenciar/bloquear** | Sempre disponível, a um clique |
| **Ritmo** | Salas com **modo calmo**: sem tempo limite obrigatório, tolerância maior a latência |
| **Pareamento** | Opção de jogar com quem **usa recursos de assistência** (`WEB`, Game Accessibility Guidelines) |
| **Privacidade** | Sem conta, sem dados pessoais obrigatórios |

## Determinismo e rollback (nível 3)

- A Aipo tem **tempo virtual determinístico** (`LOCAL`). O README do `aipo.game` fala em *rollback nativo* (`DECLARADO`, **não verificado**).
- Rollback exige guardar e restaurar o estado de **tudo** que influencia a simulação: tabelas, física, cordas e sementes.
- A física do Rapier tem modo determinístico, mas o resultado entre plataformas é **meta**, não garantia (`HIPÓTESE`).
- Um projeto da comunidade relata ter escolhido o Rapier por determinismo para rollback e depois migrado para o Jolt quando deixou de reexecutar quadros (`WEB`, fonte secundária). Ou seja: não há consenso.

**Decisão:** não prometer rollback na versão 1.

## Dedicado e web
- Servidor sem tela: o **mesmo binário** do jogo, sem janela nem renderer. Só funciona se o domínio não depender de GPU (regra 1 da arquitetura).
- Web: possível via WebRTC/WebSocket, **depois** (a saída `aipo-wasm` e `aipo-js` ajudam, `LOCAL`). Fora da versão 1.

## Decisões ligadas

`DR-09` online · `DR-05` modelo de dados (a réplica depende das tabelas).
