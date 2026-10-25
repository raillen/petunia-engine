# Riscos e fora de escopo

**Revisão:** 2026-10-24

## Resumo (TL;DR)

Os **cinco maiores riscos**: escopo, capacidade de uma pessoa só, maturidade da Aipo, custo de build do editor e licenciamento. O que **não** entra na versão 1 está listado abaixo para proteger o escopo.

## Riscos

| # | Risco | Prob. | Impacto | Mitigação |
|---|---|---|---|---|
| R1 | **Escopo enorme** (engine 3D + editor + 2 linguagens + online) | Alta | Alto | Fases com demo; spikes antes de construir; lista de fora de escopo |
| R2 | **Capacidade de uma pessoa só** | Alta | Alto | Reusar o máximo; corte agressivo; priorizar a demo alvo |
| R3 | **Aipo imatura** para tempo real (piso de ~100 ns/instrução; rodadas em rascunho; testes não executados) | Média | Alto | S1 antes de tudo; API em lote; saída `aipo-wasm`; **plano B:** Luau como linguagem principal temporária |
| R4 | **Build do editor pesado** (Qt/C++ travou a máquina antes) | Média | Médio | S4 mede o build local; considerar alternativas (`DR-04`) |
| R5 | **Licença GPL** bloqueia uso em jogos | Média | Alto | `DR-01` antes de copiar código; reescrever módulos críticos do runtime se preciso |
| R6 | **OpenGL 3.3 pode não cobrir algum efeito** (sem compute; sem culling na GPU) | Média | Médio | Culling por chunk na CPU; S3 na máquina real; plano B `wgpu` só com ADR |
| R7 | **Duas linguagens (backends) aumentam manutenção** | Média | Médio | **Uma ativa por projeto** (herdado); um schema só; Luau pode ser cortado (`DR-06`) |
| R8 | **Acessibilidade declarada sem teste humano** | Média | Alto | Só dizer "passa nos gates"; testes com pessoas na fase 6 |
| R9 | **Dependências que quebram API** (`mlua`, `wgpu`, `rmcp` e outras mudam com frequência) | Média | Médio | Fixar versões; `cargo deny`; isolar atrás de interfaces |
| R10 | **Luau com JIT nativo ainda experimental no `mlua`** (`WEB`) | Baixa | Baixo | Usar só o interpretador na v1 |
| R11 | **Determinismo entre plataformas** é meta, não garantia | Média | Médio | Não prometer rollback na v1 |
| R12 | **Custo de relay/sinalização online** | Média | Médio | Autohospedado por padrão; sem conta |
| R13 | **Nós em programas grandes viram labirinto** | Média | Médio | Quadros recolhíveis, busca, visão estrutural; texto sempre disponível |
| R14 | **Fadiga com a própria carga cognitiva do projeto** | Média | Alto | Documentação com resumo; um próximo passo por vez; fases curtas |
| R15 | **Escopo cresceu além da especificação anterior** (rios, estradas, cordas, online, clima rico) | Alta | Alto | Base (P0) primeiro; mundo natural depois; `DR-14` consciente |
| R16 | **Confundir a linha 3D com a engine 2D (`tinyEngine`)** do mesmo repositório | Média | Médio | Auditar o repositório (fase 0); não tratar estados de conclusão como prova |
| R17 | **Gameplay IR vira "mais uma linguagem"** | Média | Médio | Poucos conceitos; tudo é chamada da API; S5 antes de ampliar |
| R18 | **Direitos dos ativos do Digimon** (modelos extraídos de jogo comercial; shader recriado por fãs) | Alta | Alto | Fixtures locais fora do Git; criaturas originais no público; shader toon original (`DR-16`) |
| R19 | **O remake vira refazer o jogo inteiro** | Média | Alto | Fatia mínima fixa; critério de aceite por passo; teste diferencial |
| R20 | **UI de jogo subestimada** | Alta | Médio | S9 antes da fase 4; começar pelo mínimo do Digi-Tactics |
| R21 | **O alvo do jogo original é web; a engine mira desktop** | Média | Médio | Subconjunto WebGL2/GLES3 e runtime sem Qt; exportação web depois |

## Fora de escopo da versão 1

| Item | Por quê |
|---|---|
| **Consoles** | Custo e licenças de plataforma |
| **CSG avançado** (além de etapas simples) | Decidido por etapas na especificação anterior |
| **Navmesh automática** | A* em waypoints primeiro (`LOCAL`) |
| **Diálogo, cutscenes, quests** | Subsistemas opcionais posteriores (`LOCAL`) |
| **VR/AR** | Escopo e enjoo; foge do foco |
| **Ray tracing / iluminação global avançada** | Hardware modesto é prioridade |
| **Animação esquelética de autoria** | Fica no Petunia3D (`rig.rs`, `animation.rs`, `ik.rs` já existem lá, `LOCAL`). **Tocar** animações glTF **entra** na v1 (veja [personagens](../02-arquitetura/09-personagens-ui-e-grade.md)) |
| **Veículos, ragdoll, destruição** | Física leve na v1 |
| **FBX** | glTF é o formato principal (ADR-C04, `LOCAL`) |
| **Loja, nuvem, contas** | O Petunia3D adiou "marketplace e cloud/login" (`LOCAL`) |
| **Plugins nativos (DLL)** | Sem modelo de segurança e ABI sustentáveis; só scripts com permissões |
| **Mundo infinito com streaming total** | Mundos em blocos finitos primeiro |
| **MMO** | Online de sala, não de milhares |
| **Chat de voz** | Moderação e privacidade; chat por símbolos primeiro |
| **Sequenciador de cinemáticas** | Depois |
| **Editor de materiais por grafo completo** | Presets primeiro; grafo só no avançado |
| **Rollback netcode** | Só se houver demanda (nível 3) |
| **Exportar para web** | Possível depois (`aipo-wasm`, `aipo-js`) |
| **Ida e volta completa texto ⇄ nós** (S5b) | Estudo opcional depois da v1 |

## Condição para reabrir um item

Mesma regra do registro de decisões do Petunia3D (`LOCAL`): **existe algoritmo, há valor ao usuário, baixo custo, e há teste.**

## Plano B para a linguagem

Se o experimento S1 mostrar que a Aipo **não aguenta** o tempo real:

1. **B1:** projetos usam **Luau** como linguagem ativa; a Aipo segue como linguagem de **ferramentas, editor e Gameplay IR gerado** (menos pesada). Cabe na decisão herdada "Luau ou Aipo por projeto".
2. **B2:** compilar Aipo para **WebAssembly com JIT** (`aipo-wasm`) e medir de novo.
3. **B3:** investir nas otimizações do ADP-011 (compactar `Value`, `InvokeMethod` fundido, laço de despacho).

Isso **não** muda o desenho da API, porque ela vem do schema, não da linguagem.

## Decisões ligadas

`DR-01`, `DR-03`, `DR-04`, `DR-06`, `DR-14`.
