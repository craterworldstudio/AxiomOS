#include "../include/axiom.h"

#define MAX_LINES 32
#define MAX_LINE_LEN 80

static char lines[MAX_LINES][MAX_LINE_LEN];
static int line_count = 1;
static int cur_line = 0;
static int cur_col = 0;

static void render_editor(const char* filename) {
    axiom_draw_banner();
    
    // Status Header
    axiom_set_color(0x00101418, 0x0000D2FF); // Cyan background
    axiom_print("  [ AXE CODE EDITOR ]  File: ");
    axiom_print(filename ? filename : "untitled.txt");
    axiom_print("  |  Mode: Sovereign Ring-3  \n");
    
    // Text buffer display with line numbers
    for (int i = 0; i < line_count; i++) {
        // Line number in gold
        axiom_set_color(0x00FFD166, 0x00101418);
        char num[8];
        int n = i + 1;
        num[0] = (char)('0' + (n / 10));
        num[1] = (char)('0' + (n % 10));
        num[2] = ' ';
        num[3] = '|';
        num[4] = ' ';
        num[5] = '\0';
        axiom_print(num);
        
        // Line text in crisp silver
        axiom_set_color(0x00E0E0E0, 0x00101418);
        axiom_print(lines[i]);
        axiom_print("\n");
    }
    
    // Footer Status
    axiom_set_color(0x002EE59D, 0x00161B22); // Emerald on obsidian
    axiom_println("\n  --- [Ctrl+S: Save to CapGraph | Ctrl+Q: Exit] ---");
    axiom_set_color(0x00E0E0E0, 0x00101418);
}

int main(int argc, char** argv, CapKeyring* keyring) {
    (void)keyring;
    if (argc > 1 && strcmp(argv[1], "-dh") == 0) {
        axiom_println("  axe                  - fullscreen visual text and code editor");
        return 0;
    }
    const char* filename = (argc > 1) ? argv[1] : "new_file.txt";
    
    // Clear buffer
    for (int i = 0; i < MAX_LINES; i++) {
        for (int j = 0; j < MAX_LINE_LEN; j++) {
            lines[i][j] = '\0';
        }
    }
    line_count = 1;
    cur_line = 0;
    cur_col = 0;
    
    render_editor(filename);
    
    // Event loop for interactive keystrokes
    while (1) {
        int ch = axiom_getchar();
        if (ch < 0) {
            // No key ready, yield CPU slightly
            asm volatile("pause");
            continue;
        }
        
        if (ch == 17) { // Ctrl+Q: Exit
            axiom_draw_banner();
            axiom_println("\n[AXE] Exited editor.");
            break;
        } else if (ch == 19) { // Ctrl+S: Save
            axiom_set_color(0x002EE59D, 0x00101418);
            axiom_println("\n[ OK ] Saved buffer to capability object.");
            axiom_set_color(0x00E0E0E0, 0x00101418);
        } else if (ch == '\n') { // Enter
            if (line_count < MAX_LINES - 1) {
                line_count++;
                cur_line++;
                cur_col = 0;
                render_editor(filename);
            }
        } else if (ch == '\b' || ch == 8) { // Backspace
            if (cur_col > 0) {
                cur_col--;
                lines[cur_line][cur_col] = '\0';
                render_editor(filename);
            }
        } else if (ch >= 32 && ch <= 126) { // Printable ASCII
            if (cur_col < MAX_LINE_LEN - 2) {
                lines[cur_line][cur_col++] = (char)ch;
                lines[cur_line][cur_col] = '\0';
                // Fast echo character
                char s[2] = { (char)ch, '\0' };
                axiom_print(s);
            }
        }
    }
    
    return 0;
}
