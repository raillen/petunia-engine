# Filosofia própria × derivar do Godot

**Revisão:** 2026-10-24 · **Estado:** análise para fechar o `DR-02` · Fontes: pesquisa na internet (`WEB`) e leitura dos seus projetos (`LOCAL`)

## Resumo (TL;DR)

**Recomendação: filosofia própria**, com três condições (abaixo). O Godot entrega jogos **muito** mais rápido, mas conflita com **três eixos centrais** do que você pediu: dados em vez de classes, Aipo de primeira classe e um editor redesenhado para acessibilidade e baixa carga cognitiva.

**Honestidade:** se o objetivo fosse **lançar jogos** (como o remake do Digi-Tactics) no menor tempo, o Godot ganharia. Como o objetivo declarado é a **engine em si**, a filosofia própria é coerente.

## As três opções

| | O que é |
|---|---|
| **A. Filosofia própria** | Núcleo Rust seu, editor Qt seu, Aipo nativa. É o plano atual |
| **B. Fork do Godot** | Copiar o código do Godot e mudar o que for preciso (módulos em C++) |
| **C. Extensão do Godot** | Godot oficial, sem fork. Seu código entra como GDExtension (em Rust, via `gdext`) e plugins |

## Fatos do Godot que pesam (`WEB`)

| Fato | Por que importa |
|---|---|
| Godot **4.6** (jan/2026): Jolt como física padrão, IK novo, tema Modern, docks unificados, **LibGodot** (embutir o motor em outro programa), D3D12 padrão no Windows | Muita coisa pronta e em movimento |
| Godot **4.7** (jun/2026): saída HDR, `AreaLight3D`, Asset Store novo, **navegação por landmarks para leitor de tela** | A acessibilidade avança rápido |
| **Leitor de tela** desde o 4.5 via AccessKit (32 mil linhas); editor **ainda parcial** | Jogos acessíveis quase de graça; editor incompleto |
| Ritmo: 4.5 (set/2025), 4.6 (jan/2026), 4.7 (jun/2026) | Uma versão a cada 4 a 5 meses: custo de acompanhar |
| `ScriptLanguageExtension` permite **novas linguagens** via GDExtension; o Godot Dart é o precedente | Dá para plugar a Aipo, mas com cuidados |
| **`godot-luau-script` foi descontinuado** (arquivado em nov/2025): os benefícios sobre o GDScript "não justificaram a manutenção e a divisão" entre Luau e GDScript | Evidência direta contra "uma segunda linguagem num Godot" |
| Terreno: **Terrain3D** (MIT, clipmap, até 65 km, 32 texturas, folhagem instanciada) | Terreno forte e pronto |
| Rios: **Waterways** (malha por Bézier com fluxo; o port Godot 4 é C# e removeu o *flow baking*). Estradas: **Road Generator** (v0.5.1, "não completo") | Rios e estradas existem, mas como addons de terceiros, de qualidade desigual |
| Clima: **não encontrei** addon confirmado | A verificar |
| Rede: API de alto nível pronta (servidor com autoridade); **sem rollback** nativo | Online básico pronto |
| Modelo do motor: nós com dados e lógica juntos, herança forte (artigo oficial) | É o oposto de "dados, não classes" |
| Fork: módulos reduzem conflitos, mas exigem **recompilar o motor, templates de exportação próprios** e acompanhar cada versão | Custo contínuo |
| GDExtension: ABI estável entre versões compatíveis, mas **fixa a versão** (`gdext` ↔ Godot), sem gerenciador de pacotes | Atrito de manutenção |

Licença do Godot: MIT (`GERAL`, confirmar). Não é um obstáculo.

## Matriz de critérios

Legenda: ✔ bom · ◐ parcial · ✘ ruim.

| # | Critério | A. Própria | B. Fork | C. Extensão |
|---|---|:--:|:--:|:--:|
| 1 | **Aipo de primeira classe** (editor, depuração, docs) | ✔ | ◐ (você integra e mantém para sempre) | ✘ (fica de segunda classe ao lado do GDScript) |
| 2 | **Dados em vez de classes** | ✔ | ✘ (a base é Node e herança) | ✘ (só uma fachada) |
| 3 | **API única como dado → nós 1:1** | ✔ (desenhada para isso) | ◐ (o Godot reflete suas classes, mas a API é a dele, grande e orientada a objetos) | ◐ |
| 4 | **Editor redesenhado** (modos, simples/avançado, Project Doctor) | ✔ | ◐ (mexer no editor C++ do Godot) | ✘ (só docks e plugins) |
| 5 | Acessibilidade **do editor** | ◐ (Qt, ainda não certificada) | ◐ (herda o que há, incompleto) | ◐ |
| 6 | Acessibilidade **dos jogos** (UI) | ◐ (construir com AccessKit) | ✔ | ✔ |
| 7 | **Mundo natural integrado** | ◐ (construir tudo, mas integrado) | ◐ (addons soltos, integração a verificar) | ◐ |
| 8 | Física e cordas | ◐ (Rapier + XPBD próprio) | ✔ (Jolt; cordas por addon, a verificar) | ✔ |
| 9 | Render, pós, sombras, partículas, decals | ✘ (construir) | ✔ | ✔ |
| 10 | Animação, IK, UI, áudio, navegação, exportação | ✘ (construir) | ✔ | ✔ |
| 11 | Online | ✘ (construir) | ✔ | ✔ |
| 12 | **Tempo até o Digi-Tactics rodar** | longo | curto | curto |
| 13 | Custo de manter | tudo seu | rebase a cada 4–5 meses + templates próprios | acompanhar Godot e `gdext` |
| 14 | Controle das decisões | ✔ | ◐ | ✘ |
| 15 | Build na sua máquina (swap já engasgou) | pesado (Qt/C++) | pesado (motor C++ inteiro) | leve (Rust + `gdext`) |
| 16 | Diferencial de identidade | ✔ | ◐ | ✘ ("mais um addon") |

**Leitura:** o Godot vence os critérios 6 a 12 (tempo e quantidade de recursos prontos). A filosofia própria vence 1 a 4, 14 e 16 (as razões pelas quais você quer esta engine).

## O que o Godot daria no dia 1

Renderers (Forward+, Mobile, Compatibility), sombras, pós-processamento, partículas, decals, animação esquelética e IK, UI com Controls, áudio, navegação, física (Jolt), rede de alto nível, exportação para várias plataformas, Asset Store e uma comunidade grande (`WEB`/`GERAL`).

Na opção A, **tudo isso** vira trabalho das fases 1 a 8. É a maior parte do esforço do plano.

## Onde o Godot conflita com o que você pediu

1. **Substrato orientado a objetos.** Todo o motor é nó com herança. Fazer "tabelas e regras" em cima disso é construir uma segunda engine dentro da primeira.
2. **Linguagem.** O GDScript é de primeira classe no editor (autocomplete, depurador, docs). Uma linguagem externa não ganha isso de graça. O caso do `godot-luau-script` mostra o custo.
3. **Editor.** A experiência de baixa carga cognitiva (modos fixos, simples/avançado, Project Doctor, vista em contorno) pede um editor próprio. Na opção C é só plugin; na B é manter um editor modificado a cada versão.

## O que sobrevive se um dia você trocar para o Godot

A arquitetura proposta mantém isto **independente do motor**:

| Peça | Sobrevive numa extensão do Godot? |
|---|---|
| Schema único de comandos e a API | ✔ |
| Gameplay IR e Event Sheet (como plugin) | ✔ |
| Project Doctor (como plugin) | ✔ |
| Aipo (como `ScriptLanguageExtension`) | ◐ |
| Geradores de rio, estrada e corda em Rust (via `gdext`) | ◐ |
| Editor Qt, renderer OpenGL | ✘ |

Isso torna a decisão **menos irreversível**: o que mais custa (schema, regras, validador) é reaproveitável.

## Recomendação

**Opção A (filosofia própria)**, com estas condições:

1. **Escopo protegido** pelas fases e provas (Digi-Tactics e Ilha).
2. **Reuso máximo** do Petunia3D, da Aipo e da GUI ([lista de cópia](../01-pesquisa/06-copia-do-petunia3d.md)).
3. **Roubar o que o Godot acerta:** AccessKit para a UI do jogo, Jolt/Rapier para física, API de rede de alto nível, `ScriptLanguage` como modelo de integração de linguagem.

### Gatilhos para reabrir a decisão
Reabrir (e olhar a opção C) se:

- O experimento **S4** reprovar o Qt **e** os planos B do editor.
- A **fase 4** (animação, UI de jogo e toon) estourar o que você aceita de tempo.
- O seu **objetivo mudar** para "lançar jogos o quanto antes".

### Experimento de controle opcional (G1)
Montar a **fatia mínima do Digi-Tactics** em Godot com GDScript, com tempo limitado, e registrar o esforço. Dá um **número real** de "quanto custa não ter engine pronta" para comparar com as fases 3 e 4. Só vale se você quiser decidir com dados.

## Híbrido? (`HIPÓTESE`, não recomendado agora)
O **LibGodot** (4.6) permite embutir o motor em outro programa. Em teoria daria para usar o Godot como runtime dentro do **seu** editor. Não há evidência de que isso seja maduro; fica como estudo, não como plano.

## Fontes
- Godot 4.6: https://forum.godotengine.org/t/godot-4-6-release-all-about-your-flow/131705 · https://alternativeto.net/news/2026/1/godot-4-6-debuts-modern-theme-jolt-physics-node-ids-and-libgodot-embed
- Godot 4.7: https://godotengine.org/releases/4.7/ · https://digitalproduction.com/2026/06/23/godot-4-7-turns-up-hdr/
- Acessibilidade 4.5: https://godotengine.org/releases/4.5/
- Linguagens por GDExtension: https://talks.godotengine.org/godotcon-us-2025/talk/FYE3UE/ · https://godot-rust.github.io/
- `godot-luau-script` descontinuado: https://github.com/Fumohouse/godot-luau-script
- Luau em Godot (alternativas): https://github.com/fernforestgames/luau-gdextension
- Terrain3D: https://github.com/TokisanGames/Terrain3D
- Rios e estradas: https://github.com/Tshmofen/waterways-net · https://github.com/TheDuckCow/godot-road-generator
- Fork × módulos × GDExtension: https://github.com/goostengine/goost/discussions/135 · https://docs.godotengine.org/en/4.4/contributing/development/core_and_modules/custom_modules_in_cpp.html
- Godot sem ECS: https://godotengine.org/article/why-isnt-godot-ecs-based-game-engine/

## Decisões ligadas
`DR-02` base técnica · `DR-06` linguagens · `DR-16` provas.
