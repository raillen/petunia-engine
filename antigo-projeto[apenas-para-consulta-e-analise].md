# Petunia Game Engine 3D — Especificação consolidada de produto, sistemas e funcionalidades

> **Tipo:** documento de visão e requisitos funcionais para retomada do desenvolvimento.  
> **Data de consolidação:** 24 de outubro de 2026.  
> **Projeto:** [WASDst/petunia-game-engine](https://github.com/WASDst/petunia-game-engine).  
> **Escopo:** a **game engine 3D** projetada originalmente, e **não** a engine 2D de Python/PySide6 desenvolvida em uma fase posterior.  
> **Situação:** especificação histórica consolidada; este documento **não afirma** que todos os recursos estejam implementados.

---

## 0. Como interpretar esta especificação

O planejamento passou por diferentes fases. Para evitar misturar decisões incompatíveis, usamos quatro etiquetas:

| Etiqueta | Significado |
|---|---|
| **[Decidido]** | Direção explicitamente aprovada nas discussões da Petunia Game Engine 3D. |
| **[Planejado]** | Feature registrada nos objetivos originais P00–P06 ou na arquitetura posterior; implementação ainda exige verificação. |
| **[Proposta]** | Extensão técnica ou funcional recomendada para completar o produto; **não** é decisão histórica aprovada. |
| **[Fora do MVP]** | Recurso excluído das primeiras entregas, embora possa ser considerado futuramente. |

**Regra de precedência:** quando houver divergência, a arquitetura refinada de outubro de 2026 prevalece sobre o spike inicial de setembro de 2026. A versão mais nova estabelece Rust, editor Qt/CXX-Qt, OpenGL, scripting Luau ou Aipo por projeto, bibliotecas compartilhadas e Level Builder integrado. Os objetivos antigos P00–P06 seguem como **inventário de requisitos**, não como stack final.

> Observação importante: o repositório preservou objetivos originais Rust/3D/Aipo, mas posteriormente também recebeu trabalho para um produto 2D chamado `tinyEngine`. Portanto, não se deve interpretar todos os arquivos, estados de conclusão e roadmaps atuais desse repositório como evidência da conclusão da engine 3D.

---

## 1. Visão de produto

A **Petunia Game Engine 3D** é concebida como uma engine compacta, modular, acessível e centrada na produção prática de jogos **3D e 2.5D**, com uma experiência autoral visual que não impede desenvolvimento avançado.

**Objetivos principais:**

1. Permitir construir um nível 3D jogável rapidamente, sem exigir um DCC externo para cada parede, rampa, corredor, plataforma ou porta.
2. Tratar **Level Builder**, cenas, triggers, gameplay, assets e scripts como partes conectadas de um único fluxo.
3. Rodar bem em computadores modestos, com dependências enxutas, opções de qualidade e ferramentas previsíveis.
4. Oferecer **programação textual ou visual** sem APIs semânticas conflitantes.
5. Reutilizar bibliotecas de geometria e intercâmbio com **Petunia3D**, mantendo os aplicativos independentes.
6. Tornar o runtime desacoplado do editor e da interface Qt.
7. Permitir desenvolvimento por pessoas iniciantes e produção disciplinada por equipes e agentes de código.
8. Organizar ferramentas por domínio: construir cenário, posicionar entidades, definir regras, animar, testar e exportar.

**Tipos de jogos adequados:** FPS compacto, exploração em primeira/terceira pessoa, puzzles 3D, aventura, plataforma 3D, dungeon crawler, jogos estilizados low-poly, arenas, jogos 2.5D, experiências narrativas e protótipos interativos.

**Não perseguir inicialmente:** renderer AAA, suíte DCC completa, física de rigidbody complexa, networking massivo, marketplace, renderização cinematográfica sofisticada, criação de shaders por grafos complexos e compatibilidade universal com consoles.

---

## 2. Stack e decisões técnicas

| Área | Escolha consolidada | Situação |
|---|---|---|
| Linguagem do core, runtime e editor controllers | **Rust** | [Decidido] |
| Editor desktop | **Qt 6 / CXX-Qt**, com interface Qt desacoplada da engine | [Decidido] |
| Backend gráfico de referência na arquitetura refinada | **OpenGL** leve | [Decidido] |
| Backend gráfico do spike inicial | `wgpu`, `winit`, `glam` | [Histórico; substituído como direção principal] |
| GUI do spike inicial | `egui` atrás de feature flag | [Histórico; não é UI final] |
| Execução textual de gameplay | **Luau ou Aipo**, uma linguagem ativa por projeto na V1 | [Decidido] |
| Integração de scripts | `ScriptHost` / Host API única | [Decidido] |
| Autoria visual | Event Sheets, grafos especializados e comportamentos via **Gameplay IR** | [Decidido] |
| Simulação | **RuntimeWorld** separado de modelos de autoria, com ECS ou organização equivalente pragmaticamente enxuta | [Decidido] |
| Construção de geometria | Primitivas e **brushes convexos**; CSG por etapas | [Decidido] |
| Intercâmbio com o Petunia3D | **GLB/glTF** com metadados opcionais | [Decidido] |
| Bibliotecas compartilhadas | Repositório próprio de crates pequenas e versionadas (`petunia-libs`) | [Decidido] |
| Objetivos de exportação | Windows e Linux prioritários; Web/WASM planejado originalmente | [Decidido para prioridade desktop; Web planejado] |
| Plataforma do editor | Desktop; ferramentas organizadas para hardware modesto | [Decidido] |

### 2.1 Princípios de arquitetura

- A engine não depende do editor; o editor depende de APIs públicas da engine.
- Qt/CXX-Qt não entra no core nem no runtime standalone.
- A representação geométrica compartilhada **não** implica dependência de execução entre Petunia3D e Petunia Game Engine.
- Scripts não acessam estruturas internas do `RuntimeWorld` diretamente: usam handles/IDs e Host API.
- A edição autoral e a simulação ativa possuem ciclos de vida diferentes.
- Os backends de renderização e scripting ficam atrás de interfaces substituíveis.
- Não adicionar um framework grande só para resolver um caso pequeno.
- Nomes explícitos, tipos fortes, funções bem separadas, documentação e testes em todas as fronteiras.

### 2.2 Arquitetura macro ilustrativa

```text
petunia-libs (crates independentes/versionadas)
  ├─ ids / math / geometry / mesh
  ├─ geometry-operations / convex / csg
  ├─ assets / interchange / gltf
  └─ contracts comuns

petunia-game-engine
  ├─ project-model       autorias, cenas, referências
  ├─ runtime-world       entidades, transform, tick, ciclo de vida
  ├─ renderer            interfaces GPU, câmeras, luzes, materiais
  ├─ physics             colisores, queries e cinemática
  ├─ assets              importação, cache, dependências
  ├─ gameplay            tipos, regras, signals, behaviors e IR
  ├─ scripting-host      ScriptHost API, despacho e capabilities
  │   ├─ scripting-luau
  │   └─ scripting-aipo
  ├─ level-builder       brushes, snapping e CSG
  ├─ navigation          waypoints, A* e agents
  ├─ audio / particles / environment
  ├─ editor              Qt/CXX-Qt, ferramentas autorais
  ├─ runtime-player      executável standalone, sem Qt
  └─ exporter            builds, validação e empacotamento

Petunia3D  <-- GLB + metadados / biblioteca geométrica --> Game Engine
```

> Os nomes acima são **contratos arquiteturais sugeridos**, não uma garantia dos nomes reais das crates do repositório.

---

## 3. Organização do editor 3D

### 3.1 Workspace e navegação

- [Decidido] Editor visual com painéis de hierarquia, propriedades, assets e viewport.
- [Planejado] Áreas de trabalho distintas: **Scene**, **Level Builder**, **Gameplay**, **Assets**, **Playtest** e **Build**.
- [Proposta] Layouts salvos por workspace, painéis redimensionáveis, docking e perfis compacto/avançado.
- [Proposta] Paleta de comandos pesquisável, busca global, favoritos e histórico de ferramentas recentes.
- [Proposta] Breadcrumbs e indicadores de contexto para saber sempre qual cena, entidade, prefab ou brush está sendo editado.

### 3.2 Viewport

- [Planejado] Viewport acelerada por GPU e orientada a edição 3D.
- [Planejado] **Fly camera** com WASD e mouse look.
- [Planejado] Gizmos para mover, rotacionar e escalar.
- [Planejado] Snap à grade configurável.
- [Proposta] Câmeras orbit, pan e zoom; foco na seleção; vistas frontal, lateral, superior e perspectiva.
- [Proposta] Overlays de pivôs, colisores, volumes trigger, waypoints, frustum de câmera e bounds de entidades.
- [Proposta] Modos de visualização: texturizado, wireframe, flat/albedo, iluminação, colisores e debug.
- [Proposta] Prévia de câmera do jogo em painel secundário.
- [Proposta] Multisseleção com seleção por hierarquia e filtros.

### 3.3 Hierarquia e Inspector

- [Planejado] Scene Tree com entidades, parenting e transformação local/global.
- [Planejado] Inspector de componentes, behaviors e propriedades de triggers.
- [Proposta] Habilitar/desabilitar componentes e entidades.
- [Proposta] Duplicação, reparenting, instanciamento e substituição de prefabs.
- [Proposta] Edição em lote de múltiplas seleções.
- [Proposta] Indicação de overrides de prefab.
- [Proposta] Ajuda contextual, unidades, limites e validação de tipos nos campos numéricos.

### 3.4 Qualidade da experiência

- [Decidido] Baixa carga cognitiva e facilidade de uso.
- [Proposta] Progressive disclosure: opções comuns visíveis, configurações avançadas sob demanda.
- [Proposta] Focus Mode para reduzir a interface à tarefa atual.
- [Proposta] Navegação extensiva por teclado, foco visível, zoom de UI, ícones com rótulos e atalhos remapeáveis.
- [Proposta] Mensagens de erro que indiquem causa, objeto envolvido e correção possível.

---

## 4. Project Model, cenas e entidades

### 4.1 Estrutura de projeto

- [Decidido] Formatos autorais textuais, difáveis e versionáveis.
- [Planejado] Arquivo de mapa inicial **`map.tiny.json`** (nome histórico, sujeito a alteração/compatibilidade).
- [Planejado] CLI para validação de projeto/mapa sem abrir editor.
- [Proposta] Projeto com manifesto, cenas, prefabs, assets, gameplay, input, builds e configurações.
- [Proposta] IDs estáveis para entidade, cena, componente, asset, brush e regra.
- [Proposta] Caminhos relativos, migrações de schema, versões e referências por ID.

### 4.2 Entidades

- [Decidido] `RuntimeWorld` independente do modelo autoral da cena.
- [Proposta] Entidades com identificador estável, nome, estado, hierarquia, Transform e componentes.
- [Proposta] Ciclo de vida de criação, ativação, atualização, desativação e destruição.
- [Proposta] Spawn/despawn em runtime; persistência opcional entre cenas.
- [Proposta] Tags, grupos e filtros de busca.
- [Proposta] Sistema de eventos de ciclo de vida.

### 4.3 Transform e coordenadas

- [Planejado] Posição, rotação e escala 3D; transformação de brushes por gizmos.
- [Proposta] Hierarquia de transforms e conversões local/world.
- [Proposta] Pivôs ajustáveis e snapping absoluto/relativo.
- [Proposta] Convenção única de eixos, unidades e handedness documentada.

### 4.4 Prefabs e instâncias

- [Planejado] Prefabs iniciais: **Porta**, **Pickup** e **Spawn Point**.
- [Planejado] Esses prefabs configuram colisão, navegação e trigger de forma integrada quando necessário.
- [Proposta] Herança, variantes, overrides identificáveis e rebase controlado.
- [Proposta] Prefabs reutilizáveis de inimigo, plataforma, elevador, checkpoint e projétil.
- [Proposta] Conversão de uma seleção de entidades em prefab.

---

## 5. Level Builder — diferencial do produto

O Level Builder cria a geometria de um nível **dentro da própria engine**, sem tentar substituir ferramentas avançadas de modelagem 3D.

### 5.1 Primitivas e brushes

- [Planejado] Criar **caixas, rampas e cilindros** pela paleta.
- [Decidido] Base de modelagem por **brushes convexos**.
- [Planejado] Editar posição, rotação e escala.
- [Planejado] Grid snap com tamanho configurável.
- [Planejado] Undo/Redo via CommandDispatcher transacional.
- [Planejado] Salvar/reabrir mapa preservando IDs dos brushes.
- [Proposta] Brush de escadas, planos inclinados, arcos e formas poligonais convexas.
- [Proposta] Materiais por face, UV básico, duplicação e agrupamento.
- [Proposta] Edição de faces/vertices de brushes dentro dos limites de convexidade.
- [Proposta] Operações extrude/clip para brushes, com validação topológica.

### 5.2 CSG e geometria

- [Decidido] O Level Builder poderá usar operações geométricas compartilhadas com `petunia-libs`.
- [Planejado] CSG booleano **não entra na primeira etapa**.
- [Proposta] Adoção incremental de Union, Subtract e Intersect com resultado validado e reversível.
- [Proposta] Geometria de autoria não destrutiva sempre que viável; mesh triangulada derivada para render/colisão.
- [Proposta] Cache das operações e recomputação apenas da região modificada.
- [Proposta] Ferramentas de detecção de brushes degenerados, superfícies invertidas e interseções inválidas.

### 5.3 Fluxo de produção

1. Criar sala com brushes.
2. Ajustar paredes, rampas e plataformas via gizmos e snap.
3. Aplicar materiais ou cores-base.
4. Inserir portas, pickups, triggers e spawn points.
5. Configurar a movimentação do jogador.
6. Executar a cena e depurar colisão/navegação.
7. Transferir assets que precisam de modelagem avançada ao Petunia3D.
8. Reimportar GLB sem destruir posicionamento e identidade das instâncias.

### 5.4 Integração com Petunia3D

- [Decidido] **GLB é o formato primário** de intercâmbio; metadados opcionais preservam intenções de edição.
- [Decidido] Edição avançada permanece no Petunia3D; Level Builder concentra construção rápida.
- [Decidido] Bibliotecas de geometria são compartilhadas sem acoplar os aplicativos.
- [Planejado] **Live Link** na fase de integração da arquitetura posterior.
- [Proposta] Round trip com identidade de objetos, materiais e transforms quando possível.
- [Proposta] Aviso sobre edições que não podem ser reconstruídas a partir de GLB triangulado.
- [Proposta] Atualização incremental e botão `Reimport/Refresh` por asset.

---

## 6. Renderização 3D

### 6.1 Núcleo do renderer

- [Decidido] Backend OpenGL leve na arquitetura refinada.
- [Planejado] Abstração que separa comandos de renderização, cenas e recursos GPU.
- [Planejado] Renderização de geometria de brushes e meshes importadas.
- [Proposta] Frustum culling, batching, cache de meshes e upload incremental.
- [Proposta] Ordenação de objetos opacos/translúcidos, depth test, blending e render targets.
- [Proposta] Métricas de draw calls, triângulos, frame time, uso de memória de texturas e upload.
- [Proposta] Caminho gráfico básico para GPUs modestas, evitando depender de compute shaders.

### 6.2 Geometria, materiais e texturas

- [Planejado] Meshes simples derivados de brushes.
- [Decidido] Intercâmbio de meshes/materials por GLB/glTF.
- [Proposta] Materiais **unlit**, albedo/base-color e lit básicos; alpha mask e transparency.
- [Proposta] Normal maps, emissão, roughness/metallic conforme perfil e capacidade do backend.
- [Proposta] Texture atlases e batching por material.
- [Proposta] Materiais por face para brushes e importação consistente de UV.
- [Proposta] Inspector de materiais com preview simples.

### 6.3 Iluminação

- [Proposta] Luz ambiente, luz direcional e luz pontual.
- [Proposta] Sombras opcionais por perfil de qualidade.
- [Proposta] Controles de exposição, fog e cor ambiente.
- **[Fora do MVP]** SSR, global illumination avançada, lightmapping complexo e renderer AAA.

### 6.4 Câmeras

- [Planejado] Flycam no editor e **FPSController** no runtime.
- [Proposta] Câmeras perspective/orthographic, FOV, near/far clip e viewport.
- [Proposta] Third-person, follow, fixed, orbit, camera zones e câmera 2.5D por presets.
- [Proposta] Shake, zoom, interpolação e transição entre câmeras.
- [Proposta] Suporte a ângulos pseudo-isométricos/dimétricos para jogos estilizados.

### 6.5 Billboards e sprites em espaço 3D

- [Planejado] Billboards 2D/3D com alinhamento **cilíndrico** e **esférico**.
- [Proposta] Sprite sheets e animação de sprites em planos 3D.
- [Proposta] Ordenação de transparência e controle de facing.
- [Proposta] Elementos de UI em world space.
- [Proposta] Vegetação, fumaça, marcadores e efeitos leves com billboards.

---

## 7. Física, colisão e interações

### 7.1 Física mínima

- [Planejado] **Movimento cinemático** como primeira estratégia.
- [Fora do MVP] Física completa de rigidbodies/constraints.
- [Proposta] Colisores AABB, esfera, cápsula e primitivas convexas.
- [Proposta] Interseção, depenetração, movimento com sliding e slope limit.
- [Proposta] Queries raycast, overlap e shape cast com API unificada.
- [Proposta] Collision layers/masks separadas de render layers.
- [Proposta] Prévia de colisão e contatos no modo debug.

### 7.2 Controllers

- [Planejado] FPSController com **WASD, mouse look e pulo**.
- [Proposta] CharacterController cinemático reutilizável.
- [Proposta] Gravidade, velocidade, aceleração, step height, jump e chão detectado.
- [Proposta] Top-down 3D e third-person controller como behaviors oficiais.
- [Proposta] Plataformas móveis e suporte a triggers de interação.

### 7.3 Volumes e triggers

- [Planejado] **Pintura/criação de volumes trigger**.
- [Planejado] Eventos **OnEnter**, **OnExit** e **OnInteract**; evento OnStay é extensão possível.
- [Planejado] Lista declarativa `condição → ação`, serializada em arquivos de texto.
- [Planejado] Aipo executando a ação dentro da Host API (na primeira versão do plano).
- [Decidido] Na arquitetura refinada, mesma capacidade visual com backend **Luau ou Aipo** conforme projeto.
- [Proposta] Trigger filters por tag, grupo, tipo de entidade e camada.
- [Proposta] Debounce, cooldown, once/repeat e debug do motivo de ativação.

### 7.4 Interação contextual

- [Planejado] Prefabs Porta/Pickup/Spawn Point integrados com triggers.
- [Proposta] Interact ray, highlight de objeto, distância máxima e prompts de ação.
- [Proposta] Ações como abrir, coletar, ativar, teletransportar, trocar cena, tocar áudio e enviar Signal.
- [Proposta] Condições de inventário, quest ou estado de jogo por API tipada.

---

## 8. Gameplay System e programação visual

### 8.1 Princípio essencial

- [Decidido] **Gameplay IR** é a representação semântica compartilhada.
- [Decidido] A UI de regras/visual scripting **não** deve ser um espelho de ASTs internas da Aipo/Luau.
- [Decidido] Event Sheets, Node Graph e Behaviors são visões/ferramentas de autoria acima da mesma Gameplay API.
- [Decidido] Mesmas ações e condições acessíveis por script e por ferramentas visuais.

### 8.2 Event Sheets

- [Planejado] Lista de eventos baseada em condições e ações, primeiro para triggers.
- [Proposta] Regras do tipo `WHEN / IF / DO` com subcondições e grupos.
- [Proposta] Filtros por entidade/tag/grupo, variáveis, Callables e Signals.
- [Proposta] Resolução de conflitos e ordem de execução explícita.
- [Proposta] Breakpoints, step e histórico de execução.

### 8.3 Node Graphs especializados

- [Decidido] Autoria por grafos é possível, **sem criar um grafo genérico obrigatório para tudo**.
- [Proposta] Grafos apropriados para State Machines, Behavior Trees e fluxos que se beneficiem de relações espaciais.
- [Proposta] Editor outline/tabular alternativo aos grafos.
- [Proposta] Identidade estável entre nós e entidades do Gameplay IR.

### 8.4 Behaviors de uso imediato

- [Planejado] **Patrol** e **Chase**.
- [Planejado] Triggers e prefabs com comportamentos configuráveis pelo Inspector.
- [Proposta] Rotate, LookAt, MoveTo, Follow, Interact, Pickup, Door, Health, Projectile e Spawn.
- [Proposta] Behaviors compostos; lifecycle e prioridades determinísticas.
- [Proposta] Presets reutilizáveis e criação de behaviors por script.

### 8.5 Signals e Event Bus

- [Decidido] API de gameplay semântica compartilhada entre linguagens/autoria visual.
- [Proposta] Signals de projeto com parâmetros tipados: `DoorOpened`, `EnemyDefeated`, `ItemCollected`.
- [Proposta] Subscrever, publicar e depurar emissões no runtime.
- [Proposta] Conversão controlada entre eventos de física, UI, animação e gameplay.

### 8.6 Estado e dados

- [Proposta] Variáveis de escopo global, cena, entidade e componente.
- [Proposta] State Machines de locomotion, combat e interações complexas.
- [Proposta] Bindings reativos de variáveis para UI, animação e propriedades.
- [Proposta] Save flags e estado de triggers persistente.

---

## 9. Scripting: Luau e Aipo

### 9.1 Decisão principal

- [Decidido] Suporte a **Luau e Aipo** pela mesma **Script Host API**.
- [Decidido] Na V1, o usuário escolhe **uma única linguagem textual ativa para o projeto**, ao criá-lo.
- [Decidido] Ações visuais, behaviors e APIs não dependem da linguagem escolhida.
- [Decidido] A **Gameplay API nasce de um schema único**; dele derivam bindings/documentação/metadata onde cabível.
- [Decidido] Backends isolados e versionáveis.

### 9.2 Script Host API

Contratos conceituais:

```text
ScriptHost
  on_init(entity)
  on_update(entity, delta)
  on_collision(entity, other)
  on_trigger_enter(entity, other)
  on_trigger_exit(entity, other)
  on_interact(entity, actor)
  on_destroy(entity)

GameplayAPI
  scene.find / scene.spawn / scene.destroy
  entity.get_component / entity.set_property
  transform.get / transform.set
  physics.raycast / physics.overlap
  input.get_action
  audio.play
  particles.emit
  events.emit / events.subscribe
  navigation.find_path
```

**Nota:** são nomes ilustrativos de contratos, não declarações de API já implementadas.

### 9.3 Segurança, previsibilidade e testes

- [Decidido] Script host desacoplado da World; scripts usam apenas capacidades expostas.
- [Proposta] Handles geracionais ou IDs validados para impedir referências obsoletas.
- [Proposta] Budgets de execução e rastreamento por script.
- [Proposta] Hot-reload para desenvolvimento, com regras de preservação de estado bem definidas.
- [Proposta] Isolamento de erros por entidade/script.
- [Proposta] Determinismo sob tick fixo quando a API e os scripts o permitirem.
- [Proposta] Mesmo script testável headless.

### 9.4 Editor textual

- [Proposta] Realce de sintaxe, autocomplete da Gameplay API, diagnósticos e navegação para recursos.
- [Proposta] Go to definition/find references, documentação contextual e assinatura de funções.
- [Proposta] Logs correlacionados com entidades e origem do evento.
- [Proposta] Templates iniciais para ambas as linguagens.

---

## 10. Terreno e ambientes

### 10.1 Terreno

- [Planejado] Terrain por **heightmap**.
- [Planejado] Pincéis **Raise**, **Lower** e **Smooth**.
- [Proposta] Paint de camadas, flatten, erosion simples e stamps em fases posteriores.
- [Proposta] Resolução/LOD ajustáveis conforme orçamento de desempenho.
- [Proposta] Colisão e nav atualizados após edição de heightmap.

### 10.2 Água

- [Planejado] Água inicialmente como **plano com scroll de textura/material**.
- [Planejado] Volume/trigger que marca contato com água (`isWater`, nome histórico).
- [Proposta] Profundidade e velocidade de fluxo configuráveis como gameplay data.
- [Fora do MVP] Reflexos físicos/realtime completos e simulação avançada de fluidos.

### 10.3 Clima, atmosfera e fog

- [Planejado] Presets **chuva, neve e nevoeiro** aplicáveis rapidamente.
- [Planejado] Clima tratado como parâmetros de emissores/efeitos, não frameworks de clima independentes.
- [Proposta] Fog por distância, ambiente e intensidade; orientação direcional de vento para emissores.
- [Proposta] Presets de luz/ambiente para interior, exterior e dungeon.

### 10.4 Camadas e ambientação

- [Proposta] Decoração com meshes, sprites billboard e decals.
- [Proposta] Grupos de objetos e scatter de props/vegetação.
- [Proposta] Occlusion/culling configurável, com fallback simples.

---

## 11. Partículas, VFX e decals

### 11.1 Emissores de partículas

- [Planejado] Presets **fogo, fumaça, faíscas e poeira**.
- [Planejado] Propriedades: taxa, lifetime, velocidade, gravidade e **color over life**.
- [Planejado] Caminho CPU com limite histórico de até **5.000 partículas** para o MVP daquela fase.
- [Proposta] Burst, forma do emissor, escala/opacidade over life, billboard facing e pooling.
- [Fora do MVP] Compute shaders e GPU particles complexas.

### 11.2 Decals

- [Planejado] Marcas de tiro e sangue com **pool** e fade.
- [Proposta] Decals de sujeira, desgaste, pistas e sinais.
- [Proposta] Limites por cena, material e distância.
- [Proposta] Ferramentas de prévia e posicionamento de decal no Level Builder.

### 11.3 Tweens e efeitos temporais

- [Proposta] Interpolação de Transform, cor, opacidade, câmera, UI e áudio.
- [Proposta] Presets de easing, duração, delays e callbacks.
- [Proposta] Integração com Event Sheets, animações e cutscenes.

---

## 12. Navegação e inteligência artificial

### 12.1 Navegação

- [Planejado] Editor de **waypoints** com criação/conexão por clique e arrasto.
- [Planejado] Grafo de navegação serializado em texto.
- [Planejado] **A\*** para encontrar caminho entre waypoints conectados.
- [Planejado] Preview de caminhos no editor.
- [Fora do MVP] Navmesh automática estilo Recast.

### 12.2 AI Behaviors

- [Planejado] **Patrol** percorrendo waypoints em ciclo.
- [Planejado] **Chase** perseguindo o jogador dentro de raio configurável.
- [Proposta] Investigate, Flee, Idle, Attack e ReturnToPatrol.
- [Proposta] Sensores de distância, visão, som e trigger.
- [Proposta] Behavior Trees, Blackboard e Utility AI como sistemas opcionais posteriores.
- [Fora do MVP] Coordenação tática complexa de grandes grupos.

### 12.3 Debug

- [Proposta] Renderização de waypoints, conexões, target e caminho atual.
- [Proposta] Estado e motivo da decisão de cada comportamento.
- [Proposta] Contadores de replanejamento, falhas de rota e nós expandidos no A*.

---

## 13. Animação e personagens

> A arquitetura refinada previa integração com assets GLB do Petunia3D, mas não houve decisão igualmente detalhada sobre um Animation System 3D completo. Os itens a seguir são **propostas de produto**, não features historicamente aprovadas.

- [Proposta] Importação de meshes com skeleton, skins e animation clips via glTF/GLB.
- [Proposta] Animation Player, loop, blend e callbacks por marcador.
- [Proposta] Animation State Machine associada a behaviors de movimento.
- [Proposta] Root motion opcional e blend controlado.
- [Proposta] Suporte a rigs editados/exportados pelo Petunia3D.
- [Proposta] Assinatura tipada para eventos de animação consumidos por scripts e regras visuais.
- [Proposta] Sprite animation sobre planos 3D para jogos 2.5D.
- [Fora do MVP] Ferramentas avançadas de rig, skinning e modelagem dentro da engine.

---

## 14. Input, controles e gameplay controllers

- [Planejado] Flycam do editor e FPSController do jogo com teclado/mouse.
- [Proposta] **Input Actions** sem hardcode de teclas na lógica.
- [Proposta] Input contexts como Gameplay, UI, Dialogue, Vehicle e Editor.
- [Proposta] Remapeamento em runtime, múltiplos bindings e resolução de conflito.
- [Proposta] Mouse, teclado e gamepad em fases incrementais.
- [Proposta] Eixos, deadzones, sensibilidade e inversão de mouse/câmera.
- [Proposta] Pausa, frame step e input recording para testes.

---

## 15. UI de jogos

> Camada de interface de jogo distinta da interface Qt do editor.

- [Proposta] Canvas 2D sobreposto e elementos opcionais no espaço 3D.
- [Proposta] Label, Image, Button, ProgressBar, Container, List, InventoryGrid e DialogueBox.
- [Proposta] Layout com anchor, alinhamento, padding e responsividade básica.
- [Proposta] Bindings a propriedades de entidades e dados de gameplay.
- [Proposta] HUD FPS: vida, munição, crosshair, interação e objetivos.
- [Proposta] Menu principal, opções, pausa e sistema de prompts.
- [Proposta] Eventos de UI publicados como Signals/Actions.

---

## 16. Áudio

- [Proposta] Reprodução de efeitos SFX e trilhas musicais.
- [Proposta] Fontes de áudio 2D e 3D com atenuação simples por distância.
- [Proposta] Categorias/buses: Music, SFX, UI, Voice.
- [Proposta] Play/Stop/Pause, loops, fades e volume/pitch.
- [Proposta] Eventos de áudio acessíveis a scripts, Event Sheets e Timeline.
- [Proposta] Debug de sons ativos, vozes simultâneas e clipping.
- [Fora do MVP] DAW embutida e processamento espacial avançado.

---

## 17. Diálogo, cutscenes e sistemas narrativos

> Esses sistemas apareciam na visão mais ampla de autoria visual, mas sua implementação 3D e cronograma específico **não** foram fechados. São subsistemas opcionais posteriores.

- [Proposta] Dialogue Editor estruturado: falas, personagens, escolhas, condições, sinais e variáveis.
- [Proposta] Timeline: câmera, transforms, animação, som, UI, eventos e transições.
- [Proposta] Trigger de cutscene por volume/evento.
- [Proposta] Skip e restauração correta de estado ao pular uma sequência.
- [Proposta] Editor de quests, inventário e dados compartilhados por plugin oficial.

---

## 18. Asset pipeline, importação e interoperabilidade

### 18.1 Catálogo

- [Decidido] Compatibilidade com GLB/glTF para cenas, meshes e intercâmbio.
- [Proposta] Asset IDs estáveis, source path, import settings, hash, caches, thumbs e grafo de dependências.
- [Proposta] Import/reimport automático com File Watcher.
- [Proposta] Detecção de referência quebrada e formato inválido.
- [Proposta] Asset Browser pesquisável com filtros, previews e favoritos.

### 18.2 Formatos

- **[Decidido] GLB/glTF:** formato principal de intercâmbio 3D com metadados opcionais.
- **[Planejado] `map.tiny.json`:** formato textual do mapa no spike original.
- **[Proposta]** PNG/JPEG para imagens; WAV/OGG para áudio; formatos autorais explícitos para scenes/prefabs/rules.
- **[Proposta]** Migração compatível dos nomes/formatos antigos em vez de renomear silenciosamente.

### 18.3 Reimportação e atualização

- [Proposta] Atualizar conteúdo de GLB preservando referências estáveis sempre que a identidade puder ser mapeada.
- [Proposta] Separar transforms/overrides de instância do conteúdo binário do asset.
- [Proposta] Notificar perda de metadados ou incompatibilidade de rig/material na atualização.
- [Proposta] Live Link posterior com Petunia3D, sem requerer os dois softwares abertos para builds finais.

---

## 19. Editor de gameplay, inspeção e depuração

### 19.1 Controles de execução

- [Planejado] `tiny run` executa cenário/mapa; `tiny check` valida o projeto headless no spike inicial.
- [Proposta] Play, Pause, Stop, Single Step, Restart Scene e Run from Here.
- [Proposta] Execução standalone e editor usando o mesmo core de simulação.

### 19.2 Runtime Inspector

- [Proposta] Ver componentes, transforms, scripts, triggers e estados ativos.
- [Proposta] Filtrar entidades por nome, ID, tag, cena e tipo.
- [Proposta] Watches e correlação entre variável/script/regra/evento.

### 19.3 Debugger de gameplay

- [Proposta] Breakpoints em regras visuais e scripts.
- [Proposta] Histórico de execução de triggers e ações.
- [Proposta] Explicar por que uma condição falhou.
- [Proposta] Event trace tipado com origem e tempo.
- [Proposta] Medição de tempo por script/sistema/regra.

### 19.4 Profiler

- [Proposta] FPS, frame time, GPU/CPU, física, gameplay, navegação, draw calls, triângulos e uso de texturas.
- [Proposta] Budget por subsistema e presets de hardware modesto.
- [Proposta] Links entre gargalos e assets/regras específicos.

---

## 20. Agentes de código, MCP e automação

### 20.1 Ferramentas MCP históricas

- [Planejado] **`map.get`** — inspecionar mapa.
- [Planejado] **`map.set`** — editar configuração de mapa.
- [Planejado] **`brush.create`** — criar geometria.
- [Planejado] **`trigger.create`** — criar volumes trigger.
- [Planejado] **`event.list`** — consultar regras/eventos.
- [Planejado] **`play.test`** — rodar testes de gameplay.

### 20.2 Princípio de funcionamento

- [Planejado] Ferramentas MCP operam **sobre arquivos textuais/contratos sem exigir UI aberta**.
- [Decidido] Agentes devem usar APIs semânticas e operações rastreáveis, não cliques simulados na UI como interface principal.
- [Proposta] Dry-run, transações, validação, diff, confirmação para mudanças destrutivas e audit log.
- [Proposta] Comandos idempotentes quando possível, com retorno de erros estruturado.

### 20.3 Issue Browser / Project Doctor

- [Planejado] Detectar **porta sem trigger** e **spawn fora da navegação**.
- [Planejado] Oferecer **Quick Fix** aplicável em um clique.
- [Proposta] Detectar referências quebradas, meshes inválidas, colisores ausentes, caminhos inacessíveis, scripts com erro, assets não usados e gargalos óbvios.
- [Proposta] Diagnósticos com IDs estáveis e navegação até o objeto afetado.

---

## 21. Serialização, Undo/Redo e colaboração

- [Planejado] CommandDispatcher transacional para edição de brushes.
- [Planejado] Undo/Redo sem limite lógico artificial no objetivo P01 original (implementação pode demandar política de memória).
- [Decidido] Arquivos de projeto/maps difáveis e amigáveis ao Git.
- [Proposta] IDs persistentes; ordem de campos determinística; migrações testadas.
- [Proposta] Operações de edição compostas e reversíveis.
- [Proposta] Arquivos de autoria separados de cache e layout pessoal.
- [Proposta] Autosave, recovery, backups antes de migrações destrutivas.
- [Proposta] File watching com detecção de conflitos antes de sobrescrever mudanças externas.
- [Proposta] Diff estruturado para scenes, event sheets, prefabs e mapas.

---

## 22. Exportação e distribuição

### 22.1 Desktop

- [Decidido] **Windows e Linux são prioridades**.
- [Decidido] Runtime de jogo independente do Qt/CXX-Qt.
- [Proposta] `build/export` gera projeto com assets processados, scripts e executável standalone.
- [Proposta] Perfis Debug/Release e validação de artefatos.
- [Proposta] Targets macOS e demais plataformas conforme demanda posterior.

### 22.2 Web

- [Planejado] No roadmap antigo, `tiny export --web` gera um **ZIP WebAssembly autocontido** executável no navegador.
- [Fora do MVP inicial de runtime 3D] Web export; estava previsto para P04, não para P00.
- [Proposta] Reavaliar compatibilidade de renderer, scripting, áudio e armazenamento antes de prometer suporte Web.

### 22.3 Build pipeline

1. Validar Project Model e Gameplay IR.
2. Validar dependências e referências de assets.
3. Compilar/cachear materiais, meshes e recursos necessários.
4. Selecionar linguagem ativa de script e backend correspondente.
5. Excluir dados exclusivos do editor.
6. Incluir runtime standalone e assets.
7. Empacotar e executar smoke tests.

---

## 23. Segurança e extensibilidade

- [Decidido] Host API cria fronteira entre scripts e engine.
- [Proposta] Capabilities por módulo e verificação de handles.
- [Proposta] Plugins de editor, importação, exportação, behaviors e tipos de asset por contratos estáveis.
- [Proposta] Manifests com permissões e versões de API.
- [Proposta] Não executar código arbitrário ao apenas pré-visualizar projetos recebidos de terceiros.
- [Proposta] Testes de migração, validação de paths e proteção contra references/inputs malformados.
- [Fora do MVP] Registry público ou marketplace de plugins de terceiros.

---

## 24. Qualidade, acessibilidade e manutenção

### 24.1 Acessibilidade

- [Decidido] Minimizar complexidade e oferecer editor compreensível em hardware modesto.
- [Proposta] Uso completo de teclado, foco visível, tooltips descritivos e atalhos remapeáveis.
- [Proposta] Zoom de interface, tipografia configurável, alto contraste, redução de animações.
- [Proposta] Alternativas tabulares/outline para grafos de comportamento e cenas complexas.
- [Proposta] Estados de erro/empty/loading explícitos em painéis.

### 24.2 Engenharia

- [Decidido] Crates pequenas e versionadas, baixo acoplamento, poucos dependencies pesados.
- [Proposta] Testes unitários, integração, replay determinístico para subsistemas aplicáveis, testes headless e testes visuais de viewport.
- [Proposta] API pública documentada e versionada.
- [Proposta] Gates automáticos de build, lint, tests, docs e verificação de arquivos serializados.
- [Proposta] Feature flags para funcionalidades caras/opcionais.

### 24.3 Métricas de produto

- Tempo entre criar projeto e ter uma sala explorável.
- Quantidade de etapas para criar porta funcional, pickup e inimigo Patrol.
- Tempo de build em máquina modesta.
- Memória ocupada pelo editor ocioso e em cena típica.
- Taxa de projetos que abrem sem perda após migração.
- FPS/frame time em cenas de referência.
- Clareza dos diagnósticos, medindo correções com poucos passos.

---

## 25. Roadmap histórico P00–P06 da engine 3D

Os itens abaixo são uma **transcrição conceitual dos objetivos preservados no repositório**, não confirmação de entrega.

| Fase | Objetivo | Requisitos centrais | Estado documental encontrado |
|---|---|---|---|
| **P00** | Spike runtime 3D | Carregar `map.tiny.json`; `tiny check`; `tiny run`; abrir janela; renderizar brush; flycam; chamar Aipo; GUI opcional | `LOCKED` no arquivo histórico |
| **P01** | Brush Editor | Caixa/rampa/cilindro; move/rotate/scale; grid snap; Undo/Redo; IDs persistentes | `PLANNED` |
| **P02** | Triggers & Events | Pintar volumes; OnEnter/OnExit/OnInteract; condição/ação textual; prefabs Porta/Pickup/Spawn | `PLANNED` |
| **P03** | FPS & Environment | FPSController; heightmap Raise/Lower/Smooth; água simples; chuva/neve/fog; billboards | `PLANNED` |
| **P04** | VFX & Web | Emissor fogo/fumaça/faísca/poeira; decals pool/fade; export WASM ZIP | `DRAFT` |
| **P05** | Navigation & AI | Waypoints conectáveis, A*, Patrol e Chase | `DRAFT` |
| **P06** | MCP & Diagnostics | map.get/set, brush.create, trigger.create, event.list, play.test; Issue Browser; quick fixes; keymaps | `DRAFT` |

### 25.1 O que mudou depois desse roadmap

- `wgpu + winit + egui` compunham o **spike histórico**; a arquitetura refinada optou por **Rust + Qt/CXX-Qt + OpenGL**.
- O roteiro original falava só em **Aipo**; depois aprovamos **Luau ou Aipo**, com uma linguagem ativa por projeto.
- O roteiro original focava apenas no editor de mapas; depois aprovamos **RuntimeWorld/ECS, Gameplay IR**, bibliotecas de geometria compartilhadas, GLB e **Live Link**.
- Foram mencionadas fases posteriores de integração (P0–P10), mas seus nomes/aceites detalhados não estão completos nas fontes disponíveis; não é correto inventar marcos oficiais.

---

## 26. Roadmap consolidado recomendado para retomada

**Este é um plano de implementação sugerido com base nas decisões recuperadas, e não uma reprodução literal das fases antigas.**

### R0 — Auditoria e contratos

- [ ] Isolar/identificar os diretórios e branches 3D, sem destruir o trabalho 2D.
- [ ] Registrar ADR de stack final Rust/Qt/OpenGL/Luau/Aipo.
- [ ] Definir e versionar `petunia-libs`.
- [ ] Definir IDs, formato de projetos, política de migração e Host API schema.

### R1 — Vertical slice 3D headless + janela

- [ ] `check` em projeto/mapa textual.
- [ ] `run` cria janela independente de Qt e renderiza uma sala de brushes.
- [ ] Flycam, câmera runtime e transform 3D.
- [ ] Testes headless de cena, matemática e serialização.

### R2 — Editor de level building

- [ ] Qt/CXX-Qt + viewport OpenGL.
- [ ] Hierarquia/Inspector/Assets.
- [ ] Caixa, rampa, cilindro, gizmos e grid snap.
- [ ] Undo/Redo, save/load, seleção e validação.

### R3 — Jogabilidade mínima

- [ ] Collider cinemático e FPSController.
- [ ] Volumes trigger OnEnter/OnExit/OnInteract.
- [ ] Prefabs Porta, Pickup, Spawn.
- [ ] Ação/condição por Gameplay IR e debug básico.

### R4 — Script Host e autoria visual

- [ ] Schema único da Gameplay API.
- [ ] Backend Luau e backend Aipo desacoplados.
- [ ] Seleção da linguagem por projeto.
- [ ] Event Sheets e Behaviors consumindo o mesmo IR.
- [ ] Hot-reload/testes headless quando seguros.

### R5 — Ambientes e navegação

- [ ] Terrain heightmap Raise/Lower/Smooth.
- [ ] Água simples, billboards e clima.
- [ ] Waypoint editor, A*, Patrol e Chase.

### R6 — Produção e integração

- [ ] Particle emitters e decals.
- [ ] Import/reimport GLB.
- [ ] Petunia3D Live Link e metadados opcionais.
- [ ] Recursos mínimos de material, animação e audio.
- [ ] Build desktop Windows/Linux.

### R7 — Automação e maturidade

- [ ] MCP sem necessidade de editor aberto.
- [ ] Issue Browser, quick fixes e Project Doctor.
- [ ] Profiler, Runtime Inspector, determinismo testável e crash recovery.
- [ ] WebAssembly apenas após validar compatibilidade ponta a ponta.

---

## 27. Cenários de aceite de ponta a ponta

### Cenário A — Construir uma dungeon jogável

1. Criar um projeto 3D.
2. Desenhar uma sala com paredes de brush, uma rampa e um corredor.
3. Definir grid e materiais por face.
4. Inserir Spawn Point e FPSController.
5. Adicionar porta com volume trigger.
6. Configurar `OnInteract -> OpenDoor` sem código textual.
7. Pressionar Play e percorrer o mapa.
8. Salvar, fechar e reabrir sem perda de IDs/overrides.

### Cenário B — Criar IA simples

1. Adicionar inimigo com comportamento Patrol.
2. Criar waypoints conectados.
3. Definir raio de percepção.
4. Ativar Chase ao encontrar o jogador.
5. Visualizar no debug qual comportamento está ativo e sua rota A*.

### Cenário C — Escolher scripting

1. Criar projeto e escolher Luau **ou** Aipo.
2. Adicionar uma ação `PlaySound` via Gameplay API.
3. Chamar a mesma ação pelo Event Sheet.
4. Invocar API equivalente no script.
5. Validar referências/tipos sem dependência direta do editor.

### Cenário D — Intercâmbio com Petunia3D

1. Criar geometria de nível com brushes.
2. Exportar ou referenciar asset GLB.
3. Refinar mesh/material no Petunia3D.
4. Reimportar e manter o posicionamento/instância quando suportado.
5. Verificar diagnóstico explícito quando alguma edição não puder ser reconciliada.

### Cenário E — Agentic tooling

1. Agente chama `map.get`.
2. Gera um brush com `brush.create`.
3. Cria trigger com `trigger.create`.
4. Chama `play.test` headless.
5. Project Doctor aponta erro; agente aplica correção rastreável.
6. Arquivos finais continuam legíveis e pequenos no diff Git.

---

## 28. Backlog consolidado por prioridade

### P0 — Base mínima obrigatória

- [ ] Rust core/runtime independente do Qt
- [ ] Editor Qt/CXX-Qt integrado com viewport OpenGL
- [ ] Project/Scene/Entity/Transform e IDs persistentes
- [ ] Asset import e GLB básico
- [ ] Brushes caixa/rampa/cilindro + grid snap
- [ ] Gizmos + Inspector + Hierarquia
- [ ] Undo/Redo transacional
- [ ] `check`, `run` e playtest
- [ ] Flycam e controller FPS cinemático
- [ ] Colliders básicos + volumes trigger
- [ ] Event Sheet mínimo com Gameplay IR
- [ ] ScriptHost + backend de linguagem por projeto

### P1 — Capacidade real de construir jogos

- [ ] Prefabs Porta/Pickup/Spawn e variantes
- [ ] Waypoints + A* + Patrol/Chase
- [ ] Terreno heightmap e água simples
- [ ] Billboards e sprites 3D
- [ ] Clima, particles, decals e iluminação básica
- [ ] UI/HUD mínimo e áudio
- [ ] Import/reimport de assets
- [ ] Runtime Inspector + debugger de regras
- [ ] Export desktop

### P2 — Produção avançada

- [ ] CSG ampliado e ferramentas de brush avançadas
- [ ] Animação 3D, state machines e timeline
- [ ] Dialogue, inventário, quests e save
- [ ] Materiais avançados e perfis de render
- [ ] Live Link Petunia3D
- [ ] MCP, Issue Browser e Project Doctor
- [ ] Profiler completo, testes de replay e hot-reload
- [ ] Plugin API pública
- [ ] Export Web/WASM

As marcações acima são **prioridades recomendadas**, não status de implementação.

---

## 29. O que ainda precisa de decisão explícita

1. Nome final público do produto e extensões dos arquivos (`tinyEngine` era nomenclatura histórica).
2. A forma exata do renderer OpenGL (versão mínima, suporte a perfis e fallback).
3. A abstração de física 3D e se/quando incorporar rigidbodies.
4. ECS interno: implementar somente os recursos realmente necessários, sem adotar frameworks grandes por inércia.
5. Contrato/semântica final de CSG e round-trip entre brushes e GLB.
6. Matriz de formatos de assets e limites de importação.
7. V1 de animação skeletal: importar clips existentes ou oferecer edição autoral básica.
8. Formato físico de arquivos do Gameplay IR e como preservar referências estáveis.
9. Regras de hot-reload e persistência de estado para Luau/Aipo.
10. Limites de suporte a Web/WASM, builds e targets.
11. Modelo de plugins/capabilities e execução segura de projetos externos.
12. A forma final de integração Live Link com Petunia3D.

---

## 30. Princípios não negociáveis para implementação

1. **Não transformar a Petunia em uma cópia monolítica da Godot/Unreal.**
2. **Não confundir editor 3D com software de modelagem 3D completo.**
3. **Não duplicar API de gameplay** entre Event Sheets, grafos, Luau e Aipo.
4. **Não acoplar Petunia3D e Petunia Game Engine em runtime.**
5. **Não fazer o runtime depender de Qt.**
6. **Não assumir que arquivos históricos de planejamento são código entregue.**
7. **Não reescrever recursos do projeto 2D por acidente ao retomar a linha 3D.**
8. **Não mudar backend/stack silenciosamente sem ADR.**
9. **Não começar pela UI cosmética antes de modelo, serialização, comandos e execução.**
10. **Não tratar propostas desta consolidação como decisões já aprovadas.**

---

## 31. Fontes recuperadas e rastreabilidade

### Discussões de projeto preservadas no contexto

- **10/10/2026:** decisão de produto **Petunia Game Engine** separado do Petunia3D; geometria e bibliotecas compartilhadas; RuntimeWorld e engine modular.
- **10/10/2026:** decisões complementares de `petunia-libs`, Luau/Aipo por projeto, Host API gerada por schema, brushes convexos, GLB como intercâmbio e Live Link.

### Objetivos originais do repositório (3D)

- [P00 — Spike runtime 3D](https://github.com/WASDst/petunia-game-engine/blob/feat/cxx-qt-migration/.ai/goals/P00-G01.goal.json)
- [P01 — Brush Editor](https://github.com/WASDst/petunia-game-engine/blob/feat/cxx-qt-migration/.ai/goals/P01-G01.goal.json)
- [P02 — Triggers e eventos Aipo](https://github.com/WASDst/petunia-game-engine/blob/feat/cxx-qt-migration/.ai/goals/P02-G01.goal.json)
- [P03 — FPS, terreno e ambiente](https://github.com/WASDst/petunia-game-engine/blob/feat/cxx-qt-migration/.ai/goals/P03-G01.goal.json)
- [P04 — Partículas, decals e Web](https://github.com/WASDst/petunia-game-engine/blob/feat/cxx-qt-migration/.ai/goals/P04-G01.goal.json)
- [P05 — Navegação e A*](https://github.com/WASDst/petunia-game-engine/blob/feat/cxx-qt-migration/.ai/goals/P05-G01.goal.json)
- [P06 — MCP e diagnósticos](https://github.com/WASDst/petunia-game-engine/blob/feat/cxx-qt-migration/.ai/goals/P06-G01.goal.json)

### Documentos posteriores de outros escopos — **não confundir com esta especificação**

- [ADR de migração Rust/CXX-Qt/Aipo do produto 2D](https://github.com/WASDst/petunia-game-engine/blob/feat/cxx-qt-migration/docs/decisions/0003-rust-cxx-qt-aipo-stack.md). Demonstra decisões de stack de uma fase posterior, mas seu conteúdo é para **tinyEngine 2D**, não comprova que as funcionalidades 3D já existem.
- [Roadmap 2D posterior](https://github.com/WASDst/petunia-game-engine/blob/feat/cxx-qt-migration/docs/development/roadmap.md). Usar exclusivamente para entender divergências históricas, não como roadmap de conclusão da engine 3D.

---

## 32. Resumo executivo final

O coração da **Petunia Game Engine 3D** não era um renderer sofisticado: era um **pipeline integrado de criação de jogos 3D acessível**, que permitisse construir uma sala por brushes, colocar entidades e comportamentos, escrever ou montar regras visuais, testar na hora e exportar um jogo autônomo.

A arquitetura posteriormente amadureceu para **Rust + editor Qt/CXX-Qt + OpenGL + Gameplay IR + RuntimeWorld + ScriptHost (Luau/Aipo)**, com **`petunia-libs`** e **GLB/Live Link** conectando a engine ao Petunia3D sem dependência direta entre aplicativos. Essa é a base mais fiel para retomar o projeto 3D.

**Próxima ação recomendada:** fazer uma auditoria do estado atual do repositório e abrir uma linha de implementação 3D própria, sem confundir nem sobrescrever a evolução da engine 2D.
