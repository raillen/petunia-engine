# Filosofia de engenharia: o que adotamos do Digi-Tactics

**Revisão:** 2026-10-24 · **Fonte:** `~/Documentos/digimon-stranger-story-animated-models/AGENTS.md` (`LOCAL`), cruzado com `petunia3D/AGENTS.md` e com o código copiável

## Resumo (TL;DR)

O `AGENTS.md` do Digi-Tactics tem **22 regras úteis**. Adotamos **todas**, traduzidas de TypeScript para Rust. Elas já estão no novo [`AGENTS.md`](../../AGENTS.md) da engine.

Achei **seis pontos de atrito** entre essa filosofia e o que planejamos copiar. O mais sério: o `Command` do Petunia3D é uma *trait* com **245 implementações**, e o `ModuleRegistry` usa `dyn`. Ambos contrariam "sem hierarquias de classes e sem frameworks de plugin".

## Como ler a tabela

**Adotar** = mesma regra. **Traduzir** = mesma ideia, ferramenta de Rust. **Nova** = ideia do Digi-Tactics que nossa documentação ainda não tinha.

## Regra por regra

| # | Regra do Digi-Tactics | Como fica na Petunia Engine | Tipo |
|---|---|---|---|
| 1 | Preservar o trabalho; não assumir Git | Autosave, recuperação (copiar o `recovery_dialog`), backup antes de migrar, nunca apagar em massa | Adotar |
| 2 | Reusar → refatorar → mover → reescrever por último | Igual ao Petunia3D (`Princípio de Conservação`) | Adotar |
| 3 | Distinguir verificado, provisório, aproximação, planejado | Campo **`status`** em cada entrada do schema da API | **Nova** |
| 4 | Entrega verdadeira: `pass` / `fail` / `not run` / `blocked` | Igual | Adotar |
| 5 | Dados, acessibilidade, estabilidade e correção acima de polimento e benchmark | Funde com nosso desempate | Adotar |
| 6 | Nomes completos em inglês (`elapsedSeconds`) | `snake_case` completo; abreviações proibidas; `xtask lint-names` | Traduzir |
| 7 | Formatos externos e identificadores de libs só em adaptadores | Tipos de lib **nunca** na API, no schema, nos saves ou no protocolo de rede | Adotar |
| 8 | Dados separados de comportamento; funções livres; registros de operação | Structs + funções livres; tabelas de funções | Adotar |
| 9 | Sem hierarquias, sem localizador de serviços, sem framework genérico de plugins | Sem trait de domínio; sem `dyn Module`; scripts com permissões **não** são framework de plugin | Adotar |
| 10 | Helpers nomeados quando responsabilidades divergem; documentar contratos | Igual | Adotar |
| 11 | `domain` e `game/core` sem dependência de renderização, DOM ou debug | Crates de domínio sem render, janela, Qt, física, áudio, rede; `xtask check-architecture` | Traduzir |
| 12 | UI chama **comandos validados**; leitura por **consultas**; sem execução arbitrária | Schema único de comandos e consultas | Adotar |
| 13 | Comandos documentam entradas, saídas, falhas e reversibilidade; **helpers e quadros não são comandos** | Refina nosso "tudo é comando" (veja atrito 2) | Adotar |
| 14 | Sem imports circulares; fronteiras estreitas; dependência só por necessidade | Grafo de crates sem ciclos; **registro de dependências** e checklist de admissão | Traduzir |
| 15 | Especialização por composição: padrão → perfil → sobrescrita | Prefabs, materiais, personagens, qualidade, clima, perfis de acessibilidade | Adotar |
| 16 | Validar JSON, comandos, caminhos e cenários; *casts* não validam | `serde` + `validate()` com limites; `as` e `unsafe` não são validação | Traduzir |
| 17 | Possuir e liberar recursos de GPU explicitamente; nunca liberar o compartilhado pela instância | Renderer dono; instâncias com handles; caches compartilhados contados | Traduzir |
| 18 | Limites do host: ≤2 workers de teste, 1 visual; perfil de automação dedicado; loopback; nunca desligar segurança | `nextest -j2`, contexto GL dedicado, serviços só em loopback | Traduzir |
| 19 | Contexto progressivo e skills sob demanda | Funil de leitura no `AGENTS.md` | Adotar |
| 20 | Verificação em camadas; mocks de canvas não são evidência visual; **sem GL = bloqueado**; capturas congeladas | `xtask test-visual`; GL ausente é `blocked`, nunca verde | Traduzir |
| 21 | Toda capacidade nova: comando/consulta, **superfície de inspeção**, testes, docs EN + pt-BR | **Manifesto de capacidades** verificável + "sala de depuração" | **Nova** |
| 22 | Ciclo: escopo → baseline → regressão → mudança mínima → verificar → docs; entrega com **um** próximo passo | Igual | Adotar |

Mais duas regras de contexto que também valem: *"estatísticas da wiki são geradas das definições"* (nossa referência da API vem do schema) e *"código legado não segue tudo; migrar ao tocar, sem renomeação global às cegas"*.

## Seis pontos de atrito (com resolução)

### 1. `Command` como trait com 245 implementações
O Petunia3D define `Command` como trait; há **245** `impl Command for …` em 29 arquivos (`LOCAL`). A regra 9 proíbe hierarquias de classes.

**Resolução:** o **comando é um registro de dados** (serializável) mais uma função registrada numa **tabela**. Vantagem extra: o mesmo registro viaja pela rede, pelo MCP e pelo histórico de undo. Copiamos a **semântica** (validação prévia, transação, undo), não o padrão de trait.

### 2. "Tudo é comando" × "helpers e quadros não são comandos"
Nossa documentação dizia "tudo é comando". O Digi-Tactics afirma que **funções internas e atualizações por quadro não são comandos**.

**Resolução:** vale a versão dele. **Toda mutação feita por usuário, script ou agente é um comando.** Sistemas internos (física, cordas, clima por quadro) não são. Isso também evita inflar o schema.

### 3. `ModuleRegistry` com `dyn Module`
O registro de módulos do Petunia3D usa objetos de trait. É exatamente o "localizador/framework genérico" que a regra 9 proíbe.

**Resolução:** **não copiar.** Usar uma **tabela ordenada de sistemas** (nome, fase, função). É dado, e a ordem do quadro fica visível.

### 4. Abreviações no código que vamos copiar
Medido (`LOCAL`, 2026-10-24):

| Palavra | Petunia3D | Aipo |
|---|---:|---:|
| `ctx` | 423 | 77 |
| `idx` | 260 | 284 |
| `cfg` | 239 | 172 |
| `mat` | 189 | 15 |
| `dt` | 52 | 46 |
| `msg` | 40 | 110 |

**Resolução:** o código copiado é **legado**. Registrar o commit de origem, migrar **ao tocar**, com teste de regressão, **sem renomeação global às cegas** (regra do próprio Digi-Tactics). Coordenadas `x y z w`, cores `r g b a` e UV `u v` continuam permitidas.

### 5. Classes do Qt na GUI
O Qt obriga herança (`QWidget`). A regra 9 permite **classes de biblioteca em adaptadores**.

**Resolução:** a GUI inteira é um **adaptador**. As regras ficam em Rust puro (o crate `editor` já é assim, `LOCAL`). Nenhuma classe própria sobe para o domínio.

### 6. Idioma da documentação
O Digi-Tactics usa **inglês canônico + pt-BR sincronizado**, com identificadores de página iguais. Nossa documentação atual está **só em pt-BR**.

**Resolução proposta (`DR-19`):** a documentação atual é **rascunho de discussão em pt-BR**. Ao iniciar a implementação, passa a **inglês canônico (`docs/en`) + pt-BR (`docs/pt-BR`)**, e a referência da API é **gerada do schema** nos dois idiomas. O `AGENTS.md` já nasce em inglês.

## O que a filosofia deles melhora na nossa documentação

| Ideia | Onde entra |
|---|---|
| Campo `status` (verificado, provisório, aproximação, planejado) | Schema da API |
| Manifesto *feature → comando → consulta → painel → teste → doc* | `docs/capabilities.json` + `xtask` |
| **Sala de depuração**: cena inspecionável com API de comandos e consultas | Modo "Jogar" do editor e um app `debug` |
| Testes visuais com perfil dedicado e **sem focar a área de trabalho** | Fase 4 |
| Capturas congeladas: semente, tempo, câmera, viewport, luz | Regressão visual |
| Dados de equilíbrio/estatísticas **geradas** das definições | Referência da API; Project Doctor |
| Registro das limitações e de "o que não é" | Já adotado nos documentos |

## Decisões ligadas

`DR-02` base técnica · `DR-15` gates · `DR-19` idioma da documentação.
