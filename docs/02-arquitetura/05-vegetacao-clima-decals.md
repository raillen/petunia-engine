# Vegetação, clima, decals e billboards 3D

**Revisão:** 2026-10-24 · **Estado:** proposta; precisa de protótipos

## Resumo (TL;DR)

Quatro sistemas que dão vida ao mapa. Todos seguem a mesma ideia: **o arquivo guarda regras e retoques manuais**, e a engine **gera** as instâncias de forma repetível. Arquivos ficam pequenos e o criador edita a regra, não 50 mil árvores.

## Vegetação

### O que o criador faz
Escolhe "Floresta de pinheiros" e pinta a área. Ajusta densidade, mistura e exclusões.

### Como funciona
| Tema | Proposta |
|---|---|
| **Regra de espalhamento** | Mapa de densidade pintado + filtros: inclinação, altitude, distância da água, **exclusão por estrada**, agrupamento (clumping), escala e rotação aleatórias |
| **Repetível** | Semente por mundo. As instâncias nascem iguais em toda máquina (importa para online) |
| **O que se guarda** | Regras + retoques manuais (adicionar/remover/mover). Não as milhões de instâncias |
| **Streaming** | Instâncias geradas **por chunk**, perto da câmera |
| **Desenho** | Instanciado por chunk; poucas chamadas de desenho por nível de detalhe |
| **Vento** | **Uma função única**, usada por todos os níveis de detalhe e também por cordas, bandeiras e partículas |
| **Colisão** | Só troncos próximos do jogador, de um pool |
| **Interação** | Opcional: plantas se curvam quando algo passa (textura pequena com posições de atores) |

Resumo conferido (`WEB`): instanciar por tile/chunk, **culling e LOD**, uma função de vento compartilhada entre LODs e esconder trocas de LOD com **cross-fade ou pontilhado (dither)**. Em um exemplo medido, trocar copas de 100–180 triângulos por impostores levou o LOD distante de 2–3,6 ms para 0,2–0,7 ms. O gargalo observado foi o **vértice**, então reduzir geometria vale mais que otimizar sombreador.

### Níveis de detalhe (LOD)

```text
 perto ─────────────────────────────────────────────► longe
 Malha completa → Malha simples → Billboard 3D (impostor) → nada
```

## Billboards 3D

"Billboard 3D" aqui significa o **impostor**: uma imagem que **parece ter volume** de longe.

| Tipo | O que é | Uso |
|---|---|---|
| **Axial** | Um quadro que gira só em torno do eixo vertical | Árvores muito distantes |
| **Cruzado** | Dois ou três quadros em X | Arbustos, grama média |
| **Octaédrico** | Atlas de várias vistas; escolhe e mistura a vista certa | Árvores e rochas de longe, com **iluminação coerente** |

Problemas conhecidos (`WEB`): troca de malha para billboard é visível sem mistura; objetos assimétricos precisam de várias vistas (por isso o octaédrico).

**Geração:** uma ferramenta **assa** o atlas a partir do modelo. O Petunia3D já tem exportação de modelos e UV por atlas (`xatlas`, `LOCAL`), então o "assar impostor" pode viver lá ou no editor da engine (`DR-11`).

## Clima

### O que o criador faz
Move um controle de **hora do dia** e escolhe um preset ("Tempestade"). Pode definir **zonas** de clima.

### Como funciona
Um **estado único** de atmosfera alimenta todo o resto:

```text
  Estado:  hora · nuvens · chuva · neve · névoa · vento · umidade do chão · temperatura
     │
     ├─► Céu e sol            (modelo analítico)
     ├─► Nuvens               (camada 2D ou raymarch leve opcional)
     ├─► Precipitação         (partículas presas à câmera)
     ├─► Molhado e poças      (parâmetro de material)
     ├─► Neve acumulada       (máscara por normal voltada para cima)
     ├─► Vento                (vegetação, cordas, bandeiras, partículas)
     └─► Áudio                (chuva, vento, trovão)
```

Boas práticas conferidas (`WEB`):

- **Camadas independentes** permitem escalar do PC ao portátil.
- **Transições por interpolação**, nunca saltos.
- **"Camada de memória":** o chão **demora mais para secar** do que a chuva para parar. Cobertura de nuvens, névoa e umidade respondem em **tempos diferentes**.
- Efeitos de superfície vêm de **material** (molhado, poça, neve), não de objetos novos.
- Para hardware fraco: **eventos de clima roteirizados** e variação por região dão ilusão de complexidade.

### Conforto sensorial no clima
Relâmpago e trovão são gatilhos comuns de fotossensibilidade e de sensibilidade sonora. O estado de clima deve respeitar o **limitador de flash** e um controle de **intensidade de chuva/trovão** (veja [pós-processamento](03-renderizacao-e-pos.md) e [acessibilidade](../04-acessibilidade/02-jogos-criados.md)).

### Ganchos de jogo
```aipo
# Ilustrativo
weather.set(preset = "storm", transition_seconds = 30)
weather.on_change(fn(e) { ui.say(f"Clima: {e.name}") })
```

## Decals

Decal é uma imagem **projetada** sobre superfícies (lama, marca de pneu, sujeira, rachadura, grafite, marcação de faixa).

> Cuidado com o nome: o Petunia3D tem "decalque" de **pintura de textura** (`LOCAL`, `decal_tool.rs`). O decal da engine é **projetado em tempo real**. São coisas diferentes. A experiência de edição (alças de mover, escalar, girar e valor digitado) pode ser reaproveitada.

### Duas técnicas (`WEB`)

| | **Deferred (em espaço de tela)** | **Clusterizada (forward)** |
|---|---|---|
| Precisa de | Profundidade + G-Buffer | Lista de decals por cluster; muda os sombreadores de material |
| Vantagem | Simples e barato; iluminação uma vez por pixel | Mistura de normal mais fácil; boa com MSAA |
| Problema | Não funciona em geometria transparente; mistura de normal em MRT é frágil | Complexidade, mais registradores, custo em runtime |

### Proposta em duas etapas (`DR-12`)
1. **Etapa 1:** decals projetados **depois do opaco**, usando só a profundidade (sem G-Buffer), com iluminação simples. Cobre estrada, lama, marcas. Barato.
2. **Etapa 2 (se precisar):** decals clusterizados para mistura de normal precisa.

### Regras de jogo
| Regra | Motivo |
|---|---|
| **Orçamento e vida útil** (máximo de N; esmaecer o mais antigo) | Custo previsível |
| **Máscara de quem recebe** (por camada) | Evita decal em personagem |
| **Opção "sem sangue e gore"** no sistema de decals | Game Accessibility Guidelines (`WEB`) |
| **Decal em faixa** ao longo de spline | Marcação de estradas, rachaduras |

## Tabelas de dados (esboço)

```text
 ScatterRule : área, densidade_mapa, filtros, planta, semente
 PlantAsset  : malhas[LOD], impostor, vento{força, frequência}
 Billboard   : tipo, atlas, vistas
 WeatherZone : área, preset, prioridade
 Decal       : material, tamanho, camada_alvo, vida_util
```

## Decisões ligadas

`DR-12` decals · `DR-13` pós · `DR-11` onde assar impostores.
