# Design da interface do editor

**Revisão:** 2026-10-25 · **Decisão:** [DR-26](../06-decisoes/registro-de-decisoes.md) · **Layout atual:** [editor](11-editor.md#layout-da-janela)

## Resumo (TL;DR)

O editor segue **um conjunto pequeno de regras**, tiradas de design systems publicados (Unity, Fluent 2, WCAG) e de três referências visuais (o redesenho do Unity que você mandou, o Figma do Petunia3D e o app Claude). Texto de 12 px, ícones de 16 px, controles de 24 px, **um** acento de cor, menus atrás do `≡` e uma **paleta de comandos** (`Ctrl+K`) para achar qualquer coisa.

## As regras (o que memorizar)

| Regra | Valor | De onde vem |
|---|---|---|
| Texto normal | **12 px** (densidade Compacta, padrão); 13 Padrão; 14 Confortável | Unity: "12 px wherever possible" `WEB` |
| Texto mínimo | 12 px, em qualquer escala | regra U08 do projeto |
| Ícones | **16 px** (desenho de 14 px com 1 px de margem) | Unity iconography `WEB` |
| Altura de botão e campo | **24 px** na Compacta; 28 e 32 nas outras | Unity: botões de 18–24 px `WEB`; **WCAG 2.5.8**: alvo mínimo 24×24 `WEB` |
| Raio | 4 px nos controles, 8 px nos painéis | Fluent 2: "Medium 4 px" para botões `WEB` |
| Espaçamento | múltiplos de 4 px | Fluent 2 (rampa 2, 4, 8, 12, 16…) `WEB` |
| Borda de campo | lados claros, **base escura com 3:1** | WCAG 1.4.11 (contraste de não-texto) `GERAL`; padrão de entrada do Fluent `GERAL` |
| Contraste de texto | ≥ 4,5:1 em todos os textos dos dois temas | WCAG 1.4.3 `GERAL`; medido (abaixo) `LOCAL` |
| Cor | neutros quentes + **um** acento (violeta Petúnia) | app Claude (creme `#faf9f5`, tinta `#141413`) `WEB`; flor petúnia |
| Ferramentas | agrupadas por tipo, com espaço entre os grupos; nada "para encher a barra" | Unity toolbar `WEB` |
| Opções da ferramenta | numa faixa própria, perto da cena | Unity Tool Settings overlay `WEB` |
| Menus | atrás de `≡` por padrão; opção "Menus por extenso" | JetBrains New UI `WEB` |
| Paleta de comandos | `Ctrl+K`, busca tolerante, letras achadas em destaque, recentes primeiro, atalho ao lado, Esc fecha | padrões de paleta `WEB` |
| Painéis | fixos (não flutuantes) e redimensionáveis | Figma UI3: painéis flutuantes "deixaram as pessoas mais lentas" `WEB` |

## Cores medidas

Contraste calculado pela fórmula da WCAG, nos valores dos arquivos de tema (`LOCAL`).

| Par | Claro | Escuro |
|---|---|---|
| Texto principal no painel | 15,6:1 | 14,0:1 |
| Texto secundário no painel | 7,3:1 | 8,6:1 |
| Texto atenuado no painel | 5,2:1 | 5,6:1 |
| Texto sobre o acento | branco 6,1:1 | escuro 6,7:1 |
| Base do campo no painel | 3,6:1 | 4,3:1 |

No tema escuro, o texto sobre o acento é **escuro**: branco daria 2,8:1.

## Antes e depois (o que mudou e por quê)

| Antes | Problema | Agora |
|---|---|---|
| Menu, barra de texto e trilho: três faixas | Muito texto antes da cena | Uma barra: `≡`, projeto, workspaces, busca, desfazer, refazer, salvar |
| Ferramentas no trilho e depois na barra | Longe da cena | Coluna flutuante no canto do viewport (Blender, Unity "Tools") |
| Cinco controles de snap na faixa | Faixa lotada | Um botão com o passo atual + `⌄` que abre os passos, o alvo e a inclinação |
| Campos de 28–30 px e texto de 14 px | Tudo grande demais | 24 px e 12 px, sem cair abaixo do alvo de 24 px |
| Editor de código numa aba de baixo | Pouco espaço para escrever | **Workspace Código**: lista de scripts, editor, saída e a API do script |
| Rótulos `rotation_degrees`, tabelas `Transform` | Inglês técnico | "Rotação", "Transformação"; o nome técnico fica na dica |
| Ícone de busca quebrado (`d="15.5 15.5 21 21"`) | Só aparecia o círculo | Caminho corrigido (`M15.5 15.5L21 21`) |

## Workspaces

| Workspace | Atalho | Painéis |
|---|---|---|
| **Cena** | `F3` | Hierarquia, viewport, Inspetor, Diagnósticos e Histórico |
| **Código** | `F4` | Scripts, editor, Problemas e Console, Referência da API |

Cada workspace **lembra o arranjo dos painéis** (abrir, fechar, tamanhos) entre sessões. Os workspaces de Pintura, Animação e Efeitos do [DR-24](../06-decisoes/registro-de-decisoes.md) continuam propostos; a troca de workspace já está pronta para recebê-los.

## Acessibilidade mantida

- Todo comando tem **menu**, e quase todos têm **atalho**; a paleta acha os que não têm.
- `Alt+letra` abre cada menu mesmo atrás do `≡`; `F10` abre o `≡`.
- Botões só com ícone têm **nome acessível** e dica com o atalho.
- A paleta mostra comandos **indisponíveis em cinza**, nunca escondidos.
- Densidade **Confortável** (32 px, texto de 14 px) e texto maior (`Ctrl+=`) continuam.

**Não testado:** leitor de tela real, navegação só por teclado nos painéis novos e as pessoas do teste S8.

## Fontes

- [Unity Foundations — tipografia](https://www.foundations.unity.com/fundamentals/typography), [botão](https://www.foundations.unity.com/components/button), [barra de ferramentas](https://www.foundations.unity.com/components/toolbar), [ícones](https://www.foundations.unity.com/fundamentals/iconography), [overlays](https://unityeditordesignsystem.unity.com/patterns/overlays)
- [Fluent 2 — layout](https://fluent2.microsoft.design/layout), [formas](https://fluent2.microsoft.design/shapes)
- [WCAG 2.5.8 — tamanho do alvo](https://www.digitala11y.com/understanding-sc-2-5-8-target-size-minimum/)
- [JetBrains New UI](https://www.jetbrains.com/help/idea/new-ui.html)
- [Figma — como desenharam o UI3](https://www.figma.com/blog/our-approach-to-designing-ui3/)
- [Paleta de comandos — padrão](https://uxpatterns.dev/patterns/advanced/command-palette), [8 estados](https://www.setproduct.com/blog/command-palette-ui-design-guide)
- App Claude — cores levantadas pela comunidade, não oficiais: [DESIGN.md](https://github.com/voltagent/awesome-design-md/blob/main/design-md/claude/DESIGN.md)
- Petunia3D (local): `docs/image-references/` (Figma do redesenho), `crates/ui-cxxqt/cpp/shell/` (barra do app, segmentado, paleta)
