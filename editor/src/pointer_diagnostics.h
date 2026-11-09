#pragma once

class QMainWindow;

// `--pointer-info`: logs the window scale factor and every click's global and window-local
// position, and draws a crosshair where each click landed. It is a measuring tool for the
// fractional-scale bug report, not a product feature: nothing else reads what it writes.
//
// Run the editor with the flag, click the places that misbehave, and send the `[escala]` and
// `[ponteiro]` lines from the terminal. The crosshair shows whether the click landed where the
// pointer was drawn.
void install_pointer_diagnostics(QMainWindow& window);
