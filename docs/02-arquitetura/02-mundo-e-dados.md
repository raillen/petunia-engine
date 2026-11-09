# Mundo e dados

**Revisão:** 2026-10-24 · **Estado:** proposta para discussão

## Resumo (TL;DR)

O mundo é um conjunto de **tabelas**. Cada linha é uma **entidade**. Cada coluna é um **dado** (posição, vida, "porta aberta"). Comportamento vive em **funções e regras**, nunca dentro dos dados.

Para quem programa a engine, isso é um ECS (Entity-Component-System). Para quem cria jogos, é uma planilha: **"Tabela Porta, coluna aberta"**. Os scripts falam em tabelas, não em classes.

## Exemplo

```aipo
# Ilustrativo (não é contrato). Dados: uma tabela "Door".
struct Door {
    var open = false
    needs_key: String
}

# Consulta em lote: o laço roda na engine, não no script.
let portas_perto = world.find(table = "Door", near = player, radius = 5.0)
world.set_all(portas_perto, open = true)
```

## Peças do modelo

| Peça | O que é | Detalhe |
|---|---|---|
| **Entidade** | Uma linha | Handle com índice + geração |
| **Tabela (componente)** | Um conjunto de colunas | `Transform`, `Door`, `Health`, `Rope` |
| **Tag** | Marca sem dados | `Tree`, `Enemy`, `Interactive` |
| **Regra** | "Quando X, se Y, faça Z" | Biblioteca, não palavra-chave |
| **Sistema** | Função nativa que roda por fase | Escrito em Rust; scripts só pedem dados |
| **Chunk** | Célula espacial do mundo | Para streaming, vegetação, rede |

## Dois tipos de identidade (herdado do Petunia3D)

O Petunia3D separa dois conceitos. A engine deve fazer o mesmo (`LOCAL`, `core/schema_contracts.rs`: *"persisted identity, not a session handle"*).

| Identidade | Dura | Usada em |
|---|---|---|
| **UUID persistente** | Para sempre, vai para o arquivo | Salvar, prefabs, rede entre sessões, comandos do editor |
| **Handle de sessão** (índice + geração) | Só nesta execução | Scripts e sistemas |

O handle com geração impede "usar entidade removida": um handle velho vira **obsoleto** em vez de apontar para outra coisa. O `aipo-host` já implementa isso (`HandleTable`, `LOCAL`).

## Mudanças estruturais só em pontos seguros

Criar ou remover entidade no meio de um laço corrompe o laço. Por isso:

1. O script chama `world.spawn(...)`.
2. A chamada **entra num buffer de comandos**.
3. O buffer é aplicado **no ponto seguro** do quadro (etapa 7 do quadro, veja a [visão geral](01-visao-geral.md)).

O `aipo-poppy` já tem `CommandBuffer` exatamente para isso (`LOCAL`). O script recebe o handle na hora, mas a entidade passa a existir no ponto seguro. A documentação deve dizer isso com clareza.

## Autoria e runtime: um modelo só

| Abordagem | Problema |
|---|---|
| Autoria separada do runtime (Unity: "baking") | Dois formatos, dois bugs |
| **Um modelo, colunas extras só do editor** | Escolhida |

A tabela `Transform` existe no editor e no jogo. O editor adiciona colunas como `editor.locked`, `editor.hidden`, `editor.layer`. O exportador as remove.

## Hierarquia (pai e filho)

Em vez de uma árvore de objetos, há uma coluna `parent` e um sistema que propaga transformações. A hierarquia é um **dado**, não uma classe. Prefabs são **moldes de linhas** (`LOCAL`, `prefab.rs`: snapshot com `revision`, instâncias detectam versão antiga).

## Salvar e carregar

- Formato: **ZIP com `manifest.json` + dados versionados**, como o `.petunia` (`LOCAL`).
- Tabelas serializam com **versão de schema** e migração explícita.
- Estado de jogo salvo é **só tabelas**: pequeno, repetível, bom para replay.
- Escrita **atômica**; limites contra arquivo hostil (contagem, profundidade, tamanho) como no `op_graph` (512 nós, 2048 arestas) e no `package.rs` (anti zip-bomb) (`LOCAL`).

## Opções para o armazenamento (`DR-05`)

### A. Armazenamento próprio (conjunto esparso) — **recomendada**
- Pequeno, sob controle total. Pensado para expor tabelas e consultas aos scripts.
- Custo: escrever e testar.

### B. `bevy_ecs` sozinho
- Maduro e rápido. Pode ser usado sem o resto do Bevy (`GERAL`, confirmar).
- Custo: o modelo de mundo, consultas e ciclos de versão seguem o Bevy, que quebra API com frequência (`WEB`).

### C. Árvore de cena (estilo Godot/Fyrox)
- Mais familiar para quem vem de outras engines.
- Custo: herda o desenho orientado a objetos que a filosofia quer evitar.

**Recomendação: A.** O Petunia3D concluiu que "ECS especulativo" não serve **ao modelador** (`LOCAL`); uma engine de mundos é outro problema, e a tabela é a forma mais direta de expor "dados em vez de classes" para scripts. Mitigação: manter o armazenamento atrás de uma interface pequena, para trocar depois sem reescrever os sistemas.

## Determinismo (para online e replay)

| Fonte de não determinismo | Tratamento |
|---|---|
| Aleatoriedade | Semente única por mundo (`PoppyRng`, `LOCAL`) |
| Ordem de iteração | Ordem fixa por id de entidade |
| Tempo | Passo fixo; tempo virtual na Aipo (`LOCAL`) |
| Física | Modo determinístico opcional (Rapier oferece; `WEB`, veja [física](06-fisica-e-cordas.md)) |
| Threads | Resultados combinados em ordem fixa |

Determinismo total entre plataformas é **meta**, não garantia (`HIPÓTESE`).

## 2D é um modo, não um mundo separado

Um jogo 2D usa a **mesma** cena, as **mesmas** entidades e os **mesmos** comandos: o que muda é a câmera (`ProjectionMode::Orthographic`) e o fato de a entidade ter a tabela **`Sprite`** (`asset`, `size`, `color`, `layer`, `flip_x`, `flip_y`) em vez de um `Brush`. Por isso o gizmo, o inspetor, o snap, os andares, o desfazer e a gravação valem igual; a imagem `.png` entra pelo projeto (`assets/…`) e vira pixels RGBA no quadro, sem tipo de biblioteca na cena.

Um objeto pode ter `Brush` **e** `Sprite`: o quadro traz os dois (o sólido e o quad texturizado), então a mesma entidade serve de parede e de letreiro. Como o sprite não escreve profundidade, a camada (`layer`) — não a distância — decide quem fica na frente dos outros sprites (`LOCAL`, `DR-38`).

**Animação é mais uma linha, não outro objeto.** Com `SpriteAnimation` (`frames`, `columns`, `fps`, `loop`, `playing`, `frame`) a mesma imagem vira uma **folha** cortada em quadros; sem a linha, o sprite continua estático. O quadro mostrado **não fica gravado na cena**: é `frame_at(relógio do anfitrião)`, uma função pura, então dois sprites podem tocar a mesma folha em tempos diferentes e a cena salva não muda a cada quadro. O editor congela o relógio em edição e o modo **Jogar** / `petunia run` o avançam; o renderizador só recebe o retângulo do quadro (`LOCAL`, `DR-39`).

## Decisões ligadas

`DR-05` armazenamento · `DR-10` formatos · `DR-09` online · `DR-38` 2D por `Sprite` · `DR-39` folha de sprites.
