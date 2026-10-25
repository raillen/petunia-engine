# Princípios da API

**Revisão:** 2026-10-24 · **Estado:** proposta

## Resumo (TL;DR)

A API deve poder ser **lida em voz alta**. São 12 regras. Cada função nova passa por uma **lista de verificação** antes de entrar.

Elas valem para Aipo, Luau e nós, porque saem do mesmo schema.

## Exemplo: antes e depois

```aipo
# Antes (protótipo atual, LOCAL): números mágicos, abreviações, estado solto
if host_key_down(262) or host_key_down(68) { player_vx = player_vx + accel * dt }

# Depois (proposta, ilustrativo): ação com nome, palavras inteiras
if input.held("move_right") {
    world.set(player, velocity = world.get(player, "velocity") + accel * time.step)
}
```

## As 12 regras

| # | Regra | Por quê |
|---|---|---|
| 1 | **Verbos primeiro, palavras inteiras.** `create`, `remove`, `get`, `set`, `find`, `play`, `show`, `hide` | Leitura sem decodificar |
| 2 | **Uma forma só para cada coisa.** Sem apelidos | Herdado da Aipo |
| 3 | **Substantivos do criador de mundos:** `river`, `road`, `rope`, `weather`, `plant`, `decal` | O menu de autocomplete vira um dicionário do domínio |
| 4 | **Argumentos nomeados sempre.** No máximo 2 posicionais; no máximo ~7 no total | Aipo já tem argumentos nomeados |
| 5 | **Valores padrão bons.** A chamada mínima já funciona e fica bonita | O "primeiro minuto" |
| 6 | **Dados entram, dados saem.** Registros e listas simples. Referências a coisas do mundo são **handles opacos** | Sem objetos com métodos escondidos |
| 7 | **Unidades fixas e escritas na doc:** metros, segundos, graus, 0–1 para proporções | Zero ambiguidade |
| 8 | **Nada acontece "à distância".** Mudanças estruturais só em pontos seguros (`CommandBuffer`) e a doc diz quando | Previsível |
| 9 | **Erro diz o que fazer.** Posição + nome do problema + correção + "Quis dizer…?" | Regra 4 da Aipo |
| 10 | **Nada falha em silêncio.** Handle obsoleto, permissão negada e valor inválido são erros com código | `aipo-host` já faz (`LOCAL`) |
| 11 | **Módulos pequenos:** no máximo ~12 funções por módulo; sem encadeamento profundo | Carga cognitiva |
| 12 | **Todo recurso tem exemplo testado.** O exemplo da doc roda nos testes | A doc nunca mente |

## Lista de verificação para cada função nova

- [ ] O nome é um verbo + substantivo que alguém lê em voz alta?
- [ ] Aparece na ordem certa no autocomplete (por substantivo)?
- [ ] A chamada mínima, só com o necessário, funciona?
- [ ] Os padrões produzem um resultado bom?
- [ ] Há versão **em lote**, se for chamada em muitos itens?
- [ ] A unidade de cada número está escrita?
- [ ] Os erros têm código estável e uma sugestão?
- [ ] Diz o custo (barato ou orçado) e o efeito no desfazer?
- [ ] Tem exemplo curto, testado?
- [ ] Vira um nó legível (rótulo, pinos, cor)?
- [ ] Respeita permissões (qual capability pede)?

## Dicionário de verbos (o conjunto fechado)

Um conjunto **pequeno e fixo** facilita aprender e prever.

| Verbo | Significa | Exemplo |
|---|---|---|
| `create` | Faz algo novo e devolve um handle | `rope.create(...)` |
| `remove` | Apaga | `decal.remove(d)` |
| `get` / `set` | Lê / escreve dados | `world.get(e, "health")` |
| `find` | Procura por critério | `world.find(tag = "Tree")` |
| `on` | Reage a um evento | `rules.on("touch", ...)` |
| `play` / `stop` | Som, animação, efeito | `sound.play("porta")` |
| `show` / `hide` | Visibilidade | `ui.show("menu")` |
| `move` / `turn` | Movimento relativo | `camera.move(...)` |
| `share` | Marca para a rede | `net.share(...)` |

Novo verbo = decisão registrada. Sinônimos são recusados.

## Nomes de módulos (substantivos)

`world`, `input`, `rules`, `time`, `camera`, `physics`, `rope`, `river`, `road`, `plant`, `weather`, `decal`, `sound`, `ui`, `net`, `save`, `access` (acessibilidade).

## O que a API **não** faz

| Evitado | Alternativa |
|---|---|
| Classes e herança | Tabelas e regras |
| Callbacks de ciclo de vida obrigatórios (`on_create`, `on_update`) | Regras por evento; `time.every(...)` |
| Códigos de tecla (`262`) | Ações nomeadas (`"move_right"`) |
| Máscaras de bits | Nomes de camada |
| Estado global escondido | Tabelas explícitas |
| Funções com 10 parâmetros | Registro nomeado ou várias chamadas pequenas |
| Abreviações (`pos`, `vel`, `rot`) | `position`, `velocity`, `rotation` |

## Mensagem de erro (modelo)

```text
Erro ROPE_ANCHOR_STALE  (arquivo ponte.aipo, linha 12)

  A âncora "poste_a" foi removida do mundo.

  Como corrigir:
    • Crie a corda depois de criar o poste.
    • Ou use world.exists(poste_a) antes de criar a corda.
```

Cada erro tem **código estável**, texto em **português** e uma **ação**.

## Documentação da API

Cada página tem a mesma forma (para quem tem TDAH ou dislexia, previsibilidade é leitura mais fácil):

1. **Uma linha:** o que faz.
2. **Exemplo mínimo.**
3. **Parâmetros** (tabela curta).
4. **Erros** (código + correção).
5. **Veja também** (máximo 3 links).

## Decisões ligadas

`DR-06` linguagens · `DR-07` nós · `DR-15` testes dos exemplos.
