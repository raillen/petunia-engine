# Petunia Engine engineering guide

> Rules for AI agents and contributors. Modeled on `digimon-stranger-story-animated-models/AGENTS.md` (Digi-Tactics) and on `petunia3D/AGENTS.md`, adapted to Rust. Analysis and rationale (pt-BR): `docs/00-visao/05-filosofia-de-engenharia.md`.
> **Current phase: phase 1 (walking skeleton) is complete in code; phase 2 (editor and map building) has started.** Commands marked *(planned)* do not exist yet. Status per item: `docs/05-roadmap/03-estado-da-implementacao.md`. Do not implement anything without a recorded decision.
> Language: this file is canonical English. User-facing documentation is currently pt-BR drafts (see `DR-19`).

## 1. Decisions and truthful delivery
- Preserve user work. A workspace may not be a Git repository: never assume rollback exists. Autosave and recovery are product features; back up before destructive migrations.
- Reuse, then refactor, then move; rewrite only when necessary. Make incremental, verified changes.
- A request is intent, not specification. If it contains a design flaw, say so, propose the fix and record the decision in `docs/06-decisoes/`.
- Distinguish verified behavior, provisional behavior, approximation, and planned functionality. API schema entries carry a `status` field for this.
- Label documentation facts `LOCAL`, `WEB`, `DECLARADO`, `GERAL`, `HIPÓTESE` (see `docs/README.md`). Never claim research without a source.
- Never claim a test ran without its actual result. Report `pass`, `fail`, `not run`, or `blocked`.
- User data, accessibility, host stability, and correctness outrank visual polish and benchmarks.
- Never change the stack silently. Inherited decisions (Rust, Qt/CXX-Qt editor, OpenGL, GLB, Gameplay IR, one active language per project, `petunia-libs`) change only through a recorded decision.
- No `git commit`, merge, or push unless explicitly asked.

## 2. Architecture invariants

### Language and naming
- Engine core, runtime and tools are Rust. The editor GUI is C++/Qt behind an adapter. Scripts are Aipo (default) or Luau.
- Use full English `snake_case` names: `elapsed_seconds`, `maximum_health_points`, `animation_role`. Never introduce `dt`, `ctx`, `mat`, `geom`, `idx`, `cfg`, `msg`, `tmp`, `pos`, `vel`, or single-letter closure parameters.
  - Allowed: `x y z w r g b a u v` as vector, color or UV components.
  - Enforced by `cargo xtask lint-names` *(planned)*.
- External formats and library identifiers are exceptions **only at adapters**. Map them into descriptive internal names.

### Data and behavior
- Separate serializable definitions and state from behavior. Use plain structs/enums, small free functions and composed operation records (tables of data and functions), inspired by Aipo structs and associated functions.
- No project-owned trait hierarchies for domain logic, no `dyn` plugin frameworks, no service locators, no speculative abstractions. Library classes are allowed at adapters only (for example Qt widget classes inside the GUI adapter crate).
- **Commands are data records plus registered functions, not trait objects.** A command payload is serializable, so it can travel over the network, MCP, macros and undo history.
- Do not copy `ModuleRegistry` / `dyn Module`. Use an explicit, ordered system table.
- The script host with capabilities is a product feature, not a plugin framework. No native plugins (DLLs). Mods are scripts with manifests.

### Boundaries
- `unsafe` is allowed only at adapters: `petunia_render_gl` (OpenGL), `petunia_player` (windowing) and `petunia_editor_ffi` (the C boundary of the editor). Every other crate keeps `unsafe_code = "deny"`.
- Domain crates (`world`, `schema`, `ir`, `commands`, `sculpt`, `terrain`, `water`, `roads`, `flora`, `atmosphere`, `grid`, …) must not depend on rendering, windowing, Qt, physics/audio/network libraries, debug tools or adapters. Enforced by `cargo xtask check-architecture` *(planned)* using a `cargo metadata` allow-list.
- Presentation (editor, scripts, MCP, network) invokes **validated commands**. Observation uses **queries** that return copies. Never expose arbitrary code execution.
- Every user-, script- or agent-facing mutation is a command. Commands document inputs, outputs, failures, reversibility, permission and cost. Internal helpers and per-frame updates are **not** commands.
- No circular dependencies. Keep public crate boundaries narrow.
- The runtime never depends on the editor or on Qt.

### Data hygiene
- Production code never panics: no `unwrap`, `expect`, `panic!`, `todo!`, `unimplemented!`, `unreachable!` outside tests. Use `Result` and `?`. `catch_unwind` only as a last resort at the command dispatcher and render loop.
- Validate all external data (files, scripts, network messages, mods, imported scenarios): parse into typed values, then validate with limits on size, depth and count. Only finite floats. Casts and `unsafe` are not validation.
- Asset specialization composes **defaults → profile → overrides**; it is not inheritance. Applies to prefabs, materials, characters, quality, weather and accessibility profiles.
- Explicitly own GPU resources. The renderer owns them; instances hold handles; shared caches are reference counted; an instance never frees a shared resource. Test load, unload and context loss.
- Determinism-critical code (RNG sequence, grid and A\*, simulation order, game noise) is ours, never a dependency's.

## 3. Dependencies
- Add a dependency only for a demonstrated need, using the admission checklist in `docs/02-arquitetura/10-bibliotecas-auxiliares.md`:
  1. commodity, not identity;
  2. permissive license (`cargo deny`);
  3. maintained (recent release or active repository);
  4. fits behind an adapter;
  5. not determinism-critical;
  6. does not force an async runtime or an object model on us;
  7. removal cost is known;
  8. recorded in the dependency register.
- Library types never appear in the public API, the schema, save files or the network protocol.
- Pin versions. `tokio` only in the MCP and relay crates. Heavy optional features (for example Aipo `wasmtime`) stay off by default.

## 4. Host and automation guardrails
- Toolchain: stable Rust 1.97.1 (`rust-version = "1.97"`). The `1.98.1` pinned by Petunia3D is not installed here.
- Build setup that works on this machine: `export CARGO_TARGET_DIR=/tmp/petunia-engine-target CARGO_BUILD_JOBS=2`. The target directory lives in tmpfs because the disk has been nearly full; check `df -h /` first. `lld` is the linker (see `.cargo/config.toml`).
- Build `petunia_cli` with `--no-default-features` for a fast headless loop (no windowing or OpenGL crates).
- The Qt editor lives in `editor/` and links `petunia_editor_ffi`. `scripts/editor.sh` builds both (output in `/tmp`) and runs it. Automated checks of the window use `scripts/editor-smoke.sh`, which runs in a **virtual display (Xvfb)**: never open the editor on the user's own session from automation, and never write to the user's real editor settings (set `XDG_CONFIG_HOME` to a scratch folder).
- Use at most two test threads (`cargo nextest run -j 2` or `--test-threads=2`) and one visual-test worker. Use `CARGO_BUILD_JOBS=2` for anything touching Qt/C++. Avoid simultaneous heavy builds and visual tests.
- Visual tests use a dedicated automation GL context or profile, never the personal desktop session. Do not steal focus or move the OS pointer.
- Debug, MCP and network test services bind to loopback only. Never disable sandboxing or GPU security to get a green test.
- Run long operations in the background. Bound downloads, scenario sizes, memory and test durations.
- Check free disk space (`df -h .`) before large asset operations. Never bulk-delete assets, caches or unknown files.
- **Always clean stale, unnecessary build outputs.** At the end of every task that compiled or ran tests, run `scripts/clean-builds.sh` and report what it freed. It only touches outputs this project creates: a duplicate `target/` inside the repository (the build folder is `/tmp/petunia-engine-target`, set in `.cargo/config.toml`), incremental caches older than a day, and leftover test folders (`/tmp/petunia-smoke.*`, `/tmp/petunia-x.*`, `/tmp/petunia_editor_test_*`). When the disk is nearly full, `scripts/clean-builds.sh --all` also removes the build folders themselves (the next build is a full rebuild). Never point it at other paths, and never delete user projects created for testing outside `/tmp`.
- Stop only processes owned by this project's automation.

## 5. Progressive context
- Start with this file, then `docs/00-visao/04-descricao-completa.md`, then `docs/06-decisoes/registro-de-decisoes.md`, then the changed module and its tests.
- Load only the skills the task needs. Agents are task-specific, not permission to spawn unbounded workers.

| Topic | Read |
|---|---|
| Vision, principles | `docs/00-visao/` |
| Prior decisions | `docs/01-pesquisa/05-especificacao-anterior.md` |
| What to copy from Petunia3D | `docs/01-pesquisa/06-copia-do-petunia3d.md` |
| Libraries and adapters | `docs/02-arquitetura/10-bibliotecas-auxiliares.md` |
| Architecture by subsystem | `docs/02-arquitetura/` |
| Editor (session, JSON operations, Qt shell) | `docs/02-arquitetura/11-editor.md` |
| Languages, API, visual logic | `docs/03-linguagens-e-api/` |
| Script Host (Aipo), code editor | `docs/03-linguagens-e-api/05-script-host.md` |
| Accessibility | `docs/04-acessibilidade/` |
| Phases and experiments | `docs/05-roadmap/` |
| Proof projects | `docs/07-prova-de-conceito/` |

## 6. Layered verification
Gates for changed code. Every gate reports an honest status.

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo nextest run --workspace -j 2
cargo deny check
cargo xtask check-architecture   # planned
cargo xtask check-dependencies   # planned: every dependency is in the register
cargo xtask lint-names           # planned
cargo xtask docs-check           # planned: generated API docs, en and pt-BR parity
```

- Test pyramid, six layers: pure domain, in-memory command contracts, headless GUI, hostile data and fuzzing, visual regression, accessibility.
- Run `cargo xtask test-visual` *(planned)* for scene, material, animation and interaction changes. Unit mocks are not visual evidence. A missing GL context is `blocked`; it must never silently pass.
- Freeze seed, time, camera, viewport and lighting for captures. Investigate differences; never silently widen tolerances.
- Use **differential tests** against a reference with the same seed and compare outputs byte for byte. Store golden data in a language-neutral format so any port can be checked.
- Test invalid input, repeated loads, missing assets and clips, and resource cleanup, as well as success paths.
- Every new capability needs: a command and/or query, an inspection surface, tests, and English plus pt-BR documentation. A capability manifest (`docs/capabilities.json`, *planned*) maps each capability to its command, query, panel, test and doc and is checked automatically.

## 7. Task lifecycle and handoff
- Establish scope → inspect baseline → add a regression test → implement the minimum change → verify → update documentation and translations.
- Canonical prose is English (`docs/en`); synchronized pt-BR lives in `docs/pt-BR` with matching page identifiers. Generated references (API, statistics) come from the schema and definitions; never duplicate numbers by hand.
- Record implementation status honestly. Do not turn roadmap items into implemented claims.
- Handoff: outcome, changed paths, executed checks and results, host status, limitations, and **one** next step.

## 8. Legacy and copied code
- Code copied from Petunia3D or Aipo is **legacy**. Record the origin commit in `ATTRIBUTION.md`. Migrate touched modules to these rules with regression coverage. Do not perform blind global renames.
- Measured at 2026-10-24 (word-boundary matches in `.rs`): Petunia3D has `ctx` 423, `idx` 260, `cfg` 239, `mat` 189, `dt` 52; Aipo has `idx` 284, `cfg` 172, `msg` 110, `ctx` 77, `dt` 46.
- Petunia3D defines `Command` as a trait with 245 implementations. Port the **semantics** as data-driven commands; do not copy the trait pattern.

## 9. Provenance and safety
- Every new asset (model, texture, font, sound, icon) goes into `ATTRIBUTION.md` with author, source and license.
- Assets from commercial games (for example Digimon models and shaders) stay **outside Git** as local fixtures. Public demos use original or clearly licensed assets.
- Clean-room only. No copying or disassembling proprietary tools.

## 10. Tie-breakers
1. User data and zero work loss.
2. Accessibility and neurodivergent-friendly UX.
3. Host stability and modest hardware.
4. Correctness and determinism.
5. Coherence: one source of truth per responsibility.
6. Familiarity with reference tools (Godot, Unity, GameGuru MAX), without copying code.
