# Registro de dependências

**Revisão:** 2026-10-29 · Regra do `AGENTS.md`: toda dependência direta entra aqui **antes** de entrar no `Cargo.toml`. `cargo xtask check-dependencies` *(planejado)* vai exigir isso automaticamente.

Versões: as resolvidas pelo `Cargo.lock` em 2026-10-29 (o `png` entrou nesta revisão). O `Cargo.lock` completo tem **299 pacotes** (o do Petunia3D tem 379). A edição de malha trouxe **12** deles (as 2 diretas e 10 indiretas, abaixo); o resto da diferença para os 212 de 2026-10-24 deve ter vindo do Script Host (DR-25) e das etapas seguintes (não conferi pacote a pacote).

## Resumo (TL;DR)

13 dependências diretas do Cargo. Quatro fazem o trabalho de plataforma e ficam **atrás de adaptadores** (`petunia_render_gl` e `petunia_player`). Duas são geometria e ficam **só dentro** de `petunia_mesh` (`glam` e `manifold-rust`, desde 2026-10-25, DR-22). `gltf` e `png` são leitores de arquivo e ficam **só dentro** de `petunia_import`. A física (`rapier3d`) fica só em `petunia_physics`. As outras são utilitários (serialização e erros). O editor soma **dependências do sistema** (Qt, CMake), listadas mais abaixo.

## Registro

| Dependência | Versão | Para que serve | Onde (adaptador) | Licença | Alternativa | Custo de remover |
|---|---|---|---|---|---|---|
| `serde` | 1.0.229 | Ler e gravar valores (`Vec3`, `Color`, projetos, schema) | Crates de domínio (derive) | MIT/Apache-2.0 | Escrever à mão | Alto: o formato de arquivos depende dela |
| `serde_json` | 1.0.154 | Formato de texto dos projetos, comandos e schema | `petunia_world`, `petunia_schema`, `petunia_commands`, `petunia_project` | MIT/Apache-2.0 | `miniserde`, `simd-json` | Médio |
| `thiserror` | 2.0.21 | Tipos de erro com mensagem | Todos os crates com erros | MIT/Apache-2.0 | Escrever `Display` à mão | Baixo |
| `glow` | 0.18.0 | Chamadas OpenGL seguras o bastante | **`petunia_render_gl`** (único) | MIT/Apache-2.0/Zlib | `glium`, `wgpu` (plano B) | Médio: o renderer depende dela |
| `winit` | 0.30.13 | Janela e eventos de teclado e mouse | **`petunia_player`** (único) | Apache-2.0 | SDL3 | Médio |
| `glutin` | 0.32.3 | Criar o contexto OpenGL 3.3 | **`petunia_player`** (único) | Apache-2.0 | SDL3 | Médio |
| `glutin-winit` | 0.5.0 | Liga o `glutin` ao `winit` | **`petunia_player`** (único) | MIT | Fazer à mão | Baixo |
| `raw-window-handle` | 0.6.2 | Tipo comum entre janela e contexto | **`petunia_player`** (único) | MIT/Apache-2.0/Zlib | — | Baixo |
| `glam` | 0.27.0 | Vetores e matrizes do código de malha copiado do Petunia3D. Aprovada em 2026-10-25 (DR-22) com a condição de ficar só dentro do crate | **`petunia_mesh`** (único; nunca na API, no schema, nos arquivos nem na rede) | MIT/Apache-2.0 | `petunia_math` (reescrever ~10,9 mil linhas de legado) | Médio: o legado inteiro usa |
| `rapier3d` | 0.36.1 (com `enhanced-determinism`) | Corpos rígidos, colisões, raios, gatilhos. **Escolhida por você em 2026-10-25** (DR-27: "Já com corpos físicos", "Também malha convexa já"); já era a recomendação do DR-08. Traz `parry3d` (geometria), `glamx`/`glam` 0.33 e `nalgebra` | **`petunia_physics`** (único; tipos do Rapier nunca saem do crate) | Apache-2.0 | Jolt (C++), solver próprio | Médio: o adaptador tem ~900 linhas; a API pública e os arquivos não mudam |
| `manifold-rust` | 0.13.1 | Booleanos de malha (fundir, cortar, interseção). **Aprovada por você em 2026-10-25** (DR-22) | **`petunia_mesh`** (único, `legacy/boolean.rs`) | Apache-2.0 | Escrever CSG robusto (meses) | Médio: só `boolean.rs` a troca |
| `gltf` | 1.4.1 (`default-features = false`, `utils`, `names`) | Ler modelos `.glb`/`.gltf`. **Aprovada por você em 2026-10-26** (DR-29: "Fase 2: aprovado"). Sem as funções de imagem: texturas não são lidas | **`petunia_import`** (único; tipos da `gltf` nunca saem) | MIT/Apache-2.0 | Ler glTF à mão (JSON + buffers) | Baixo: o crate tem uma função de entrada e uma de saída; trocar só mexe nele |
| `png` | =0.18.1 | Decodificar imagens `.png` para sprites 2D (DR-49). Só decodifica: os limites (32 MiB de arquivo, 4096 por lado, 8 Mi pixels) e o RGBA8 de saída são do nosso código | **`petunia_import`** (único; os pixels saem como `Vec<u8>` RGBA) | MIT/Apache-2.0 | Escrever um decodificador PNG (inflate, filtros, CRC) — semanas, com risco de segurança | Baixo: `decode_png` é a única porta; trocar só mexe nele |

## Observações

- **`manifold-rust` traz 10 pacotes indiretos**, todos permissivos: `clipper2-rust` 1.1.0 (BSL-1.0, a licença Boost), `dashu-int`, `dashu-ratio` e `dashu-base` 0.5.1, `num-modular` 0.6.5, `num-traits` 0.2.19, `rustc-hash` 2.1.3, `static_assertions` 1.1.0, `rustversion` e `autocfg` (MIT ou Apache-2.0). Só o editor os usa: o runtime (`petunia_player`) e o `petunia_render` **não** dependem de `petunia_mesh` (desenhar uma malha só precisa de `petunia_math::PolygonMesh`). Conferido: o código do `manifold-rust` não tem `unsafe`; a opção `parallel` (Rayon) fica **desligada**, então os booleanos rodam numa thread só e dão o mesmo resultado sempre (teste de determinismo).

- **`glutin` está há mais de um ano sem release** (0.32.3, 2025-04-30). Vigiar; plano B: binding de SDL3.
- **`png` traz 5 pacotes indiretos**, todos permissivos e de puro Rust: `bitflags` 2.13.2, `crc32fast`, `fdeflate`, `flate2` e `miniz_oxide` 0.8.9 (o `flate2` está com o backend Rust, sem C). Nenhum código `unsafe` nosso: a decodificação fica atrás de `decode_png`, que valida tamanho e devolve cópia.
- **Duplicações transitivas no `Cargo.lock`:** avisadas com `multiple-versions = "warn"` no `cargo deny check bans`:
  - `thiserror` e `thiserror-impl`: 2.0.21 direta nossa e 1.0.69 puxada transitivamente por `sctk-adwaita` / `calloop` (via `winit`).
  - `syn`: 3.0.9 direta via `serde_derive` / `thiserror 2` e 2.0.119 via `thiserror-impl 1.0.69`.
  - `rustix` (0.38.44 e 1.1.5) e `linux-raw-sys` (0.4.15 e 0.12.1): puxadas por subsistemas Wayland e X11 do `winit`.
  - `windows-sys` (0.52.0, 0.59.0, 0.61.2): puxadas por `glutin`, `winit` e `rustix`.
- **Avisos e exceções em `deny.toml`:**
  - `allow-wildcard-paths = true`: permite dependências de caminho relativas (`path = "crates/..."`) dentro do workspace sem exigir especificação artificial de `version` para pacotes locais (`publish = false`).
  - `[advisories] ignore = ["RUSTSEC-2026-0192"]`: aviso informativo de que `ttf-parser 0.25.1` foi declarado sem manutenção ativa pelo autor upstream. É dependência indireta transitiva do `winit 0.30.13` (`winit` -> `sctk-adwaita` -> `ab_glyph` -> `owned_ttf_parser` -> `ttf-parser`) para fontes das decorações de janela em Wayland. Não há vulnerabilidade de segurança reportada nem correção disponível no `winit 0.30.x`.
- **Tipos de biblioteca não aparecem** na API pública, no schema, nos saves nem no protocolo. `glam` existe **só dentro** de `petunia_mesh`, para o código copiado do Petunia3D; o resto da engine usa os tipos próprios de `petunia_math` (`Vec3`, `Color`, `Transform`, `PolygonMesh`).
- **Licença dos pacotes:** todas permissivas. `deny.toml` permite só licenças permissivas; MPL-2.0 (`symphonia`, via Kira) fica fora até haver uma exceção registrada.

## Dependências do sistema (editor Qt)

Não são pacotes do Cargo. Ficam fora do `Cargo.lock`, e o editor só compila se existirem. O motor, o `petunia run` e a CLI **não dependem** delas.

| Dependência | Versão medida aqui | Para que serve | Licença | Alternativa | Custo de remover |
|---|---|---|---|---|---|
| Qt 6 (`Widgets`, `OpenGLWidgets`, `Svg`) | 6.4.2 no Pop!_OS 24.04 (mínimo exigido: 6.4; 6.12.0 foi medido em 2026-10-25) | Janela do editor, docas, diálogos e o viewport GL | LGPL-3.0 nos módulos usados, com **link dinâmico** (GERAL; conferir antes de distribuir) | Slint, egui, Qt/QML (CXX-Qt) | Alto: toda a pasta `editor/` |
| CMake | 4.4.4 | Compilar `editor/` | BSD-3-Clause | Meson | Baixo |
| Ninja | 1.13.2 | Gerador de build usado pelo script | Apache-2.0 | Make | Baixo |
| Compilador C++20 | GCC 16.2.1 | Compilar `editor/` | GPL (ferramenta, não entra no produto) | Clang 23 (instalado) | Baixo |

Só para o teste de fumaça (`scripts/editor-smoke.sh`), nunca no produto:

| Ferramenta | Para que serve |
|---|---|
| `Xvfb` | Display virtual: o teste não aparece na sua tela |
| `xdotool` | Simula clique e teclado na janela |
| ImageMagick (`import`) | Captura os quadros do canvas e do jogo no teste de fumaça |
| Mesa (`LIBGL_ALWAYS_SOFTWARE=1`) | OpenGL por software dentro do display virtual |

Os crates `petunia_editor` e `petunia_editor_ffi` usam só as dependências registradas acima.

## Aipo (Script Host, 2026-10-25)

O crate `petunia_script` embute a linguagem **Aipo** ([DR-25](06-decisoes/registro-de-decisoes.md)). É projeto seu (MIT; reuso livre pelo `DR-01`), usado por **dependência de caminho** (`../aipo-lang/crates/...`), sem cópia de código. Só `petunia_script` depende dele; `petunia_editor`, `petunia_cli` e `petunia_player` chegam a ele por esse crate.

| Crate | Para que serve | Recursos |
|---|---|---|
| `aipo-source`, `aipo-diagnostics`, `aipo-syntax`, `aipo-ast`, `aipo-hir`, `aipo-sema`, `aipo-ir`, `aipo-bytecode` | Compilar e dar erros com linha e coluna; `aipo-ast` fornece os operadores do leitor IR (DR-34) | padrão |
| `aipo-vm` | Executar, com orçamento de instruções | **sem** recursos padrão |
| `aipo-runtime`, `aipo-stdlib` | Biblioteca padrão (com `sh`, `fs`, `env`, `io` **removidos** pelo host) | `aipo-stdlib` **sem** recursos padrão (sem `regex`) |
| `aipo-host` | Capabilities e handles | padrão |
| `aipo-formatter` | Botão Formatar do editor de código | padrão |

Pacotes do crates.io que entraram com ela (LOCAL, `cargo tree -p petunia_script`): `byteorder` 1.5.0, `unicode-normalization` 0.1.26 e `tinyvec` 1.13.3. Os demais (`serde`, `serde_json`, `thiserror` e suas macros) já estavam no registro.

**Custo de remover:** baixo. `petunia_script` é uma folha; sem ele o motor roda sem scripts.
**Risco:** o build depende da pasta irmã `../aipo-lang`. Para distribuir, vira dependência git fixada por commit.

## Candidatas ainda **não** adotadas

Detalhes e evidências em [bibliotecas auxiliares](02-arquitetura/10-bibliotecas-auxiliares.md): `rapier3d` (física), `kira` (áudio), `renet` (rede), `cosmic-text` (texto), `accesskit` (acessibilidade), `mlua` (Luau).

Cada uma só entra com sua fase e depois de passar no checklist de admissão.
