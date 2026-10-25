# O que aproveitar da Aipo

**Revisão:** 2026-10-24 · **Fonte:** leitura direta de `~/Documentos/Projetos/aipo-lang` (`LOCAL`)

## Resumo (TL;DR)

A Aipo já é **quase tudo que a engine pede de uma linguagem**: sintaxe feita para neurodivergentes, imutável por padrão, sem classes, erros que ensinam, VM em Rust, schema de host com permissões e um embrião de engine de jogos (`aipo-poppy`).

Faltam, para uso em jogos 3D em tempo real: **tipos vetoriais**, **desempenho medido**, **LSP** e **depurador**. A seção "Lacunas" lista tudo.

## A linguagem em 30 segundos

```aipo
struct Servidor {
    id: Int                        # imutável por padrão
    var status: String = "online"  # var = pode mudar
}

fn saudacao(nome: String) -> String {
    return f"Olá, {nome}!"
}

match estado
    when Estado.Ativo { desde } { io.println(f"ativo desde {desde}") }
    else { io.println("outro") }
```

Pontos que combinam com a engine (`LOCAL`, `SYNTAX.md`):

| Regra da Aipo | Por que serve à engine |
|---|---|
| Uma forma só para cada coisa | API previsível; projeção em nós sem ambiguidade |
| Palavra completa vence símbolo (`and`, `not`) | Leitura fácil para dislexia e TDAH |
| `struct` só tem dados; comportamento fica em `Tipo:nome` | Combina com "dados, não classes" |
| Interface estrutural automática (sem declarar) | Menos cerimônia |
| `attempt { } failed { }` com **rollback automático** das mutações | Seguro para scripts de jogador e de editor |
| Falhas recuperáveis separadas de erros de programação | Jogo não cai por erro de script |
| Async cooperativo com **tempo virtual determinístico** | Bom para simulação repetível e online |
| Diagnósticos com código estável e sugestão | Editor de código amigável |
| 41–43 palavras-chave; sem herança, sem generics, sem bitwise | Pequena de aprender |

## Peças reaproveitáveis (tamanhos medidos em linhas de Rust, `LOCAL`)

| Crate | Linhas | O que é | Decisão |
|---|---|---|---|
| `aipo-lexer`, `aipo-syntax`, `aipo-ast`, `aipo-hir`, `aipo-sema`, `aipo-ir`, `aipo-bytecode` | ~21 mil | Frente de compilação até bytecode | **REUSAR** como dependência |
| `aipo-vm` | 19 mil | VM de pilha, orçamento de instruções (`AIPO_RT_BUDGET_EXCEEDED`) | **REUSAR** — com medição (veja Lacunas) |
| `aipo-stdlib` | 14,5 mil | Biblioteca padrão portátil | **REUSAR** (filtrar o que um jogo precisa) |
| `aipo-wasm` | 13 mil | Compila para WebAssembly; execução com JIT via Wasmtime | **REUSAR** como saída de desempenho |
| `aipo-js` | 2 mil | Emissor de JavaScript | Opcional (exportar para web) |
| `aipo-formatter` | 0,9 mil | Formatador canônico | **REUSAR** — base do "texto ⇄ nós" |
| `aipo-package` | 6,7 mil | Pacotes com SHA de commit, lockfile, cache offline | **REUSAR** — compartilhar módulos e mods com segurança |
| `aipo-c-abi` | 4 mil | Embutir em C | Opcional |
| `aipo-cli` | 13 mil | `run`, `check`, `fmt`, `test`, `build` | **REUSAR** — ferramentas de linha de comando |
| `aipo-host` | 2,2 mil | **Host ABI**: schema (AHS), capabilities, `HostValue`, handles geracionais, `HostFault` | **REUSAR e ESTENDER** — centro do desenho |
| `aipo-poppy` | 1,7 mil | Perfil de engine de jogos: `World`, `CommandBuffer`, `Simulation` fixa, `PoppyRng` | **ADAPTAR** — vira o perfil "petunia" |
| `aipo-game-host` | 5,5 mil | Protótipo 2D (macroquad + egui) | **INSPIRAR** só |
| `packages/aipo-game` | — | Pacote 2D: Atores, Cenas, Behaviors, Tilemap, UI imediata, Partículas, **nós Gatilho→Filtro→Ação** | **INSPIRAR** |
| `packages/aipo-ui`, `aipo-egui`, `aipo-zoe` | — | UI em Aipo | Avaliar para UI dentro do jogo |

## O ativo mais importante: `aipo-host`

O comentário de abertura do crate resume (`LOCAL`): *"o compilador nunca aprende o que é um host"*. O host descreve sua API **como dado** e entrega ao script só valores simples e handles.

Três regras do crate:

1. **Nega por padrão.** Um perfil não concede nada sem ordem explícita. Permissões só podem ser **reduzidas**, nunca ampliadas (`CapabilitySet::narrow`).
2. **Nunca usar estado liberado.** Handle liberado vira "obsoleto" (stale), não aponta para outro objeto que reutilizou a vaga.
3. **Problema do host é falha, não pânico.** Cada recusa tem código estável.

Isso é exatamente o que a engine precisa para: scripts de jogadores, mods, plugins e agentes de IA. Veja [estratégia de linguagens](../03-linguagens-e-api/01-estrategia-de-linguagens.md).

## O ativo mais promissor: `aipo-poppy`

Documento de decisão (`LOCAL`, `docs/canon/…Poppy Pivot…`, aprovado 2026-09-15):

- Aipo mantém **um núcleo só**. Jogo, web e script são **perfis de host**, não dialetos.
- Recursos de domínio entram como **API de host/biblioteca**, nunca como palavra-chave nova (`behavior`, `component`, `system` ficam de fora).
- Valores pequenos do host (`Vec2`, `Vec3`, `Quat`, `Color`, `Rect`, `Transform`) **copiam por valor**.
- Objetos externos (`Entity`, `Asset`, `Texture`, `Scene`) são **handles geracionais**.
- Mudanças estruturais no mundo vão para um **`CommandBuffer`** e só são aplicadas em pontos seguros.
- Bindings, autocomplete e docs vêm do **mesmo registro semântico**. "Evitar listas manuais paralelas."

A Petunia Engine é a **concretização 3D** dessa ideia. **Pergunta aberta:** a "Poppy Game Engine" citada nesses documentos é outro projeto seu, que a Petunia Engine substitui, ou convivem? A [especificação anterior](05-especificacao-anterior.md) não menciona Poppy; fala em Petunia Game Engine 3D e em `ScriptHost` com backends Luau e Aipo. (`DR-11`)

## Lacunas para jogo 3D em tempo real

| # | Lacuna | Evidência (`LOCAL`) | Impacto |
|---|---|---|---|
| 1 | **Desempenho** | ADP-011: "piso de ~100 ns por instrução"; `Value` de 48 bytes, plano de reduzir para 16–24; comparação com Luau mostrou diferença | Script de jogo precisa ser **em lote** (veja abaixo) |
| 2 | **Sem tipo vetorial** nativo | `Vec3` planejado como "host value"; hoje listas e dicionários | Matemática 3D desajeitada |
| 3 | **Sem LSP** | Planejado na lista de crates do Poppy Pivot; **não existe** em `crates/` | Editor de código sem autocomplete até construir |
| 4 | **Sem depurador/profiler** | Não encontrei crate | Difícil depurar jogo |
| 5 | **Sem operações de bits** | `SYNTAX.md` §13 | Máscaras de colisão precisam virar nomes de camadas |
| 6 | **`Float` sem NaN/Infinity** | `SYNTAX.md` §3.2 | Bom para determinismo; exige cuidado ao receber `f32` da GPU/física |
| 7 | **Maturidade** | `PROJECT_STATE.md`: rodadas P07 em DRAFT e "testes não executados a pedido do usuário" | **Rodar a suíte antes de depender** |
| 8 | **Hot reload com migração de estado** | Descrito como "reload transacional guest" em DRAFT | Essencial para "editar enquanto joga" |
| 9 | **Mensagens em português** | Palavras-chave em inglês (correto, "uma forma só"); mensagens a confirmar | Diagnósticos devem sair em pt-BR |

### Como contornar o desempenho (hipótese a medir, `HIPÓTESE`)

Se cada instrução custa ~100 ns, um quadro de 16 ms comporta algo como 160 mil instruções para **todos** os scripts. Isso é pouco para "para cada árvore, faça X".

A saída é **desenhar a API em lote**: o script diz **o quê**, a engine faz o **laço** em código nativo.

```aipo
# Ruim: o script itera 10 mil vezes (caro)
each arvore in world.find(tag = "Tree") {
    world.set(arvore, wind_strength = 0.8)
}

# Bom: uma chamada, o laço roda na engine
world.set_all(tag = "Tree", wind_strength = 0.8)
```

Além disso, `aipo-wasm` (JIT via Wasmtime) é uma saída para lógica pesada. Metas numéricas só depois do experimento S1 (veja [roadmap](../05-roadmap/01-fases-e-experimentos.md)).

## `aipo.game` e `aipo-game-host`: o que aprender

Os exemplos do `aipo-game-host` mostram **o que evitar** (`LOCAL`, `examples/26_interactive_game.aipo`):

```aipo
# Código real do protótipo: números mágicos de teclado
if host_key_down(262) or host_key_down(68) { player_vx = player_vx + accel * dt }
```

Para a engine, o equivalente deve ser **ação com nome**:

```aipo
if input.held("move_right") { ... }   # ilustrativo
```

Já o `aipo.game/nodes.aipo` traz uma boa ideia: **Gatilho → Filtro → Ação** (`LOCAL`). A implementação atual monta código por concatenação de texto, o que é frágil. A engine deve projetar nós a partir da **árvore sintática**, não de strings. Veja [programação em nós](../03-linguagens-e-api/04-programacao-em-nos.md).

O README do `aipo.game` afirma simulação determinística e "rollback netcode nativo" (`DECLARADO`). Não verifiquei. Tratar como objetivo, não como fato.

## Decisões ligadas

`DR-06` linguagens · `DR-07` nós 1:1 · `DR-11` relação com Poppy e Petunia3D.
