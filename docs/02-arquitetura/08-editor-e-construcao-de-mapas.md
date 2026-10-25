# Editor e construção de mapas

**Revisão:** 2026-10-24 · **Estado:** proposta; toolkit (Qt/CXX-Qt) decidido antes, validação em S4

## Resumo (TL;DR)

O editor tem **poucos modos**, sempre na **mesma geografia de tela**. Cada ferramenta tem 4 caminhos: **predefinição rápida**, **campos numéricos**, **atalho de teclado** e **desfazer**. O primeiro minuto gera um mapa bonito.

Base: o Shell v2 do Petunia3D, que já define regiões fixas e acessibilidade como critério de aceite (`LOCAL`, `workspaces-feedback-accessibility.md`).

## Geografia de tela (reaproveitada)

```text
┌──────────────────────────────────────────────────────────┐
│ Cabeçalho: projeto · desfazer/refazer · modos · comandos │
├──────┬─────────────────────────────┬─────────────────────┤
│      │                             │ ESTRUTURA           │
│ Tri- │                             │ "o que existe aqui?"│
│ lho  │      SUPERFÍCIE DE          ├─────────────────────┤
│ de   │      TRABALHO (3D)          │ PROPRIEDADES        │
│ fer- │                             │ "como é o atual?"   │
│ ra-  │  Barra de ação contextual   │                     │
│ men- ├─────────────────────────────┴─────────────────────┤
│ tas  │ GAVETA: biblioteca · Doctor · console · código    │
└──────┴───────────────────────────────────────────────────┘
```

As posições são **sempre as mesmas**. Muda o conteúdo conforme o modo (`LOCAL`: "gramática espacial estável, conteúdo contextual").

## Modos (máximo 7)

| Modo | O que o criador faz |
|---|---|
| **Mundo** | Terreno, água e rios, estradas, vegetação |
| **Construir** | **Level Builder:** brushes convexos (caixa, rampa, cilindro), grade com snap, depois assistente de salas, portas, janelas, escadas e peças modulares |
| **Objetos** | Props, prefabs, entidades com comportamento |
| **Atmosfera** | Hora do dia, clima, pós-processamento |
| **Lógica** | **Event Sheet** ("quando / se / faça"), grafos de estado e vista de nós |
| **Código** | Editor de texto (Aipo e Luau) |
| **Jogar** | Testar no editor; "Testar online" |

O **Project Doctor** fica na gaveta, sempre acessível.

## Princípios de construção

1. **Bom por padrão.** "Novo projeto" oferece **modelos iniciais** (Ilha, Vila, Floresta, Corredor, Plataforma) que já saem com luz, céu e vegetação coerentes. Referência: o GameGuru MAX gera algo jogável logo de início (`WEB`).
2. **Auto-popular.** Um clique preenche vegetação ou prédios; ferramentas finas refinam (`WEB`, GameGuru MAX).
3. **Edição ao vivo.** Mudar uma regra ou script com o jogo rodando. A Aipo descreve "reload transacional" (`LOCAL`, em rascunho).
4. **Undo ilimitado e sem medo.** Desfazer em tudo. O TrenchBroom é lembrado justamente por isso (`WEB`).
5. **Colocar sem mouse.** Todo posicionamento tem alternativa por teclado e por valor numérico.
6. **Ajuda ao colocar.** Encaixar no chão, alinhar à normal, grade configurável, rotação/escala aleatórias.
7. **Dois níveis de detalhe.** Simples por padrão; "Mostrar mais" revela parâmetros avançados. Isso reduz a carga cognitiva.
8. **Bloqueio de mapa ("blockout").** Primeiro formas simples para testar a jogabilidade, depois o visual (`WEB`, Level Design Book).

## Level Builder (base) e construção de prédios

**Base já decidida** (especificação anterior, `LOCAL`): níveis nascem de **brushes convexos** (caixa, rampa, cilindro) com **grade e snap**, **gizmos**, material por face e **Undo/Redo transacional**. CSG (união, subtração) entra **depois**, por etapas.

Por cima, o **assistente de salas** (referência GameGuru MAX, `WEB`): desenhar a planta, definir o pé-direito, e o editor gera paredes, piso e teto; **portas, janelas e escadas** se encaixam em módulos. Kits modulares usam **grade grossa baseada no módulo** (por exemplo 5 m para paredes de 5 m, `WEB`, Level Design Book). O assistente **gera brushes e prefabs**; não é um segundo sistema.

Prefabs iniciais herdados: **Porta**, **Pickup** e **Spawn Point**, já integrados com colisão, navegação e gatilho.

## Entidades com comportamento (referência FPS Creator)

Uma **entidade** é: modelo + tabelas de dados + **regras**. A biblioteca traz entidades prontas (porta, chave, inimigo que patrulha, item coletável). O criador **troca a regra**, não o código.

Em vez do FPI do FPS Creator (`:cond:ação`), a regra é legível:

```aipo
# Ilustrativo
rules.on("touch", tag = "Door", do_this = fn(event) {
    if inventory.has(event.who, item = "chave_azul") {
        world.set(event.target, open = true)
    }
})
```

Veja [programação visual](../03-linguagens-e-api/04-programacao-em-nos.md): a mesma regra aparece no **Event Sheet**, com as mesmas chamadas da API.

## Project Doctor (validador)

Inspirado no *Game-Ready Validator* do Petunia3D: severidade, **código estável**, alvo, mensagem simples e **ação recomendada**; "errors bloqueiam, warnings avisam, info informa" (`LOCAL`, `game_ready.rs`). O TrenchBroom oferece algo parecido, com "correção rápida" (`WEB`).

| Grupo | Exemplos de verificação |
|---|---|
| **Mundo** | Rio sobe; estrada com ponta solta; ilha de estradas desconectada; sem ponto de início do jogador |
| **Física** | Objeto sem colisão; corda sem âncora |
| **Desempenho** | Triângulos, instâncias ou decals acima do orçamento |
| **Navegação** | Área alcançável pelo jogador? |
| **Acessibilidade** | Veja [jogos acessíveis](../04-acessibilidade/02-jogos-criados.md) |
| **Script** | Diagnósticos do `aipo check` e do Luau |

Cada item tem botão **"Corrigir"** quando existir correção segura, ou **"Mostrar onde"** que move a câmera.

## Editor de código integrado

| Recurso | Fonte | Observação |
|---|---|---|
| Destaque de sintaxe | Tokens do `aipo-lexer` (Aipo); gramática para Luau | |
| **Diagnósticos ao digitar** | `aipo check` em processo (188 µs para 858 B, `LOCAL`, baseline) | Rápido o bastante para tempo real |
| **Formatar ao salvar** | `aipo fmt` | Uma forma só de escrever |
| Autocomplete, ajuda, assinatura | **Schema da API** | Sem lista manual paralela |
| Ir para definição, renomear | `aipo-lsp` (**ainda não existe**, `LOCAL`) | A construir |
| Luau | `luau-lsp` (processo externo) com definições geradas pelo schema | `WEB`: aceita arquivos de definição e `analyze` |
| Mensagens | Português, com a correção sugerida | Regra 4 da Aipo |
| Depurador | **Não existe** hoje | Lacuna conhecida |

Opções de conforto do editor de código estão em [editor acessível](../04-acessibilidade/01-editor.md).

## Toolkit de interface (`DR-04`): Qt 6 / CXX-Qt

**Decidido antes** (`LOCAL`): editor em **Qt 6 com CXX-Qt**, desacoplado do core; o runtime do jogo **nunca** depende de Qt. Esta documentação **mantém**.

| Ponto | Situação |
|---|---|
| Base | Shell v2 do Petunia3D, com testes de teclado/foco e gate de acessibilidade (`QAccessible`) (`LOCAL`) |
| **Risco 1: build pesado** | A máquina já engasgou com Qt/C++ (`CARGO_BUILD_JOBS=2`, swap cheia, `LOCAL`) |
| **Risco 2: licença** | Qt é LGPL/GPL/comercial (`GERAL`); veja [licenças](../01-pesquisa/04-licencas.md) |
| **Risco 3: viewport embutido** | OpenGL compartilhando contexto com o Qt: o Petunia3D já faz (FBO importado sem *readback*, `LOCAL`) |
| **Risco 4: editores ricos** | Editor de código e de nós de boa qualidade precisam ser construídos em Qt |

O experimento **S4** não reabre a escolha. Ele **mede**: tempo de build local, memória, leitor de tela no inspetor e a viabilidade do editor de código. Só se S4 falhar, entram os planos B:

- **B1:** editor em tecnologia web (WebView + CodeMirror/Monaco): ótima acessibilidade e editores prontos; difícil embutir o viewport 3D (`HIPÓTESE`).
- **B2:** Rust puro (egui ou Slint + AccessKit): build leve; editores ricos a construir; suporte de acessibilidade do egui hoje em Windows e macOS, **não** confirmado no Linux (`WEB`).

## Decisões ligadas

`DR-04` UI do editor · `DR-07` Gameplay IR e telas visuais · `DR-14` escopo.
