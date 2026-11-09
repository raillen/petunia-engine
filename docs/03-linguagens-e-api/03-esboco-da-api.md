# Desenho da API da Petunia Engine (Módulos Core)

**Revisão:** 2026-11-02 · **Estado:** o bloco `transform` abaixo está implementado ([DR-43](../06-decisoes/registro-de-decisoes.md)); o resto desta página continua proposta.
**Arquivo de referência:** substitui o rascunho anterior de `docs/03-linguagens-e-api/03-esboco-da-api.md`.

---

## 1. Filosofia e Regras de Ouro

A API segue os princípios fundamentais da linguagem **Aipo**:

1. **Dados separados de comportamento:** entidades são apenas identificadores inteiros (`entity: Int`). O estado reside em tabelas.
2. **Módulos por domínio:** funções são organizadas por substantivos do domínio (`transform`, `physics`, `input`, `sound`, `world`).
3. **Argumentos nomeados:** chamadas usam argumentos nomeados explícitos para clareza em voz alta e auxílio do autocomplete.
4. **Sem strings mágicas no dia a dia:** funções de módulo substituem acessos manuais a tabelas frequentes.
5. **Tipos de valor matemático:** `vec3`, `color` e `quat` copiam por valor e têm operações aritméticas diretas.

---

## 2. Visão Geral dos Módulos Core

| Módulo | Responsabilidade | Exemplo rápido |
|---|---|---|
| `transform` | Posição, rotação, escala e hierarquia no espaço 3D | `transform.rotate(entity, y = 90.0 * dt)` |
| `world` | Criação, destruição e consulta de entidades e tabelas | `let mob = world.spawn(prefab = "Goblin")` |
| `physics` | Raycasts, sobreposições, impulsos e velocidades | `physics.raycast(from = pos, direction = dir)` |
| `input` | Ações e eixos mapeados (teclado, mouse, gamepad) | `input.pressed("jump")` |
| `camera` | Câmera ativa, projeção e alvos | `camera.look_at(cam, target = player)` |
| `sound` | Áudio 2D e áudio 3D posicional no espaço | `sound.play("coin", at = entity)` |
| `time` | Delta time, tempo decorrido e temporizadores | `time.after(seconds = 2.0, do_this = fn() { ... })` |

---

## 3. Especificação Detalhada por Módulo

### 3.1. `transform` — Espaço 3D e Orientação

Gerencia a presença espacial da entidade no mundo.

```aipo
# O vetor inteiro é um Vec3. Um eixo continua uma função própria.
var pos = transform.position(entity)
pos.x = pos.x + 5.0 * dt
transform.set_position(entity, pos)
transform.move(entity, Vec3{ x: 0.0, y: 0.0, z: 5.0 * dt })
transform.move_y(entity, 2.0 * dt)

var rot = transform.rotation(entity)   # graus Euler
rot.y = rot.y + 90.0 * dt
transform.set_rotation(entity, rot)
transform.rotate(entity, Vec3{ x: 0.0, y: 90.0 * dt, z: 0.0 })
transform.rotate_y(entity, 90.0 * dt)

let scl = transform.scale(entity)
transform.set_scale(entity, Vec3{ x: 2.0, y: 2.0, z: 2.0 })

let frente = transform.forward(entity)
let direita = transform.right(entity)
let cima = transform.up(entity)
```

`world.get` de uma coluna `Vec3` ou `Color` devolve a mesma struct. `world.row` devolve um dicionário cujas células vetor e cor são structs. A hierarquia do editor (`Transform.parent`) ainda não tem função de script.

---

### 3.2. `world` — Ciclo de Vida e Entidades

Gerencia o ciclo de vida das entidades no mundo de jogo.

```aipo
# Criação e destruição
let moeda = world.spawn(prefab = "Moeda", at = vec3(0, 1, 0))
let id_novo = world.create()
world.remove(moeda)
let existe = world.exists(moeda)

# Consultas por etiquetas e critérios
let inimigos = world.find(tag = "Inimigo")
let proximo  = world.find_nearest(tag = "Item", from = pos, radius = 10.0)

# Acesso a tabelas genéricas ou customizadas (quando necessário)
world.set_data(entity, table = "Health", column = "points", value = 100)
let vida = world.get_data(entity, table = "Health", column = "points")
let tem_vida = world.has(entity, table = "Health")
```

---

### 3.3. `physics` — Colisões e Consultas Espaciais

Consultas de raycast, sobreposição e forças físicas.

```aipo
# Raycast (retorna registro com hit, point, normal, entity)
let hit = physics.raycast(
    from = pos,
    direction = dir,
    max_distance = 50.0,
    ignore = entity
)

if hit.has_hit {
    log.info("Acertou objeto:", hit.entity)
    log.info("Ponto do impacto:", hit.point)
}

# Verificações de área
let alvos = physics.overlap_sphere(center = pos, radius = 5.0, tag = "Enemy")

# Cinemática e Dinâmica
physics.apply_impulse(entity, impulse = vec3(0, 10, 0))
physics.set_velocity(entity, velocity = vec3(0, 0, 5))
let vel = physics.get_velocity(entity)
let no_chao = physics.is_grounded(entity)
```

---

### 3.4. `input` — Entradas Abstratas

Isola a lógica do tipo físico de controle (teclado, mouse ou joystick).

```aipo
# Ações digitais (botões)
if input.pressed("jump") {
    physics.apply_impulse(entity, impulse = vec3(0, 8, 0))
}

if input.held("fire") {
    arma.disparar(entity)
}

if input.released("crouch") {
    // levantou
}

# Entradas analógicas e direcionais
let direcao = input.vector2("move")   # devolve vec2(x, y) normalizado
let mira    = input.vector2("look")   # delta do mouse ou analógico direito
let gatilho = input.axis("accelerate") # float de 0.0 a 1.0
```

---

### 3.5. `camera` — Controle de Visão

Controle de projeção e pontos de vista.

```aipo
# Câmera ativa
camera.set_active(camera_principal)
let cam = camera.get_active()

# Parâmetros
camera.set_fov(cam, fov = 75.0)

# Projeção espacial
let raio_mouse = camera.screen_to_ray(cam, screen_pos = input.mouse_position())
let tela_pos   = camera.world_to_screen(cam, world_point = pos)
```

---

### 3.6. `sound` — Áudio do Jogo

Reprodução de sons 2D e áudio 3D atenuado por distância.

```aipo
# Áudio 2D (música, interface)
sound.play_ui("click")
sound.play_music("tema_floresta", fade_seconds = 2.0)

# Áudio 3D (espacializado no mundo)
sound.play("explosao", at = pos, volume = 1.0, pitch = 1.0)
sound.play_attached("passos", target = entity, loop = true)
```

---

### 3.7. `time` — Controle Temporal

Tempo, pausas e agendamentos determinísticos.

```aipo
let dt    = time.delta()       # segundos desde o quadro anterior
let total = time.elapsed()     # tempo total de jogo em segundos

# Temporizadores do jogo (seguros e determinísticos)
time.after(seconds = 3.0, do_this = fn() {
    world.remove(bomba)
})

time.every(seconds = 1.0, do_this = fn() {
    log.info("1 segundo passou")
})
```

---

## 4. Exemplo Completo: Controlador de Personagem

Um script completo demonstrando os módulos trabalhando juntos de forma legível:

`scripts/player_controller.aipo`

```aipo
fn on_update(entity, dt) {
    // 1. Ler entrada analógica de movimento
    let move_input = input.vector2("move")

    // 2. Calcular movimento baseado na orientação do jogador
    let forward = transform.forward(entity)
    let right   = transform.right(entity)
    let deslocamento = (forward * move_input.y) + (right * move_input.x)

    // 3. Aplicar velocidade física
    let velocidade_atual = physics.get_velocity(entity)
    let velocidade_horizontal = deslocamento * 6.0

    physics.set_velocity(entity, vec3(
        velocidade_horizontal.x,
        velocidade_atual.y,
        velocidade_horizontal.z
    ))

    // 4. Pulo
    if input.pressed("jump") and physics.is_grounded(entity) {
        physics.apply_impulse(entity, impulse = vec3(0, 7.5, 0))
        sound.play("jump", at = entity)
    }

    // 5. Rotação com o mouse
    let mouse = input.vector2("look")
    transform.rotate(entity, y = mouse.x * 120.0 * dt)
}
```

---

## 5. Próximos Passos de Implementação

1. Validar e homologar este contrato de API.
2. Registrar o módulo nativo `transform` em `crates/petunia_script/src/api.rs`.
3. Adicionar o módulo nativo `input` e `physics` na esteira da Fase 1 e Fase 2.
