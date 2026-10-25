# Bibliotecas auxiliares: comprar, adaptar ou construir

**Revisão:** 2026-10-24 · **Estado:** proposta de escolhas · **Fonte das versões:** API pública do crates.io, consultada em 2026-10-24 (`WEB`)

## Resumo (TL;DR)

**Decisão (`DR-02`):** construímos a **nossa engine** e usamos bibliotecas para **cortar caminho** onde o assunto é comum e bem resolvido. As bibliotecas ficam **atrás de adaptadores**.

**Regra de ouro:** **construímos o que é a nossa identidade ou exige determinismo; compramos o que é commodity.**

| Construímos | Compramos |
|---|---|
| Schema, comandos, Gameplay IR, Project Doctor | Física de corpos (Rapier) |
| Tabelas do mundo | Áudio (Kira) |
| Cordas (XPBD), grade e A\*, RNG, ruído | Transporte de rede (renet) |
| Terreno, rios, estradas, vegetação, clima | Janela e entrada (winit, gilrs) |
| Renderer (sobre OpenGL) | Texto (cosmic-text) |
| Replicação e protocolo de rede | Acessibilidade (AccessKit) |
| Aipo | Luau (mlua), glTF, imagens, serialização |

## Como decidir: construir × comprar

**Comprar** quando **todas** as respostas forem "sim":
1. É commodity (não é o que nos diferencia)?
2. A licença é permissiva (passa no `cargo deny`)?
3. Tem manutenção (versão recente ou repositório ativo)?
4. Cabe atrás de um adaptador estreito?
5. Não é crítico para o determinismo?
6. Não nos força um modelo (runtime assíncrono, orientação a objetos)?
7. Sabemos o custo de remover?
8. Será registrado no **registro de dependências**?

Se qualquer resposta for "não", **construímos** (ou adiamos).

## O padrão de adaptador

```text
 Domínio / API / schema / saves / protocolo
              │  (só tipos nossos)
      ┌───────▼────────┐
      │   adaptador    │  adapter_physics, adapter_audio, adapter_net …
      └───────┬────────┘
              │  (tipos da biblioteca ficam AQUI)
          Rapier · Kira · renet …
```

- Tipos de biblioteca **nunca** aparecem na API pública, no schema, nos arquivos salvos nem na rede.
- Cada adaptador tem **testes de contrato**. Trocar a biblioteca não muda o resto.
- É a mesma regra do Digi-Tactics: *"identificadores de bibliotecas só em adaptadores"* (`LOCAL`).

## Escolhas e evidências

Versões e datas: crates.io em 2026-10-24.

| Área | Decisão | Biblioteca | Versão (publicação) | Licença | Observações |
|---|---|---|---|---|---|
| Janela e entrada (runtime) | **Comprar** | `winit` | 0.30.13 (2026-10-24) | Apache-2.0 | Ativa hoje. Base do AccessKit para janelas |
| Contexto OpenGL | **Comprar** | `glutin` | 0.32.3 (2025-04-30) | Apache-2.0 | **Sem release há mais de 1 ano**: vigiar; plano B: binding de SDL3 |
| Gamepad | **Comprar** | `gilrs` | 0.11.2 (2026-05-30) | Apache-2.0/MIT | |
| Carregador GL | **Comprar** | `glow` | 0.18.0 (2026-07-09) | MIT/Apache-2.0/Zlib | Petunia3D usa 0.17: subir ao copiar |
| Matemática | **Comprar** | `glam` | 0.34.1 (2026-10-06) | MIT/Apache-2.0 | Petunia3D usa 0.27: **migrar** (quebras possíveis) |
| Física de corpos | **Comprar** | `rapier3d` | 0.36.1 (2026-10-10) | Apache-2.0 | Recurso `enhanced-determinism` (determinismo entre plataformas); **não combina com `simd8`** (`WEB`) |
| Cordas e tecido | **Construir** | XPBD próprio | — | — | O Rapier só tem junta de distância |
| Áudio | **Comprar** | `kira` | 0.12.5 (2026-09-26) | MIT/Apache-2.0 | Mixer com trilhas, relógio, espacial. Limite relatado: canais além de estéreo (`WEB`). Usa `cpal` 0.18.2 |
| Decodificação de áudio | Vem com o Kira | `symphonia` | 0.6.1 | **MPL-2.0** | **Atenção:** o `deny.toml` do Petunia3D não permite MPL-2.0. Se o Cargo.lock trouxer, abrir **exceção registrada** |
| Rede: transporte | **Comprar** | `renet` | 2.0.0 (2026-01-20) | MIT/Apache-2.0 | Sem runtime assíncrono; criptografia (netcode); cliente local em memória (útil para "Testar online"). Canal `u8` |
| Rede: plano B | Reserva | `quinn` (QUIC) | 0.11.12 (2026-09-14) | MIT/Apache-2.0 | Exige runtime assíncrono e confiança em certificados |
| Rede: web e P2P | Depois | `matchbox_socket` | 0.14.0 (2026-02-13) | MIT/Apache-2.0 | WebRTC |
| Rede: replicação e protocolo | **Construir** | próprio | — | — | Comandos e tabelas; **`lightyear` depende do ECS do Bevy** (`WEB`): não serve |
| Texto do jogo | **Comprar** | `cosmic-text` | 0.19.0 (2026-04-22) | MIT/Apache-2.0 | Modelagem de texto e BiDi; atlas de glifos **nosso** |
| Acessibilidade | **Comprar** | `accesskit` + `accesskit_winit` | 0.25.1 / 0.34.1 (2026-09-25) | MIT/Apache-2.0 | Adaptador para `winit` existe |
| glTF (leitura) | **Comprar** | `gltf` | 1.4.1 (**2024-05-10**) | MIT/Apache-2.0 | **Sem release há 2 anos**, repositório ativo em 2026; reserva: fork `myth-gltf` ou o importador do Petunia3D |
| glTF (escrita), OBJ | **Adaptar** | `gltf-json`, `tobj` | do Petunia3D | — | Já usados lá |
| Imagens | **Comprar** | `image` | 0.25.10 (2026-03-10) | MIT/Apache-2.0 | |
| Luau | **Comprar** | `mlua` (feature `luau`) | 0.12.2 (2026-10-03) | MIT | JIT nativo desligado na v1 |
| Aipo | **Construir** (é seu) | crates `aipo-*` | — | MIT | **Desligar** `wasmtime`/`wasm` por padrão (50.0.0, pesado) |
| Serialização | **Comprar** | `serde`, `serde_json`, `postcard` | 1.0.229 / 1.0.154 / 1.1.3 | MIT/Apache-2.0 | JSON com chaves ordenadas nos projetos (Git); `postcard` em pacotes binários |
| Schema JSON | **Comprar** | `schemars` | 1.2.3 (2026-10-12) | MIT | Já usado no Petunia3D |
| MCP | **Adaptar** | `rmcp` | 3.7.0 (2026-10-22) | Apache-2.0 | Petunia3D usa 3.3. `tokio` **só** neste crate |
| Observar arquivos | **Comprar** | `notify` | 8.2.0 (2026-10-17) | CC0-1.0 | Hot reload |
| Utilitários | **Comprar** | `rayon`, `flume`, `uuid`, `thiserror`, `slotmap`, `tracing`, `puffin` | 1.12.0 / 0.12.0 / 1.29.0 / 2.0.21 / 1.1.1 / 0.1.44 / 0.20.0 | MIT/Apache/Zlib | `anyhow` só em aplicativos, nunca em bibliotecas |
| Arquivos ZIP | **Comprar** | `zip` | 9.1.0 (2026-10-24) | MIT | Pacotes exportados |
| SVG (só editor) | **Comprar** | `resvg` | 0.49.0 | Apache-2.0/MIT | Petunia3D usa 0.48.1 |
| GUI do editor | **Decidido** | Qt via `cxx-qt` | 0.10.0 (2026-08-24) | MIT/Apache-2.0 (a crate) | Qt tem licença própria (LGPL/GPL/comercial) |
| Testes | **Comprar** | `insta`, `proptest`, `criterion` + `cargo-nextest`, `cargo-fuzz`, `cargo-deny` | 1.49.0 / 1.12.0 / 0.8.2 | | |

`naia` (réplica de entidades): consulta ao crates.io falhou; **não verificado**.

## O que sempre construímos (e por quê)

| Peça | Motivo |
|---|---|
| **Schema, comandos, Gameplay IR, Project Doctor** | É a identidade da engine |
| **Tabelas do mundo** (colunas dinâmicas por nome) | Scripts definem tabelas em tempo de execução; as bibliotecas de ECS usam tipos estáticos |
| **RNG, A\*, ruído, ordem de simulação** | Determinismo: a sequência precisa ser **idêntica** à referência (teste diferencial com o TypeScript). O gerador padrão do Odin, por exemplo, mudou de PCG64 para chacha8rand e alterou as sequências (`WEB`): o mesmo risco existe em qualquer dependência |
| **Cordas XPBD** | Nenhuma biblioteca atende; é recurso de primeira classe |
| **Rios, estradas, vegetação, clima** | É o diferencial; integra por spline e varredura (do Petunia3D) |
| **Renderer** | Escolha herdada: OpenGL leve, atrás de fronteira neutra |
| **Replicação e protocolo** | Depende do nosso modelo de dados |

## O que evitamos

| Evitar | Por quê |
|---|---|
| `bevy_*` | Impõe modelo de mundo e quebra API a cada versão (`WEB`) |
| `wasmtime` por padrão | Pesado de compilar; só como opção |
| `tokio` no runtime | Só nos crates de MCP e relay |
| `laminar` | Sem release desde 2021 e sem criptografia (`WEB`) |
| Bibliotecas com `unsafe` espalhado sem adaptador | Violam "nunca travar" |

## Registro de dependências

Arquivo `docs/dependencies.md` *(a criar)*, **um item por dependência**:

```text
nome · versão fixada · para que serve · adaptador · licença
última publicação · alternativa · custo de remover · quem decidiu e quando
```

`cargo xtask check-dependencies` *(planejado)* falha se uma dependência direta não estiver no registro. `cargo deny` cuida de licenças e vulnerabilidades.

## Pendências de verificação

| Item | Como verificar |
|---|---|
| Rapier: `enhanced-determinism` com nosso uso | Experimento S6 (corda + corpos, resultados repetidos) |
| Kira: canais além de estéreo e se traz `symphonia` | `cargo tree` na fase de áudio |
| `glutin` sem release há mais de 1 ano | Teste na sua máquina em S3; plano B SDL3 |
| `gltf` sem release há 2 anos | S3 (esqueleto de ~25 juntas); reserva definida |
| Subir `glam`, `glow`, `rmcp` ao copiar código | Ao copiar, com testes de regressão |

## Decisões ligadas

`DR-02` base técnica · `DR-09` online · `DR-08` física · `DR-20` linguagem do núcleo (Odin).
