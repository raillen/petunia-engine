# Script Host (Aipo) e editor de código

**Revisão:** 2026-10-25 · Estado: **implementado em primeira versão** ([DR-25](../06-decisoes/registro-de-decisoes.md)). Código: `crates/petunia_script`, painel `editor/src/code_*.{h,cpp}`.

## Resumo (TL;DR)

Um projeto tem scripts `.aipo` na pasta `scripts/`. Um objeto liga a um script pela tabela `Script`. O motor chama `on_start`, `on_update(entity, dt)` e `on_event(entity, nome)`. Os scripts **mudam o mundo só por comandos validados**, sem acesso ao computador. O editor tem um **editor de código** (F4) que usa o **mesmo compilador** do jogo.

## Exemplo

`scripts/porta.aipo` (ligado a um objeto):

```aipo
fn on_start(entity) {
    log.info("começou", entity)
}

fn on_update(entity, dt) {
    let position = world.get(entity, "Transform", "position")
    world.set(entity, "Transform", "position", { "x": position["x"] + dt, "y": position["y"], "z": position["z"] })
}
```

## O que um script enxerga

| Módulo | Funções | Permissão |
|---|---|---|
| `world` | `get`, `row`, `has`, `exists`, `find` | `world.read` |
| `world` | `set`, `create`, `remove` | `world.write` |
| `game` | `tick`, `seconds`, `emit` | `game` |
| `log` | `debug`, `info`, `warning`, `error` (e `print`) | nenhuma |
| `transform` | posição, rotação, escala, mover, girar, frente/direita/cima | `world.read` / `world.write` |
| `character` | `grounded` | `world.read` |
| `character` | `move` (velocidade no plano do chão), `jump` | `world.write` |
| `collision` | `list`, `get`, `overlaps` | `world.read` |
| `collision` | `set` (muda uma coluna de uma colisão; a física se refaz no quadro seguinte) | `world.write` |
| `physics` | `raycast`, `raycast_from` (ignora o próprio objeto), `velocity` | `world.read` |
| `physics` | `set_velocity`, `impulse` | `world.write` |
| `grid` | `square`, `hex`, `distance`, `neighbors`, `range`, `path`, `to_world`, `from_world` | nenhuma |
| biblioteca padrão da Aipo | `math`, `string`, `json`, listas, dicionários, `random` com semente, `time` (sem relógio), `task`… | nenhuma |

A tabela está em **um lugar só** (`api.rs`) e gera o que o compilador aceita, as funções nativas e a ajuda do editor; um teste confere que as três coincidem. Um erro como `world.fnd` aparece **no editor, antes de jogar**.

### Colisão e física (DR-27)

```aipo
fn on_update(entity, dt) {
    // Olha 1,2 m para baixo a partir do próprio objeto (as colisões dele são ignoradas).
    let chao = physics.raycast_from(entity, {"x": 0.0, "y": -1.0, "z": 0.0}, 1.2)
    if game.tick() % 60 == 0 {
        log.info("colisões", collision.list(entity))
    }
}

fn on_collision(entity, outro, minha, dele) {
    if minha == "pés" {
        physics.impulse(entity, {"x": 0.0, "y": 4.0, "z": 0.0})
    }
}
```

- Um objeto pode ter **várias colisões com nome**; o script as acha pelo nome (`"pés"`, `"corpo"`).
- `on_collision(entity, outro, minha, dele)` e `on_collision_end(...)` são chamados quando duas colisões começam ou deixam de se tocar (gatilhos também).
- **Personagem** (`Character`): `character.move(entity, x, z)` manda andar em m/s; o motor desvia de paredes, sobe degraus e rampas e cola no chão ao descer. `character.jump(entity)` pula (vale também por um instante depois de sair de uma borda: **tempo de coiote**).
- Apagar um objeto com `world.remove` leva as colisões dele; mudar `Collider` ou `RigidBody` com `world.set` refaz a física no quadro seguinte.
- Sem colisões ou corpos no mundo, `physics.*` e `collision.overlaps` devolvem uma falha que explica: "a física não está ligada".

### Grade e caminho (`grid`, DR-38)

A grade é **dado**, não um objeto do motor: `{"kind": "square"|"hex", "size": metros}`. A célula também: `{"q": Int, "r": Int}`. Um script cria a grade uma vez e passa adiante; ela cabe numa tabela, num arquivo e na rede.

```aipo
fn on_start(entity) {
    var grade = grid.square(1.0)               // hex(1.0) para hexagonal pontuda
    let eu = grid.from_world(grade, transform.get_position(entity))
    let alvo = { "q": 4, "r": 2 }
    let paredes = [{ "q": 2, "r": 0 }, { "q": 2, "r": 1 }]
    attempt {
        let caminho = grid.path(grade, eu, alvo, paredes)?
        log.info("passos", len(caminho))
    } failed erro {
        log.warning("sem caminho:", erro)
    }
}
```

- **Quadrada**: eixos `q` e `r`, quatro vizinhas, a diagonal custa **dois** passos (`distance = |Δq| + |Δr|`).
- **Hexagonal**: coordenadas cúbicas, `s = -q - r`, seis vizinhas e `distance = (|Δq| + |Δr| + |Δs|) / 2`.
- `grid.range(cell, raio)` devolve a própria célula e tudo até `raio` passos, sempre na mesma ordem (por `r`, depois `q`): a mesma semente dá a mesma lista.
- `grid.path` é A\* com a distância exata de heurística; o primeiro caminho é o mais curto. Devolve `none` (e não falha) quando o começo ou o destino está bloqueado, ou quando não há passagem.
- `grid.to_world` dá o centro da célula no chão (`y = 0`); `grid.from_world` ignora o `y`.
- **Limites**: coordenada até ±1.000.000, célula de 0,001 a 10.000 m, raio até 256, 65.536 células bloqueadas e 65.536 expandidas por busca — acima disso é falha, não travamento.

## O que um script **não** pode

| Módulo | Por quê | Como |
|---|---|---|
| `sh` (rodar programas) | **Sem checagem de permissão na Aipo** (achado desta etapa) | **Removido** da VM, e não só negado |
| `fs`, `env`, `io` | Ler arquivos e ambiente, escrever na saída do processo | Removidos |
| Relógio do sistema | Deixaria o jogo não determinístico | Negado até o host instalar um |

Há um teste que tenta `sh.run`, `fs.read_text`, `env.get` e `io.println` e confirma que **nenhum compila**, e que um arquivo-isca não é criado.

## Como roda

- **Uma VM por anexo** (objeto + script), isolada das outras. O módulo compilado é compartilhado, mas o estado de topo é executado uma vez por objeto, então `var` de módulo não vaza de um objeto para outro. Os anexos só compartilham o mundo.
- **Orçamento por chamada**: 100 mil instruções (cerca de 10 ms). Passou disso, o script é interrompido com `SCRIPT_BUDGET_EXCEEDED`, dizendo o limite e o que fazer.
- **Falha desliga só aquele objeto**: o motor registra uma linha no console e não chama de novo, em vez de repetir o erro 60 vezes por segundo. Salvar o arquivo corrigido religa.
- **Falha não tratada** (`world.set` recusado sem `attempt`) conta como falha do script, e não passa por sucesso. Tratada com `attempt { ... } failed erro { ... }`, segue.
- **Todo comando passa pelo despachante**: mesma validação do editor, de macros e da rede.
- **Determinismo**: mesma versão do script no mesmo mundo dá o mesmo resultado (teste).
- **Sem variável global**: o estado do jogo chega às funções nativas por um handle com geração dentro da VM.

## Onde usar

| Lugar | O que acontece |
|---|---|
| `petunia check <pasta>` | Lista erros de script com **linha e coluna**; sai com código 1 |
| `petunia run <pasta>` | Os scripts rodam todo quadro; o console vai para o terminal |
| Editor, aba **Código** (`F4`) | Escrever, checar enquanto digita, formatar, salvar, executar, ligar ao objeto |
| Painel **Diagnósticos** | Mostra os erros de script e objetos que apontam para script que não existe (`SCRIPT_MISSING`) |

## O editor de código

| Recurso | Como |
|---|---|
| Erros em tempo real | O compilador de verdade, 450 ms depois de parar de digitar; sublinhado ondulado, **ponto na margem** (forma, não só cor) e lista de Problemas |
| Destaque de sintaxe | Palavras-chave da Aipo (vêm do analisador léxico dela), strings, números, comentários e módulos do motor |
| Sugestões | `Ctrl+Espaço`, ou ao digitar; depois de `world.` mostra só os membros do módulo, com assinatura e exemplo |
| Recuo | Automático depois de `{`; `Tab` recua 4 espaços; `Shift+Tab` desfaz |
| Acessibilidade | **`Esc` e depois `Tab` sai do editor** (o teclado nunca fica preso); nomes acessíveis em tudo |
| Executar (`F5`) | Roda numa **cópia** do mundo, N quadros; mostra o console e quantos objetos mudariam. **A cena não muda** |
| Salvar (`Ctrl+S` dentro do editor) | A versão anterior vai para `scripts/.backup/` |
| Formatar (`Ctrl+Shift+F`) | O formatador da Aipo; só se o texto compila na sintaxe; um passo de desfazer |

## O que falta (honesto)

| Falta | Nota |
|---|---|
| **Event Sheet e Gameplay IR** ("regras sem código") | Previsto na fase 3; não existe |
| **Luau** como segunda linguagem | Decisão adiada para o experimento S2 |
| Servidor de linguagem (LSP): ir à definição, renomear | Hoje: destaque, erros e sugestões |
| **Recarregar scripts com o jogo rodando** no `petunia run` | Hoje só o editor recarrega; o jogo lê na abertura |
| Limite de **memória** por script | Só há limite de instruções |
| Manifesto e aprovação de **mods** (permissões por mod) | O conjunto de permissões é fixo |
| Eventos de **entrada** (tecla) | A colisão já existe (`on_collision`); entrada depende do controlador |
| Depurador (pontos de parada, passo a passo) | Fase 8 |
| Mensagens do compilador **em português** | Vêm em inglês da Aipo |
| A Aipo como **dependência por caminho** (`../aipo-lang`) | Para distribuir, vira dependência fixada por commit |
