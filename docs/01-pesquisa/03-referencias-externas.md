# Referências externas

**Revisão:** 2026-10-24 · **Fonte:** pesquisa na internet nesta sessão (`WEB`), salvo quando marcado

## Resumo (TL;DR)

Para cada engine ou ferramenta de referência: **o que levar** e **o que evitar**. As três lições mais fortes:

1. **Godot removeu o VisualScript** por ser um espelho do texto, sem blocos de alto nível. Nós 1:1 só valem se a API for de alto nível.
2. **FPS Creator** provou que "condição → ação" resolve a lógica da maioria dos jogos simples, mas a sintaxe dele era ilegível.
3. **Godot 4.5** mostrou o caminho de acessibilidade: **AccessKit** para leitor de tela. O editor ainda é parcial lá.

## Tabela rápida

| Referência | Levar | Evitar |
|---|---|---|
| **Godot** | Cenas simples de entender; API de rede de alto nível; AccessKit | Herança de nós; VisualScript 1:1 sem abstração |
| **Unity** | Ideia de dados contíguos (DOTS); "baking" de autoria para runtime | Dois mundos (GameObject + ECS) misturados |
| **GameGuru MAX** | Tudo-em-um, padrões bonitos de saída, "auto-popular" vegetação, editor de construções | Fechado; dependência de biblioteca de assets |
| **FPS Creator** | Entidade = modelo + scripts; regra "condição → ação" | Sintaxe de pontuação crua |
| **Bevy** | Peças soltas (wgpu, ECS) | Quebra de API a cada versão; sem editor oficial |
| **Fyrox** | Editor Rust real (hierarquia, inspetor, viewport) | Grafo de cena orientado a objetos |
| **TrenchBroom** | Undo ilimitado; "issue browser" com correção rápida | Formato preso a um motor |
| **Roblox/Luau** | Linguagem tipada gradual, sandbox, adotada por iniciantes | — (pouco pesquisado) |

## Godot

- **Acessibilidade:** o Godot 4.5 (set/2025) integrou o **AccessKit** para leitor de tela, com mais de 32 mil linhas no PR. O suporte do **editor é parcial** (gerenciador de projetos, controles padrão, inspetor). O AccessKit expõe também configurações do sistema como alto contraste e animação reduzida. `WEB`
- **VisualScript removido (4.0):** nunca ganhou tração e "o caminho para melhorar não era claro". Usuários que pediam nós acabaram preferindo o GDScript. Comentários da comunidade: era "o GDScript com macarrão", sem componentes de alto nível como os Blueprints da Unreal. Um número citado (0,5% de uso como ferramenta principal) **não** foi confirmado na fonte primária. `WEB`
- **Cena × ECS:** o Godot explica que nós misturam dados e lógica, e que a composição acontece em nível mais alto que um componente de ECS; isso facilita entender "o que a cena faz" só olhando a árvore. `WEB`
- **Rede:** a API de alto nível usa cliente-servidor com autoridade do servidor. `MultiplayerSpawner` replica cenas criadas em jogo, `MultiplayerSynchronizer` replica propriedades, `@rpc` cobre eventos. O Godot **não tem** rollback nem predição embutidos; o complemento `netfox` cobre isso. `WEB`

## Unity (dados)

- DOTS organiza dados por tipo, lado a lado (todas as posições juntas). Mexe no desenho de memória, não só na sintaxe. `WEB`
- Relato da equipe de V Rising: usar ECS puro e **empurrar dados** para os GameObjects, nunca o contrário. `WEB`
- **Lição:** um só modelo de dados. Autoria (editor) e runtime devem ser a mesma tabela, com colunas extras só do editor.

## GameGuru MAX

Confirmado na página de recursos (`WEB`): terreno procedural com biomas e controles, esculpir e pintar; fases de até 5 km²; clima e filtros de pós-processamento ajustáveis (sol, nuvens, água, sombras, névoa, chuva, neve); **vegetação auto-populada** com um clique; mais de 1200 assets; **editor de construções** com salas, portas, janelas e escadas; modo de edição ao vivo; Lua para quem quer mais; sistema de lógica visual.

**Levar:** o "primeiro minuto" precisa gerar algo bonito; construção de prédios por salas; edição ao vivo.

## FPS Creator

- Linguagem FPI: cada linha é `:condições:ações`. Exemplo real: `:VarEqual=Active 0,PlrDistWithin=40:PlayerTake,ColOff,SetVar=Active 1`. `WEB`
- Cada entidade tem scripts de **Início**, **Principal** e **Destruição**. `WEB`
- Virou código aberto como "FPS Creator Classic" em 2016. `WEB`

**Levar:** o modelo mental "quando isto acontecer, se aquilo, então faça". **Evitar:** pontuação como `:`, `=` e vírgulas sem palavras. Em Aipo o mesmo vira regra legível. Veja [esboço da API](../03-linguagens-e-api/03-esboco-da-api.md).

## Rust: Bevy, Fyrox, wgpu

(Fontes secundárias, blogs. Confirmar nos repositórios oficiais antes de decidir.) `WEB`

- **Bevy:** ~48 mil estrelas; Bevy 0.20 em out/2026; ainda pré-1.0 com quebras por versão; sem editor visual oficial. Usa wgpu e adotou WESL (extensão de WGSL).
- **Fyrox:** chegou à 1.0 em 2026; grafo de cena tradicional; tem editor real. Comunidade menor.
- **wgpu** é biblioteca gráfica, não engine; é o backend do Bevy.

## Acessibilidade em jogos

- **Game Accessibility Guidelines** (lista completa, `WEB`): categorias Motor, Cognitiva, Visão, Audição, Fala, Geral. Itens que moldam a engine: sem imagens piscantes, controles remapeáveis, dificuldade ajustável durante o jogo, expor variáveis de jogabilidade, salvar configurações, pausar, autosave com miniatura, opção de esconder movimento de fundo, desligar sangue e gore, chat por símbolos.
- **Neurodivergência:** revisões de literatura (MDPI 2025, Springer 2025) e guias práticos recomendam controles de intensidade sensorial, ritmo ajustável, instruções claras, pontos de saída ("exit ramps"), sliders de áudio e redução de gatilhos. `WEB`
- **AccessKit:** infraestrutura em Rust para leitor de tela; já integrada em Bevy, egui, Freya, Slint e Xilem; tem API em C. `WEB`

## Fontes (links)

- Godot 4.5: https://godotengine.org/releases/4.5/ · https://godotengine.org/article/dev-snapshot-godot-4-5-dev-3/
- VisualScript: https://godotengine.org/article/godot-4-will-discontinue-visual-scripting/
- Godot sem ECS: https://godotengine.org/article/why-isnt-godot-ecs-based-game-engine/
- Rede no Godot: https://docs.godotengine.org/en/stable/tutorials/networking/high_level_multiplayer.html · https://github.com/foxssake/netfox
- Unity DOTS: https://unity.com/blog/engine-platform/dots-webinar-roundtable-q-a
- GameGuru MAX: https://game-guru.com/features
- FPI (FPS Creator): https://fpsc.fandom.com/wiki/Category:FPI_Scripting · https://forumfiles.thegamecreators.com/thread/202859
- Game Accessibility Guidelines: https://gameaccessibilityguidelines.com/full-list/
- Neurodivergência em jogos: https://www.mdpi.com/2673-7272/6/1/18 · https://link.springer.com/article/10.1007/s10209-025-01231-5 · https://access-ability.uk/2025/03/07/autistic-player-accessibility-in-video-games/
- AccessKit: https://accesskit.dev/
- Level design (blockout, TrenchBroom): https://book.leveldesignbook.com/process/blockout
