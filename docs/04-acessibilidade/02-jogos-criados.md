# Jogos acessíveis

**Revisão:** 2026-10-24 · **Estado:** proposta

## Resumo (TL;DR)

Todo jogo feito na Petunia Engine **já nasce com um menu de acessibilidade pronto**. O criador não precisa construir nada. Quando quiser, **expõe variáveis** do próprio jogo (dificuldade, velocidade) e elas aparecem no menu.

O **Project Doctor** verifica o jogo e **avisa** o que falta, com o botão "Corrigir" quando possível.

## Kit de acessibilidade incluído (ligado por padrão)

Itens retirados dos Game Accessibility Guidelines (`WEB`: https://gameaccessibilityguidelines.com/full-list/). A coluna "Nível" é a do próprio guia.

### Controles (motor)
| Recurso | Nível | Como a engine entrega |
|---|---|---|
| **Remapear controles** | Básico | Entrada **por ações nomeadas**, remapeável em runtime; perfis salvos |
| Alternar em vez de segurar | Intermediário | "Segurar → alternar" por ação, no menu |
| Uma mão / esquema simplificado | — | Perfis prontos; ações combináveis |
| Pausar a qualquer momento | Avançado | Pausa global que o criador não precisa implementar |
| Sem eventos que dependam de reflexo | — | Aviso do Project Doctor para tempo curto obrigatório |

### Cognitivo e sensorial
| Recurso | Nível | Como a engine entrega |
|---|---|---|
| **Sem imagens piscantes** | Básico | **Limitador de flash** no pós-processamento |
| **Esconder movimento de fundo** | Intermediário | Opção "Reduzir movimento": desliga tremor, balanço, parallax pesado |
| Ler texto no próprio ritmo | Básico | Diálogos **avançam por botão**, sem tempo |
| Separar volumes (efeitos, música, voz, ambiente) | Intermediário | **Mixer com 4 barras por padrão** |
| **Compressão de faixa dinâmica** | Dica de guia de autismo (`WEB`) | Opção "Volume previsível" |
| Opção de **desligar sangue e gore** | Avançado | Controle global em decals e partículas |
| Lembrar controles e objetivos | Intermediário | Painel "Objetivo atual" e "Controles" a um botão |
| **Prática sem falha** | Intermediário | Modelo de tutorial com modo sem derrota |
| Miniatura nos saves | Intermediário | Gerada automaticamente |
| Evitar movimento inesperado | Avançado | Aviso do Project Doctor para câmera que sacode sem permissão |
| **Pontos de saída** ("exit ramps") | Dica de guia de autismo (`WEB`) | Modelos de fase com **pontos de parada** claros |

### Visão
| Recurso | Como a engine entrega |
|---|---|
| **Escala de interface** | Padrão ajustável |
| **Alto contraste** da UI do jogo | Tema pronto |
| Cor nunca é o único sinal | Project Doctor verifica; **simulação de daltonismo** como ferramenta do criador |
| **Leitor de tela da UI do jogo** | Árvore de acessibilidade via AccessKit (o Godot 4.5 adotou o mesmo caminho para os controles de jogo, `WEB`) |
| Tamanho de fonte padrão legível | Padrão da engine |

### Audição
| Recurso | Como a engine entrega |
|---|---|
| **Legendas com nome de quem fala** | Sistema de legendas |
| **Legendas de som** ("[trovão]", "[passos à esquerda]") | Cada som pode ter **texto de legenda** |
| Informação nunca só por som | Project Doctor avisa |

### Geral
| Recurso | Nível | Como a engine entrega |
|---|---|---|
| Salvar todas as configurações | Básico | Perfil de acessibilidade persistente |
| Variedade de dificuldades | Básico | `access.expose` + presets |
| **Trocar dificuldade durante o jogo** | Intermediário | Menu de **Assistência** em tempo real |
| Expor variáveis de jogabilidade | Intermediário | `access.expose(...)` vira slider |
| Autosave e salvar manual | Intermediário | Padrão da engine |
| Modos de assistência (mira assistida etc.) | Intermediário | Presets da física: tempo de coiote, ajuda em beirada |
| Pular partes não essenciais | Intermediário | `access.allow_skip` em cutscenes |

## Perfis sensoriais (diferencial)

Um **perfil** é um conjunto de ajustes salvos. A engine traz alguns prontos:

| Perfil | Efeito |
|---|---|
| **Padrão** | Tudo como o criador desenhou |
| **Calmo** | Menos partículas, sem tremor, flash limitado, saturação e bloom menores, volume previsível |
| **Foco** | Esconde elementos não essenciais da UI; objetivo sempre visível |
| **Personalizado** | O jogador ajusta e salva |

O jogador escolhe no **primeiro início** (uma tela curta, pulável) e pode mudar em qualquer momento.

## Project Doctor: verificações de acessibilidade

Mesmo formato dos outros diagnósticos: severidade, **código estável**, mensagem simples, ação (`LOCAL`: `game_ready.rs`).

| Código (exemplo) | Severidade | O que verifica |
|---|---|---|
| `access.input.raw_key` | Erro | Script lê tecla fixa em vez de ação nomeada |
| `access.flash.unlimited` | Erro | Efeito de flash sem passar pelo limitador |
| `access.text.contrast` | Aviso | Contraste do texto abaixo do mínimo |
| `access.info.color_only` | Aviso | Informação transmitida só por cor |
| `access.info.sound_only` | Aviso | Som importante sem legenda |
| `access.timer.no_pause` | Aviso | Contagem regressiva que não pausa |
| `access.camera.shake_unmanaged` | Aviso | Tremor de câmera que ignora "Reduzir movimento" |
| `access.difficulty.none` | Info | Nenhuma variável exposta |
| `access.save.no_thumbnail` | Info | Save sem miniatura |

O Project Doctor **avisa**; não impede. O criador decide. Apenas `access.flash.unlimited` bloqueia a **exportação padrão** (pode ser sobreposto com confirmação).

## Neurodivergência: o que a pesquisa recomenda

Resumo de revisões e guias (`WEB`: MDPI 2025, Springer 2025, Access-Ability):

- Controles de **intensidade sensorial**: reduzir partículas, ajustar paleta, limitar brilho e som.
- **Ritmo ajustável** e pausa a qualquer momento (inclusive em cenas).
- **Pontos de saída** para quem tem dificuldade de parar.
- **Instruções claras e previsíveis**.
- Evitar gatilhos de fotossensibilidade; **volume previsível**.
- **Incluir pessoas** com TDAH e autismo no desenho e nos testes.

## Limites e honestidade

- Isto é uma **lista de recursos**, não uma certificação. A conformidade real exige **testes com pessoas**.
- O Project Doctor detecta o que é **detectável**. Ele não avalia, por exemplo, se um quebra-cabeça é cognitivamente justo.
- A estimativa de contraste usa as cores declaradas; não vê o que o pós-processamento faz com elas.

## Decisões ligadas

`DR-13` pós-processamento · `DR-15` gates e Project Doctor.
