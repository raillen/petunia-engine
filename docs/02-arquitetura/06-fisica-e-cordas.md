# Física leve e cordas

**Revisão:** 2026-10-24 · **Estado:** proposta; precisa do experimento S6

## O que já existe (2026-10-25, DR-27)

| Parte | Estado |
|---|---|
| Crate **`petunia_physics`**: adaptador do Rapier 0.36.1 (determinístico); só tipos do Petunia na API | 10 testes (queda, gatilho, raio, sobreposição, convexa e cápsula com escala, entrada hostil, determinismo bit a bit, cinemático) |
| Tabelas **`Collider`** (várias com nome por objeto, cada uma é uma entidade com `owner`) e **`RigidBody`** (`static`, `dynamic`, `kinematic`) | leitor de cena testado (colisões quebradas viram aviso com o número do objeto) |
| Script Host: passo fixo de 1/60 s, até 8 passos por quadro, poses de volta no `Transform`, `on_collision` / `on_collision_end`, API `collision.*` e `physics.*` | 4 testes pelo script |
| `petunia run` liga a física quando há colisões, corpos ou personagens; o jogador anda e a câmera o segue | compila; **não rodou com janela** |
| **Solidez automática**: tudo que é desenhado é sólido sem configurar (caixa, superfície exata, casco convexo, cápsula) | testes do leitor de cena + a sala de exemplo andável, **vista em imagem** |
| **Personagem** (`Character`): desliza em paredes, sobe degraus e rampas, pulo com **tempo de coiote**, camadas e máscaras | **6 testes**: degrau, parede, degrau alto demais, pulo, coiote ligado e desligado, entradas ruins, camadas, determinismo bit a bit |
| **Modo Jogar** (`F6`) no editor, sobre uma cópia | 2 testes pelo JSON + **visto (display virtual)** |
| Editor: adicionar, ajustar, listar sob o objeto, desenhar, duplicar e excluir colisões | 1 teste de fluxo + **visto (display virtual)** |

**Ainda não:** plataformas móveis que carregam o personagem, empurrar caixas, juntas, cordas (XPBD) e simular a física dentro do editor sem o modo Jogar. (As alças no viewport para redimensionar a colisão foram feitas em 2026-11-08, para colisões em caixa.)

## Resumo (TL;DR)

**Física leve** significa: corpos simples, personagem com controle próprio, gatilhos, raios e **orçamento**. Sem veículos, ragdolls ou destruição na versão 1.

**Cordas** são recurso de primeira classe e usam um **solver próprio** (XPBD). O motor de corpos rígidos é o **Rapier** (recomendação provisória).

## Por que dois solvers

| Necessidade | Quem resolve | Motivo |
|---|---|---|
| Corpos rígidos, colisores, personagem, raios, gatilhos | **Rapier** (Rust) | Maduro; WASM; modo determinístico opcional (`WEB`) |
| Cordas, cabos, correntes, bandeiras | **Solver próprio (XPBD)** | O Rapier tem `RopeJoint` (distância máxima entre dois corpos), **não** uma corda completa (`WEB`) |

XPBD (Extended Position-Based Dynamics) mantém **restrições de distância** entre partículas e resolve por posição, com subpassos. É estável e simples. Limitação conhecida: precisa de várias iterações e não modela bem materiais que deformam (`WEB`).

## Escolha do motor de corpos (`DR-08`)

### A. Rapier — **recomendada (provisória)**
- Rust puro, 2D e 3D, WASM, SIMD, determinismo opcional entre plataformas (`WEB`).
- Benchmarks do próprio projeto o colocam à frente do Jolt na maioria das cenas (autopublicado, `WEB`).
- Licença Apache-2.0 (`GERAL`, confirmar).

### B. Jolt Physics (C++)
- Muito completo: personagem, veículos, ragdolls, corpos moles (`WEB`).
- Ligações Rust existem (`rolt`, `oxijolt`), mas as mais completas são recentes e de um autor só (`WEB`). Risco de manutenção.

### C. Solver próprio para tudo
- Controle total. Custo muito alto e fácil errar.

**Recomendação: A**, mais XPBD próprio para cordas.

## Sistema de cordas

### O que o criador faz
Escolhe duas âncoras (poste, árvore, personagem, caixa), arrasta e a corda aparece com o **comprimento e a folga** certos. Escolhe o material: corda, cabo, corrente.

### Recursos
| Recurso | Descrição |
|---|---|
| **Corda** | Cadeia de partículas com restrição de distância |
| **Âncoras** | Em corpos, em pontos do terreno ou de malhas (`surface_attachment`, `LOCAL`), com deslocamento |
| **Comprimento variável** | Guincho: enrolar e desenrolar |
| **Corte** | Por script, por dano, por ferramenta |
| **Colisão** | Esferas varridas contra o mundo estático |
| **Tensão** | Evento "corda esticou demais" → pode romper |
| **Vento e água** | Arrasto simples usando o estado de clima e o vetor de fluxo |
| **Malha** | Tubo gerado por varredura de perfil ao longo da curva (`sweep`, `LOCAL`) |
| **Ponte de corda** | Preset: duas cordas laterais + tábuas ligadas (cadeia de corpos) |
| **Gancho e balanço** | Preset: corda presa ao personagem |
| **Tecido leve (depois)** | Mesmo solver com restrições de triângulo (bandeiras, cortinas) |

### Exemplo (ilustrativo)
```aipo
let ponte = rope.create(
    from = poste_a,
    to = poste_b,
    length = 12.0,
    segments = 24,
    material = "rope_hemp",
)
rope.on_cut(ponte, fn(e) { ui.say("A ponte caiu!") })
```

### Orçamento
- **Limite global de partículas de corda** com degradação automática (menos segmentos de cordas distantes).
- Passo **fixo** e subpassos fixos (determinístico).
- O número do limite sai de medição (S6), não de palpite.

## Personagem e "física leve"

| Tema | Proposta |
|---|---|
| Personagem | Controlador cinemático (degraus, rampas, plataformas móveis) |
| **Assistência** | Presets de acessibilidade: tempo de coiote, ajuda em beirada, queda suave |
| Gatilhos | Volumes que geram eventos (entrar, sair, dentro) |
| Raios e varreduras | Para visão, mira, IA, colocar objetos |
| Objetos dinâmicos | Poucos, com **orçamento** e "dormir" quando parados |
| Camadas | **Nomes de camada** (`player`, `world`, `water`), não máscaras de bits (a Aipo não tem operadores de bits, `LOCAL`) |

Perfis: `physics.quality = "light"` (padrão) e `"standard"`.

## Determinismo
- Passo fixo com interpolação no desenho.
- Ordem fixa por id.
- O modo determinístico do Rapier reduz diferenças entre máquinas, mas é **meta**, não garantia (`HIPÓTESE`).
- Se um dia houver *rollback netcode*, o solver de cordas precisa ser reentrante e guardar estado compacto (veja [online](07-online.md)).

## Decisões ligadas

`DR-08` física · `DR-09` online.
