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
| biblioteca padrão da Aipo | `math`, `string`, `json`, listas, dicionários, `random` com semente, `time` (sem relógio), `task`… | nenhuma |

A tabela está em **um lugar só** (`api.rs`) e gera o que o compilador aceita, as funções nativas e a ajuda do editor; um teste confere que as três coincidem. Um erro como `world.fnd` aparece **no editor, antes de jogar**.

## O que um script **não** pode

| Módulo | Por quê | Como |
|---|---|---|
| `sh` (rodar programas) | **Sem checagem de permissão na Aipo** (achado desta etapa) | **Removido** da VM, e não só negado |
| `fs`, `env`, `io` | Ler arquivos e ambiente, escrever na saída do processo | Removidos |
| Relógio do sistema | Deixaria o jogo não determinístico | Negado até o host instalar um |

Há um teste que tenta `sh.run`, `fs.read_text`, `env.get` e `io.println` e confirma que **nenhum compila**, e que um arquivo-isca não é criado.

## Como roda

- **Uma VM por arquivo**, isolada das outras. Elas só compartilham o mundo.
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
| Eventos de entrada (tecla, gatilho) e colisão | Dependem da física e do controlador |
| Depurador (pontos de parada, passo a passo) | Fase 8 |
| Mensagens do compilador **em português** | Vêm em inglês da Aipo |
| A Aipo como **dependência por caminho** (`../aipo-lang`) | Para distribuir, vira dependência fixada por commit |
