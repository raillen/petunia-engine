# Princípios

**Revisão:** 2026-10-24

## Resumo (TL;DR)

São 8 princípios. Quando dois colidem, vale a ordem da lista de desempate no fim.

Eles vêm de duas fontes: as 4 regras da Aipo (`LOCAL`, `aipo-lang/SYNTAX.md`) e a política de desempate do Petunia3D (`LOCAL`, `petunia3D/AGENTS.md` §1).

## Os 8 princípios

### 1. Acessibilidade é padrão, não recurso extra
Vale para o editor **e** para os jogos criados. Ligado por padrão, verificado por testes.

### 2. Uma forma só para cada coisa
Herdado da Aipo. Se há duas maneiras de fazer algo, uma delas está errada. Isso vale para API, ferramentas e atalhos.

### 3. Palavra completa vence símbolo e abreviação
`position`, não `pos`. `and`, não `&&`. Quem tem dislexia ou TDAH lê menos esforço em palavras inteiras.

### 4. Dados, não classes
Mundo = tabelas de dados. Comportamento = funções e regras. Sem herança, sem objetos com ciclo de vida escondido.

### 5. O erro diz o que fazer
Toda mensagem aponta a posição, nomeia o problema e sugere a correção. Em português.

### 6. Nunca perder trabalho
Undo em tudo. Autosave. Nenhum `panic` em produção. Arquivo ruim é recusado com mensagem, nunca derruba o programa.

### 7. Leve por padrão
Funciona em hardware modesto. Qualidade sobe por perfis; nada de custo escondido por frame.

### 8. Previsível
Os sistemas rodam sempre na mesma ordem. Mudanças estruturais acontecem em pontos seguros. Nenhuma "ação a distância" surpresa.

## O que isso muda na prática

| Princípio | Exemplo concreto |
|---|---|
| 2 e 3 | `input.pressed("move_right")` em vez de `host_key_down(262)` |
| 4 | Porta = linha numa tabela `Door`, não classe `DoorController` |
| 5 | "Rio sobe o terreno em 3 pontos. Quer baixar o leito? [Sim] [Ver pontos]" |
| 6 | Esculpir terreno = 1 passo de undo por pincelada |
| 7 | Física tem orçamento (`physics.quality = "light"`) |

## Desempate (ordem estrita)

1. Zero perda de trabalho.
2. Acessibilidade e UX neurodivergente-friendly.
3. Hardware modesto.
4. Coerência: uma fonte de verdade por responsabilidade.
5. Familiaridade com Godot, Unity, GameGuru MAX etc. (sem copiar código).

## Decisões ligadas

`DR-15` gates de qualidade e acessibilidade.
