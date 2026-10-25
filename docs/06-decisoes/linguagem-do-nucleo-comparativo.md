# Linguagem do núcleo: comparativo (Rust, Odin, Go, C#, TypeScript, C++, V)

**Revisão:** 2026-10-24 · **Estado:** análise para o `DR-20` · Fontes: medições nos seus repositórios (`LOCAL`), pesquisa (`WEB`), conhecimento geral (`GERAL`) e inferências (`HIPÓTESE`)

## Resumo (TL;DR)

**Recomendação: manter Rust.** Nenhuma alternativa vence em todos os critérios. As duas que mais se aproximam têm custos altos:

- **C# (editor em Avalonia + núcleo em C#)** é a alternativa mais forte se o problema for **tempo de compilação**. Perde: Luau (o binding .NET foi arquivado em jun/2026), a GUI Qt copiada e a matemática em Rust (41 mil linhas).
- **TypeScript + Three.js** é a mais forte em **acessibilidade e reaproveitamento do Digi-Tactics**. Perde: **desempenho no Linux** (WebKitGTK), determinismo de `Math.*` e as decisões herdadas (Qt, OpenGL).

**Achado sobre a Aipo:** ela foi desenhada para ser **embutida em qualquer linguagem**: crate Rust, **ABI em C** (`aipo.h`), **backend JavaScript** (ESM + source map) e **WebAssembly**. A integração da Aipo **não decide** a escolha. A do **Luau** decide mais: ela é excelente em Rust e C++ e fraca em Go, C# e TypeScript.

**Cuidado:** o substrato de execução da Aipo está em transição (VM de pilha congelada em `legacy/v0.1.0-stack-vm`; WebAssembly adotado em 2026-09-26, ADP-013). Integrar pelo **schema (AHS)** e pela ABI, **nunca pelos internos da VM**.

## Tabela 1 — Fundamentos

Legenda: ✔ bom · ◐ parcial · ✘ ruim. Células curtas; detalhes depois.

| Critério | Rust | Odin | Go | C# | TS + Three.js | C++ | V |
|---|:--:|:--:|:--:|:--:|:--:|:--:|:--:|
| **Velocidade de compilação** | ✘ lenta | ✔ muito rápida | ✔ rápida (cgo ◐) | ✔ rápida | ✔ HMR instantâneo | ✘ lenta (Qt pior) | ✔ muito rápida |
| **Segurança de memória / "nunca travar"** | ✔ pelo compilador | ◐ disciplina | ✔ GC | ✔ GC | ✔ GC | ✘ comportamento indefinido | ◐ autofree experimental |
| **Sem pausas de GC** | ✔ | ✔ | ◐ pausas curtas | ◐ mitigável | ✘ | ✔ | ◐ |
| **Determinismo numérico** | ✔ (Rapier `enhanced-determinism`) | ◐ | ◐ FMA em ARM | ◐ Bepu só na mesma arquitetura | ◐ `Math.*` varia | ◐ flags de FMA | ◐ |
| **Combina com "dados + funções"** | ◐ traits tentam | ✔✔ | ✔ | ◐ cultura OO | ✔ (Digi-Tactics prova) | ◐ cultura OO | ✔ |
| **Maturidade da linguagem** | ✔ | ◐ `dev-AAAA-MM`, sem 1.0 | ✔ | ✔ .NET 10 | ✔ | ✔ | ✘ 0.5.2, sem 1.0 |
| **Bom para agentes de código** | ✔ (feedback do compilador) | ◐ | ✔ | ✔ | ✔ | ◐ | ✘ pouco corpus |
| **Reuso: matemática Rust (41 mil linhas)** | ✔ direto | ◐ porte | ◐ porte | ◐ porte | ◐ porte (ou Rust→Wasm) | ◐ porte | ◐ porte |
| **Reuso: GUI Qt (27,5 mil linhas C++)** | ✔ CXX-Qt | ◐ via ABI em C | ◐ `miqt` | ✘ Avalonia | ✘ DOM | ✔ direto, sem ponte | ✘ sem C++ |
| **Reuso: núcleo TypeScript do Digi-Tactics** | ◐ porte | ◐ | ◐ | ◐ | ✔✔ direto | ◐ | ◐ |

## Tabela 2 — Ecossistema para a engine

| Critério | Rust | Odin | Go | C# | TS + Three.js | C++ | V |
|---|:--:|:--:|:--:|:--:|:--:|:--:|:--:|
| **Física 3D** | ✔ Rapier 0.36.1 | ◐ Jolt (binding externo), box3d novo | ✘ sem opção madura | ◐ Bepu (só mesma arquitetura) | ✔ Rapier JS determinístico | ✔✔ Jolt nativo | ✘ |
| **Áudio** | ✔ Kira | ✔ miniaudio (vendor) | ◐ | ✔ | ✔ WebAudio | ✔ | ◐ |
| **Rede** | ✔ renet | ✔ ENet (vendor) | ✔✔ | ✔ | ✔ WebSocket/WebRTC | ✔ | ◐ |
| **glTF, imagens, texto** | ✔ gltf (sem release há 2 anos), image, cosmic-text | ✔ cgltf, stb | ◐ | ✔ | ✔✔ three.js | ✔ | ◐ |
| **Renderização** | ◐ construir sobre OpenGL | ◐ construir (vendor: OpenGL 4.6, wgpu) | ◐ raylib-go (cgo); Ebitengine é 2D | ◐ Stride como referência | ✔ Three.js r186, WebGPU + WebGL2 | ◐ construir | ◐ |
| **Acessibilidade do editor** | ◐ Qt `QAccessible` | ✘ nenhuma nativa | ◐ via Qt | ✔ Avalonia (UIA, AT-SPI2) | ✔✔ DOM + ARIA | ◐ Qt | ✘ |
| **Acessibilidade da UI do jogo** | ✔ AccessKit + `winit` | ◐ AccessKit via API em C | ◐ | ◐ | ✔✔ HUD em DOM | ◐ AccessKit via C | ✘ |
| **Editor de código e de nós** | ◐ construir em Qt | ◐ | ◐ | ✔ AvaloniaEdit, bibliotecas de nós | ✔✔ Monaco/CodeMirror, bibliotecas de nós | ◐ QScintilla + canvas existente | ✘ |
| **Exportar para web** | ◐ Wasm + WebGL2 (`HIPÓTESE`) | ◐ alvo wasm existe | ◐ binários grandes | ◐ runtime pesado | ✔✔ nativo | ◐ Emscripten | ◐ backend JS |
| **Linux no alvo da v1** | ✔ | ✔ | ✔ | ✔ | ◐ WebKitGTK fraco (Tauri) ou Chromium pesado (Electron) | ✔ | ✔ |

## Tabela 3 — Integração do Luau e da Aipo

Esta é a tabela que você pediu. **Luau** é a segunda linguagem de scripts (uma ativa por projeto); **Aipo** é a primeira classe.

| | Rust | Odin | Go | C# | TS + Three.js | C++ | V |
|---|---|---|---|---|---|---|---|
| **Luau: como embutir** | `mlua`, feature `luau` | C API do Luau via `foreign` (sem binding conhecido) | **sem binding Luau encontrado**; `gopher-lua` é Lua 5.1 (`GERAL`) | `luau-dotnet` **arquivado em 2026-06-23**; Lua-CSharp é Lua 5.2; NLua é Lua 5.1/5.2 | Lua 5.x em Wasm/JS existe; **Luau não verificado** | **nativo** (Luau é C++) | C API via interop (`HIPÓTESE`) |
| **Luau: nota** | ✔✔ `mlua` 0.12.2 (MIT), sandbox, `set_interrupt`, vetor nativo | ◐ | ✘ | ✘ (precisaria de P/Invoke próprio) | ◐ | ✔✔ | ◐ |
| **Aipo: caminho principal** | **crates Rust** (nenhuma FFI) | **ABI em C** (`aipo.h`) | ABI em C via cgo, **ou** Wasm via `wazero` | ABI em C via P/Invoke, **ou** Wasm | **backend JS**: ESM + source map | **ABI em C** (exemplo oficial `c_embedding`) | ABI em C via interop |
| **Aipo: nota** | ✔✔ | ✔ | ◐ cgo ou Wasm (custo de 1,5 a 3 vezes numa medição, não de jogos) | ◐ | ✔✔ testado contra a VM em Node (`differential.rs`) | ✔ | ✔ |
| **Aipo: capacidades e orçamento** | ✔ | ✔ `aipo_runtime_grant_capability`, `set_instruction_budget` | ✔ (mesma ABI) | ✔ (mesma ABI) | ◐ **sem `fs`**; orçamento a verificar | ✔ | ✔ |
| **Aipo: funções do host** | Schema AHS nativo | `aipo_runtime_register_host_fn` | idem | idem | via shim JS | idem | idem |
| **Esforço para as duas linguagens** | **Baixo** | Médio | **Alto** | **Alto** (Luau) | Médio (Luau) | **Baixo** | Médio |

Fatos de apoio: o ADP-009 define a ABI em C "síncrona, versionada, com tipos opacos" para "C, C++, Zig ou outras" (`LOCAL`); `aipo-c-abi` exporta `aipo_runtime_create`, `aipo_runtime_register_host_fn`, `aipo_runtime_call`, `aipo_runtime_step`, `aipo_runtime_set_instruction_budget` e `aipo_wasm_compile/run` (`LOCAL`). O ADP-013 adota o **WebAssembly como substrato** com Wasmtime no desktop e execução nativa em navegadores (`LOCAL`).

## Linguagem por linguagem

### Rust (referência)
- **Ganha em:** segurança verificada pelo compilador, Rapier/Kira/renet/AccessKit/cosmic-text, `mlua`+Luau, Aipo nativa, ferramentas de teste (nextest, proptest, insta, deny) e agentes de código.
- **Perde em:** **tempo de compilação** (379 pacotes no Petunia3D), cultura de traits que tenta o desenho orientado a objetos.
- **Mitigação:** [alavancas de build](odin-vs-rust.md) e o experimento **S0**.

### Odin
- **Ganha em:** compilação muito rápida (14 mil linhas em ~230 ms, benchmark de terceiros, `WEB`), vetores e matrizes embutidos, `vendor:` com `cgltf`, `miniaudio`, `ENet`, SDL3, OpenGL 4.6, wgpu, raylib 6.0, box2d/box3d; melhor encaixe filosófico.
- **Perde em:** sem 1.0 (releases mensais `dev-AAAA-MM`), sem gerenciador de pacotes oficial nos resultados, sem Rapier/Kira/AccessKit nativos, física 3D por binding externo.
- **Histórico:** a baseline da Aipo já foi Odin e virou Rust em 2026-09-15 (`LOCAL`).
- Detalhes do porte: [Odin × Rust](odin-vs-rust.md).

### Go
- **Ganha em:** simplicidade, compilação rápida, ótima rede, GC com pausas curtas, `wazero` (Wasm em Go puro), `miqt` para Qt 6 (MIT, v0.14.0 em mai/2026).
- **Perde em:** 3D: Ebitengine é **só 2D**, `raylib-go` usa cgo; **sem física 3D madura**; **sem Luau**; cgo atrapalha (o primeiro build do `miqt` leva ~10 minutos); GC em jogo de ação.
- **Veredito:** boa linguagem para **servidor/relay**, não para o núcleo da engine.

### C#
- **Ganha em:** iteração e build rápidos, ferramentas excelentes, **Avalonia 12** com acessibilidade nativa (UIA no Windows, **AT-SPI2 no Linux**, `WEB`), editor de código e de nós já existentes, **Stride** (MIT, C# 14/.NET 10, Vulkan/D3D12 no 4.4, `WEB`) como referência.
- **Perde em:** **Luau** (binding arquivado; Lua só 5.1/5.2), a GUI Qt e a matemática em Rust seriam reescritas, GC, **Bepu só determinístico na mesma arquitetura**, o editor do Stride só roda no Windows (`WEB`).
- **Veredito:** a **melhor alternativa** se o gargalo for compilar e se você aceitar abrir mão do Luau nativo e do Qt.

### TypeScript + Three.js
- **Ganha em:** reuso direto do **Digi-Tactics** (núcleo de ~4 mil linhas, comandos com Zod, HUD em React), **Aipo via backend JS** testado contra a VM, **acessibilidade em DOM/ARIA**, Monaco/CodeMirror, Rapier JS determinístico, web nativa e iteração instantânea. Three.js r186 (set/2026) tem WebGPU com fallback WebGL2, iluminação em clusters (r185) e cascatas de sombra (r186, `WEB`). WebGPU dá **compute**, o que o plano OpenGL 3.3 não tem.
- **Perde em:** **Linux**: o Tauri usa WebKitGTK, com desempenho e WebGL que variam; Electron é consistente mas pesado (fontes secundárias, `WEB`). Terreno grande, vegetação e 13 GB de modelos pesam no navegador. `Math.sin/cos` **não é idêntico entre motores** (`GERAL`), exigindo funções próprias para determinismo. A API do Three.js muda a cada versão (`async` descontinuado, `PostProcessing` → `RenderPipeline`, `Clock` → `Timer`), então adaptadores são obrigatórios (o Digi-Tactics já os usa).
- **Conflito:** exige **reabrir decisões herdadas**: Qt (`DR-04`) e OpenGL leve (`DR-03`).
- **Veredito:** melhor para **jogos táticos e de interface**, pior para o **mundo natural** pesado.

### C++
- **Ganha em:** **Luau nativo**, Jolt nativo, **Qt sem ponte** (a GUI de 27,5 mil linhas já é C++), ABI em C da Aipo como cidadã de primeira classe, todo o ecossistema.
- **Perde em:** **o pior tempo de compilação** (justo o que você quer evitar), comportamento indefinido contra "nunca travar", cultura orientada a objetos, gerenciamento de dependências fragmentado.
- **Veredito:** só se você quiser **uma única linguagem com Qt**, aceitando o custo de build e de segurança.

### V
- **Ganha em:** compilação muito rápida, interop com C simples, recarga de código a quente (`WEB`).
- **Perde em:** **sem 1.0** (0.5.2, jul/2026), **autofree é experimental**, quase nenhum ecossistema de jogo nos resultados, pouca base para agentes de código.
- **Veredito:** **risco alto demais** para o núcleo.

## O que cada linguagem exigiria que reabríssemos

| Escolha | Decisões herdadas afetadas |
|---|---|
| Rust | Nenhuma |
| C++ | Nenhuma; remove o CXX-Qt |
| Odin, Go | `DR-04` (ponte Qt por ABI), `DR-06` (Luau por C API ou sem Luau) |
| C# | **`DR-04`** (Qt → Avalonia), **`DR-06`** (Luau), reescrita da matemática |
| TypeScript | **`DR-03`** (OpenGL → WebGL/WebGPU), **`DR-04`**, `DR-06`, e "runtime sem Qt" muda de sentido |
| V | Todas |

## Recomendação

1. **Manter Rust** (`DR-20`, opção 1).
2. **S0 primeiro:** medir o build de `world + schema + commands` sem Qt, `mlua`, `rmcp` e `wasmtime`, na sua máquina.
3. **Se S0 reprovar**, comparar em ordem: **S10** (Odin, só o núcleo de geometria, por ABI) e **S11** (C#: uma fatia com Avalonia, viewport, leitor de tela e Aipo por ABI).
4. **TypeScript + Three.js como produto separado**: um **player web** futuro para jogos táticos, reaproveitando o schema e a Aipo por JS. Não como núcleo da engine.
5. **Seguro barato desde já:** núcleos de geometria como funções puras de dados e **dados de referência em JSON** (qualquer porte futuro se valida contra eles).

## Pendências de verificação

| Item | Como |
|---|---|
| Luau em Odin, Go, TS e V | Spike curto de "Luau + um vetor + um limite de CPU" na linguagem escolhida, só se ela voltar à mesa |
| `NuLua` (sucessor citado pelo `luau-dotnet`) | Verificar antes de descartar C# por causa do Luau |
| Custo real do Wasm para a Aipo em jogo | S1 (`aipo-wasm` com Wasmtime) |
| WebKitGTK × Chromium no seu Linux | Teste visual de um chunk de terreno, só se TypeScript voltar à mesa |
| Qualidade das fontes secundárias (Tauri × Electron) | Confirmar em fontes primárias antes de decidir |

## Fontes
- Odin: https://github.com/odin-lang/Odin/tree/master/vendor · https://github.com/odin-lang/Odin/releases/tag/dev-2026-10
- Jolt para Odin: https://github.com/jrdurandt/joltc-odin
- Luau em .NET (arquivado): https://github.com/nuskey8/luau-dotnet · Lua-CSharp: https://github.com/nuskey8/Lua-CSharp
- Go: https://wasmruntime.com/en/blog/wazero-vs-cgo-2026 · `miqt`: https://github.com/mappu/miqt
- Stride: https://www.stride3d.net/ · https://gamefromscratch.com/stride-4-4-released/
- Avalonia (acessibilidade): https://docs.avaloniaui.net/docs/app-development/accessibility · https://avaloniaui.net/blog/avalonia-12
- Bepu (determinismo): https://github.com/bepu/bepuphysics2/blob/master/Documentation/QuestionsAndAnswers.md
- Three.js r186: https://github.com/mrdoob/three.js/releases/tag/r186 · https://www.utsubo.com/blog/threejs-2026-what-changed
- Tauri × Electron: https://www.abratabia.com/native-wrappers/tauri-vs-capacitor.php
- V: https://vlang.io/ · https://en.wikipedia.org/wiki/V_(programming_language)
- Rapier determinístico (JS): https://www.npmjs.com/package/@dimforge/rapier3d-deterministic
- Aipo: `aipo-lang/docs/decisions/adp-009.md`, `adp-010.md`, `adp-013.md`, `examples/c_embedding` (`LOCAL`)

## Decisões ligadas
`DR-20` linguagem do núcleo · `DR-03` renderer · `DR-04` UI do editor · `DR-06` linguagens de script.
