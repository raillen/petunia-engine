# Resumo executivo

**Revisão:** 2026-10-24

## Resumo (TL;DR)

A Petunia Engine é uma engine 3D para **criar mundos naturais e conectados** (rios, estradas, florestas, clima) com **pouca fricção** e **acessibilidade de verdade**.

Ela **retoma** a [Petunia Game Engine 3D](../01-pesquisa/05-especificacao-anterior.md) (decisões de 10/10/2026), amplia o escopo e junta duas peças que você já tem:

- **Petunia3D** — fundação de editor: comandos com undo, splines, grafo de operações, plugins Lua, MCP, formatos de arquivo.
- **Aipo** — a linguagem: leitura simples, dados em vez de classes, VM em Rust, schema de host com permissões.

O resto (renderer, física leve, cordas, clima, online) é novo.

## Para quem é

| Pessoa | O que precisa |
|---|---|
| Quem cria mapas e jogos sem ser programador | Ferramentas diretas, bons padrões, "funciona logo de cara" |
| Pessoa neurodivergente (TDAH, dislexia, autismo) | Menos carga cognitiva, previsibilidade, controle sobre estímulos |
| Pessoa com deficiência visual ou motora | Teclado completo, leitor de tela, alvos grandes, alternativas ao mouse |
| Quem programa e quer uma API limpa | Dados + funções, sem hierarquias de classes |

## O que a engine entrega (visão final)

1. **Editor de mapas** com terreno, água, estradas ligadas, vegetação, clima e decals.
2. **Física leve** (corpos simples, personagem, gatilhos) e **cordas** como recurso de primeira classe.
3. **Pós-processamento leve e bonito**, com controles de conforto sensorial.
4. **Aipo como linguagem principal**, **Luau como alternativa** (uma ativa por projeto).
5. **Programação visual 1:1 com a API:** Event Sheet ("quando / se / faça"), grafos especializados e nós, todos sobre a mesma API dos scripts.
6. **Editor de código integrado** com diagnósticos em linguagem simples.
7. **Online integrado:** criar sala, compartilhar código, jogar.
8. **Acessibilidade verificada**: um "Project Doctor" que avisa problemas de acessibilidade do jogo antes de publicar.

## Ideia central: a API é um dado

Hoje cada ferramenta teria sua lista de funções: uma para Aipo, outra para Luau, outra para os nós, outra para a documentação. Isso gera divergência.

Em vez disso, a engine descreve cada função **uma vez**, num schema. O schema gera tudo:

```text
            Schema da API da engine (um só)
                       │
   ┌─────────┬─────────┼─────────┬──────────────┐
   ▼         ▼         ▼         ▼              ▼
 Aipo      Luau      Nós     Autocomplete   Docs + MCP
(bindings) (tipos)  (catálogo) (editor)     (agentes)
```

Os dois projetos já usam esse padrão em pequena escala (`LOCAL`): o `aipo-host` (schema AHS) e o `CommandIntent` + JSON Schema do Petunia3D. A engine o torna o centro do desenho.

## Três riscos que mandam no plano

1. **Escopo.** É muito grande. O plano usa fases com uma demo alvo por fase (veja o [roadmap](../05-roadmap/01-fases-e-experimentos.md)).
2. **Licença.** Petunia3D é GPL-3.0 e Aipo é MIT. Isso afeta o que pode entrar no runtime dos jogos. Veja [licenças](../01-pesquisa/04-licencas.md).
3. **Maturidade da Aipo.** O próprio projeto registra um piso de custo por instrução e estados "DRAFT" sem testes executados (`LOCAL`). Precisa de medição antes de depender dela em tempo real.

## Demo alvo (critério de sucesso final)

> Uma ilha com **um rio**, **uma estrada que cruza o rio por uma ponte de corda**, **floresta**, **chuva que molha o chão**, **decals** de lama e marcas de pneu, e **duas pessoas jogando online**. Tudo montado no editor, sem escrever código, e depois ajustado com um script Aipo de 20 linhas.

## Decisões ligadas

`DR-01` licença · `DR-02` base técnica · `DR-05` modelo de dados · `DR-14` escopo e demo. Veja também o [registro de decisões](../06-decisoes/registro-de-decisoes.md).
