# Glossário

**Revisão:** 2026-10-24 · Cada termo em uma ou duas frases, em palavras simples.

## Resumo (TL;DR)

Se um termo travar a leitura, procure aqui. Está em ordem alfabética.

| Termo | Em palavras simples |
|---|---|
| **Ação (de entrada)** | Nome dado a uma intenção do jogador, como `jump`. A tecla fica separada e o jogador pode trocá-la. |
| **AccessKit** | Biblioteca que conecta o programa ao leitor de tela do sistema. |
| **AHS (Aipo Host Schema)** | Descrição, em dados, de tudo o que uma engine oferece à linguagem Aipo. |
| **AST (árvore sintática)** | O texto do código transformado numa árvore que o programa entende. |
| **Billboard 3D / impostor** | Imagem que parece ter volume quando vista de longe. Substitui malhas caras. |
| **Auto-battler** | Jogo tático em que você monta o time e o combate acontece sozinho (estilo Teamfight Tactics). |
| **Bloom** | Brilho suave ao redor de luzes fortes. |
| **Brush (convexo)** | Forma sólida simples (caixa, rampa, cilindro) usada para montar níveis rapidamente. |
| **Capability (permissão)** | Licença explícita para fazer algo (ler o mundo, tocar som). Tudo é negado por padrão. |
| **Chunk (bloco)** | Pedaço do mundo carregado ou processado de uma vez. |
| **Comando** | Qualquer ação do usuário ou script, com id, validação e desfazer. |
| **CommandBuffer** | Fila de mudanças que só são aplicadas num ponto seguro do quadro. |
| **Decal** | Imagem projetada sobre superfícies (lama, marca de pneu). |
| **Deferred / Forward** | Duas formas de calcular luz: guardar dados e iluminar depois (deferred), ou iluminar ao desenhar (forward). |
| **Cel-shading (toon)** | Estilo visual com degraus de luz e contorno, como desenho animado. |
| **Determinismo** | Mesma entrada, mesmo resultado, sempre. Importa para replay e online. |
| **ECS** | Organizar o mundo em entidades, dados e sistemas, em vez de classes. |
| **Entidade** | Uma "coisa" do mundo; uma linha numa tabela. |
| **Event Sheet** | Lista de regras "QUANDO / SE / FAÇA" para criar lógica sem programar. |
| **Flow map (mapa de fluxo)** | Textura ou dados que dizem para onde a água corre. |
| **Gameplay IR** | Representação neutra e tipada das regras do jogo. As telas visuais editam o IR; o IR gera código Aipo ou Luau. |
| **Gerstner (onda)** | Onda matemática que dá crista afiada e vale suave à água. |
| **glTF / GLB** | Formato aberto para modelos 3D de jogos. |
| **Handle** | Etiqueta que aponta para algo do mundo, sem expor o objeto. Fica "obsoleto" se o objeto some. |
| **Hot reload** | Trocar o código com o jogo rodando. |
| **Host (hospedeiro)** | O programa que roda a linguagem; aqui, a engine. |
| **HostValue** | Valor simples que passa entre a engine e o script. |
| **IK (cinemática inversa)** | Posicionar um membro (pé, mão) e deixar o resto se ajustar. |
| **Impostor octaédrico** | Impostor com várias vistas, para parecer 3D de qualquer ângulo. |
| **Interpolação** | Suavizar o movimento entre dois estados. |
| **LOD (nível de detalhe)** | Versões mais simples de um objeto para quando ele está longe. |
| **LSP (Language Server Protocol)** | Serviço que dá autocomplete, erros e ajuda a qualquer editor de código. |
| **LUT (cor)** | Tabela que troca cores por outras; usada na gradação de cor. |
| **Luau** | Variante rápida e tipada da Lua, criada pela Roblox. |
| **MCP** | Protocolo para agentes de IA usarem ferramentas. |
| **Nó (visual)** | Bloco gráfico que representa uma chamada ou construção do código. |
| **Pós-processamento** | Efeitos aplicados à imagem pronta (cor, brilho, bordas). |
| **Predição + reconciliação** | O cliente adivinha o próprio movimento e corrige quando o servidor responde. |
| **petunia-libs** | Repositório de crates pequenas e versionadas, compartilhadas entre o Petunia3D e a engine. |
| **Prefab** | Molde reutilizável de um ou mais objetos. |
| **Project Doctor** | Validador do projeto (mapa, física, scripts, acessibilidade) com botão "Corrigir". Também chamado Issue Browser. |
| **Relay** | Servidor que ajuda jogadores a se conectarem atrás de roteadores. |
| **Replicação** | Copiar o estado do jogo entre computadores. |
| **RuntimeWorld** | O mundo em execução, separado dos dados de autoria do editor. |
| **Rollback netcode** | Reexecutar o passado quando chega informação atrasada. Exige simulação determinística. |
| **Round-trip (ida e volta)** | Converter A para B e de volta para A, sem perder nada. |
| **Schema (da API)** | A descrição única, em dados, de todas as funções da engine. |
| **Skinning** | Deformar a malha de um personagem com os ossos do esqueleto. |
| **Spike** | Experimento curto e descartável para responder uma pergunta técnica. |
| **Spline** | Curva suave definida por poucos pontos. |
| **Tabela (componente)** | Conjunto de colunas de dados; "Porta: aberta, chave". |
| **Teste diferencial** | Rodar duas versões do mesmo código com a mesma entrada e exigir resultado idêntico. |
| **Tonemap** | Converte a luz do jogo (HDR) para a tela comum, preservando cores. |
| **Varredura (sweep)** | Arrastar um perfil ao longo de uma curva para gerar malha (estrada, tubo de corda). |
| **wgpu** | Biblioteca gráfica em Rust que fala com várias APIs (Vulkan, Metal, DirectX, WebGPU). |
| **XPBD** | Método para simular cordas e tecidos por restrições de distância; estável e simples. |
