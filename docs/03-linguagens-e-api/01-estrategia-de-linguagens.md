# Estratégia de linguagens (Aipo + Luau)

**Revisão:** 2026-10-24 · **Estado:** parcialmente decidido antes (uma linguagem ativa por projeto); depende de S1 e S2

## Resumo (TL;DR)

- **Aipo é a linguagem de primeira classe e o padrão.** Documentação, exemplos e editor nascem nela.
- **Luau é a alternativa**, escolhida **na criação do projeto**. Serve a quem já conhece Lua/Roblox ou quer tipos graduais.
- **Uma linguagem ativa por projeto na V1** (decisão herdada da [especificação anterior](../01-pesquisa/05-especificacao-anterior.md)). Isso corta metade da manutenção e da documentação.
- As duas falam com a engine pelo **mesmo contrato** (schema da API) e pelas **mesmas permissões**. Não existem duas APIs.
- As **telas visuais** (Event Sheet, grafos) funcionam nas duas, porque vêm do Gameplay IR ([nós](04-programacao-em-nos.md)).
- A engine **nunca** adiciona palavra-chave de jogo à Aipo. Recursos de jogo são **bibliotecas** (decisão herdada do Poppy Pivot, `LOCAL`).

## Quadro comparativo

| | **Aipo** | **Luau** |
|---|---|---|
| Papel | Primeira classe (padrão do projeto) | Alternativa por projeto |
| Origem | Seu projeto, em Rust | Open source (Roblox), em C++ |
| Escrita | Pensada para neurodivergentes; uma forma só; palavras inteiras | Familiar a quem vem de Lua; mais flexível |
| Tipagem | Dinâmica forte com contratos opcionais | **Gradual**: `nocheck`, `nonstrict`, `strict` por arquivo (`WEB`) |
| Embutir em Rust | Nativo (`aipo-vm`) | `mlua` com a feature `luau` (`WEB`) |
| Sandbox | Orçamento de instruções + capabilities (`LOCAL`) | `Lua::sandbox`, globais somente leitura, `set_interrupt` para limite de CPU (`WEB`) |
| Vetores | **Faltam** (planejados como valor do host) | Tipo `vector` nativo (`WEB`) |
| Ferramentas | `aipo check/fmt/test`; **sem LSP** | `luau-lsp` com definições customizadas (`WEB`) |
| Telas visuais (Event Sheet, grafos) | **Sim**, via Gameplay IR | **Sim**, via Gameplay IR |
| Maturidade | Jovem; rodadas em rascunho (`LOCAL`) | Madura; JIT nativo em `mlua` ainda **experimental** (`WEB`) |

## Por que Luau como segunda (e não Lua 5.4)

O Petunia3D usa **Lua 5.4** via `mlua` (`LOCAL`). Trocar para **Luau** é, em tese, só mudar a feature do `mlua` (`HIPÓTESE`, validar em S2):

| Ponto | Luau | Lua 5.4 |
|---|---|---|
| Tipos | Graduais, com análise estática | Nenhum |
| Sandbox | Pensado para isso (`WEB`) | Precisa ser montado à mão |
| Vetores | Nativos | Não |
| Público | Comunidade enorme (Roblox) | Ampla |
| Custo | Dependência C++ no build | Dependência C no build |

**Recomendação:** Luau como alternativa. A especificação anterior já decidiu "Luau ou Aipo". As opções realistas, em ordem: Luau; Lua 5.4 (já funciona no Petunia3D); só Aipo. "Só Aipo" simplifica muito e deve voltar à mesa se o experimento S2 mostrar custo alto (`DR-06`).

## O contrato único: schema da API da engine

### Ideia
Cada função da engine é descrita **uma vez**, como dado. O schema gera tudo:

| Saída | Para quê |
|---|---|
| Bindings Aipo | Chamar a engine em Aipo |
| Arquivo de definições Luau (`.d.luau`) | Tipos e autocomplete em Luau |
| Catálogo de nós | Cada função = um nó |
| Autocomplete e ajuda | Editor de código |
| Documentação | Site e ajuda embutida |
| Ferramentas MCP | Agentes de IA |
| Validação de comandos | CLI e rede |

### De onde vem
Já existem **duas** metades (`LOCAL`):

| Metade | O que tem | Onde |
|---|---|---|
| **Host ABI da Aipo** | `HostSchema`, `FunctionSchema`, `TypeSchema`, `HandleSchema`, permissões, valores, falhas com código | `aipo-host` |
| **Comandos do Petunia3D** | `CommandIntent` + JSON Schema; descritores de nós com parâmetros tipados | `schema_contracts.rs`, `op_graph.rs`, `RecipeParamSpec` |

**Proposta:** **estender o AHS** (que é seu e é MIT) para o que falta: unidades, faixas, valores padrão, exemplo, custo (barato / com orçamento), efeito no undo e "como aparece como nó". Isso evita criar um terceiro formato.

### O que cada entrada do schema precisa ter

| Campo | Exemplo |
|---|---|
| Nome e módulo | `rope.create` |
| Resumo em uma linha | "Cria uma corda entre duas âncoras." |
| Parâmetros | nome, tipo, unidade, faixa, padrão |
| Retorno | Handle `Rope` |
| Falhas | códigos estáveis + sugestão de correção |
| Permissão exigida | `world.write`, `physics.ropes` |
| Custo | `barato` · `orçado` (conta no orçamento de física) |
| Desfazer | "desfaz no editor", "não se aplica no jogo" |
| Exemplo | Código curto, testado |
| Aparência de nó | Rótulo, cor, ordem dos pinos |

## Permissões (uma só, para as duas linguagens)

O `aipo-host` **nega por padrão** e só permite **reduzir** permissões, nunca ampliar (`LOCAL`). O Petunia3D tem manifesto de capabilities para Lua com a mesma ideia (`LOCAL`). Unificar:

| Contexto | Permissões típicas |
|---|---|
| Script do jogo | `world.read`, `world.write`, `physics`, `ui`, `sound`, `net.share` |
| Script de editor/ferramenta | + `editor.command`, `assets.import` |
| **Mod ou script de terceiros** | Só o que o manifesto pede **e** o jogador aprova |
| Agente de IA (MCP) | Política própria; tudo passa pelo despachante de comandos (`LOCAL`, `mcp/`) |

Sem acesso a arquivos, rede ou processos por padrão.

## Limites de execução

| Limite | Fonte |
|---|---|
| Orçamento de instruções por quadro e por chamada | Aipo: `AIPO_RT_BUDGET_EXCEEDED` (`LOCAL`); Luau: `set_interrupt` (`WEB`) |
| Teto de memória por script | Petunia3D: 8 MiB (`LOCAL`); Luau: limite no host (`WEB`) |
| Tamanho do script | Petunia3D: 64 KiB (`LOCAL`) |
| Intenções por execução | Petunia3D: 256 (`LOCAL`) |

Os valores finais saem de medição (S1, S2).

## Contrato de desempenho: scripts mandam, a engine executa

A Aipo tem um piso de ~100 ns por instrução (`LOCAL`, ADP-011). Portanto:

1. **API em lote.** `world.set_all(...)`, `world.find(...)`. O laço roda em Rust.
2. **Regras reagem a eventos**, não varrem o mundo todo a cada quadro.
3. **Matemática pesada fica na engine.** Rios, estradas, clima, vegetação, cordas e física são nativos.
4. **Se um script ficar lento**, o editor mostra um **perfil de custo por script** e sugere a chamada em lote.
5. **Saída de emergência:** compilar a lógica para WebAssembly com JIT (`aipo-wasm`, `LOCAL`).

## Integração de tempo

| Mecanismo da Aipo | Na engine |
|---|---|
| `task.sleep` em tempo **virtual** | Mapeia para o **tempo de jogo** (respeita pausa e câmera lenta), não o relógio de parede |
| `async fn` / `await` | Permitido em regras e cutscenes simples |
| Falha em `attempt { } failed { }` | Mutações do script são desfeitas; evento de erro vai ao console do editor |

## Recarregar código (hot reload)

Meta: **recarregar scripts e preservar o estado serializável** (as tabelas). O Poppy Pivot já escolhe esse caminho em vez de preservar o heap da VM (`LOCAL`). A Aipo tem "reload transacional" em rascunho. Como o estado do jogo é **só tabelas**, a migração é natural.

## Lacunas da Aipo que bloqueiam a engine

Lista completa em [aproveitamento da Aipo](../01-pesquisa/02-aproveitamento-aipo-lang.md). As cinco que mandam:

1. Tipos vetoriais com valor (`Vec2`, `Vec3`, `Quat`, `Color`).
2. Medição de desempenho do ponto de vista de jogo.
3. LSP.
4. Depurador e perfil.
5. Mensagens de diagnóstico em português.

## Decisões ligadas

`DR-06` linguagens · `DR-07` nós · `DR-15` qualidade.
