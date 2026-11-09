#pragma once

class QMainWindow;

// `--pointer-info`: registro de diagnóstico para quando um clique "não funciona".
//
// Desenha uma cruz vermelha no ponto em que o Qt recebeu o clique (a camada não recebe mouse, então
// não muda o que o clique faz) e imprime uma linha em stderr por clique, com o widget que recebeu,
// o widget desenhado sob o cursor e a geometria da tela e da janela.
//
// Serve para separar dois problemas que parecem iguais:
//   - a cruz cai longe do ponteiro → as coordenadas chegam trocadas (escala do compositor);
//   - a cruz cai sob o ponteiro e o controle não responde → o clique chegou; quem falha é o
//     comando disparado por ele (ou algo cobre o controle sem receber o clique).
//
// Também imprime a linha `[escala]` na abertura, 1,5 s depois (quando o compositor já ajustou a
// janela) e a cada redimensionamento, com plataforma, telas, dpr e geometria da janela.
void installPointerDiagnostics(QMainWindow& window);
