# Arquitetura — visão geral

**Revisão:** 2026-10-24 · **Estado:** proposta, alinhada às decisões da [especificação anterior](../01-pesquisa/05-especificacao-anterior.md)

## Resumo (TL;DR)

A engine tem **3 camadas** e **1 contrato central**:

- **Domínio** (dados e regras do mundo): sem GPU, sem janela, sem UI. Testável sozinho.
- **Serviços** (renderer, física, rede, áudio, scripts): cada um atrás de uma fronteira estreita.
- **Apps** (editor, jogo exportado, servidor sem tela, linha de comando, MCP): finos, só orquestram.
- **Contrato central:** o **schema da API**. Tudo o que um script, um nó ou um agente pode fazer está descrito ali.

Decisões herdadas que valem aqui: **Rust**; editor **Qt/CXX-Qt** (o runtime **nunca** depende de Qt); renderer **OpenGL leve**; **`RuntimeWorld`** separado da autoria; reuso por crates em **`petunia-libs`**.

Padrão herdado do Petunia3D: *a UI emite intenções; a aplicação valida e despacha comandos; o domínio executa; o renderer só desenha* (`LOCAL`, `AGENTS.md` §2.3).

## Diagrama

```text
 ┌───────────────── APPS (finos) ─────────────────┐
 │ Editor   Jogo exportado   Servidor   CLI   MCP │
 └───────────────────────┬────────────────────────┘
                         │ intenções / comandos
 ┌───────────────────────▼────────────────────────┐
 │        Aplicação: CommandDispatcher + Undo      │
 │   valida · executa em transação · registra      │
 └───────┬──────────────────────────────┬─────────┘
         │                              │
 ┌───────▼────────┐          ┌──────────▼──────────┐
 │    DOMÍNIO     │          │     SCRIPTS         │
 │ mundo, terreno │◄────────►│ Aipo · Luau · nós   │
 │ rios, estradas │ schema   │ (permissões, limite │
 │ flora, clima…  │ da API   │  de instruções)     │
 └───────┬────────┘          └─────────────────────┘
         │ extrai quadro (somente leitura)
 ┌───────▼───────────────────────────────────────┐
 │ SERVIÇOS: render · física · cordas · rede ·    │
 │           áudio · entrada · UI do jogo         │
 └────────────────────────────────────────────────┘
```

## Regras de dependência (herdadas)

1. **Domínio não conhece GPU, janela nem toolkit de UI.**
2. **Módulos não se conhecem.** Falam por eventos e pelo registro de módulos.
3. **Só o crate de render toca a GPU.**
4. **O runtime nunca depende do editor.**
5. **Entrada externa é hostil:** arquivo, script, rede e mod têm limites e retornam `Result`.

## Proposta de crates (nomes provisórios)

| Grupo | Crates | Observação |
|---|---|---|
| Base | `math`, `world`, `schema`, `ir`, `commands` | `world` guarda tabelas e ids; `schema` é o contrato; `ir` é o Gameplay IR |
| Conteúdo | `assets`, `package`, `prefabs` | glTF, pacotes ZIP versionados |
| Mundo natural | `terrain`, `water`, `roads`, `flora`, `atmosphere`, `decals` | Cada um gera dados + malha; sem GPU |
| Serviços | `render`, `post`, `animation`, `physics`, `ropes`, `net`, `audio`, `input`, `ui_runtime`, `grid` | `render` é o único com GPU |
| Scripts | `script_host`, `lang_aipo`, `lang_luau`, `rules` | Um host, duas linguagens |
| Qualidade | `doctor` | Validador do mapa e de acessibilidade |
| Apps | `editor`, `player`, `server`, `cli`, `mcp` | Finos |

## Um quadro do jogo (ordem fixa)

A ordem é sempre a mesma. Isso cumpre o princípio "previsível".

```text
 1. Entrada          → traduz teclas/botões em ações nomeadas
 2. Rede (receber)   → aplica estado e entradas recebidas
 3. Regras e scripts → rodam em passo fixo (ex.: 60 Hz), com limite de instruções
 4. Física           → corpos, personagem, gatilhos
 5. Cordas           → solver próprio, mesmo passo fixo
 6. Eventos pós-física → colisões, cortes de corda, entradas em volume
 7. Aplicar comandos adiados → spawn/remove só aqui (ponto seguro)
 8. Mundo vivo       → clima, vento, água, vegetação (estado)
 9. Rede (enviar)    → deltas das tabelas compartilhadas
10. Extrair quadro   → cópia somente leitura para o renderer
11. Renderizar       → passes de imagem e pós-processamento
```

O passo fixo (itens 3 a 7) é separado do desenho. O desenho interpola entre dois estados. O `aipo-poppy` já usa `Simulation` com passos fixos e semente de aleatoriedade (`LOCAL`).

## Três produtos, um ecossistema

| Produto | Papel | Licença sugerida |
|---|---|---|
| **Petunia3D** (existe) | Criar e exportar assets (modelos, texturas, esqueletos) | GPL-3.0 (atual) |
| **Petunia Editor** (novo) | Montar mundos, lógica e jogos | GPL ou igual ao runtime |
| **Petunia Runtime** (novo) | Rodar o jogo exportado | Permissiva (veja `DR-01`) |

A ponte é o **glTF/GLB** e o formato de pacote (`LOCAL`: o Petunia3D já exporta GLB; ADR-C04). Unificar o modelador dentro do editor é uma opção **futura** (`DR-11`).

## Perfis de qualidade (para hardware modesto)

| Perfil | Meta | Ideia |
|---|---|---|
| Simples | Máquinas muito modestas | Sem sombras em cascata; vegetação curta; pós mínimo |
| Padrão | Maioria | Sombras em 2–3 cascatas; água com refração; pós leve |
| Alto | Máquinas boas | AO, MSAA/SMAA, vegetação longa |

Metas numéricas (FPS, memória) **só depois de medir**. O Petunia3D adota a mesma regra: "não inventar limites; estabelecer por medição" (`LOCAL`, decision-register).

## Decisões ligadas

`DR-02` base técnica · `DR-03` renderer · `DR-05` modelo de dados · `DR-11` relação com Petunia3D.
