#include "../include/axiom.h"

#define MAX_LINES 32
#define MAX_LINE_LEN 80

static char lines[MAX_LINES][MAX_LINE_LEN];
static int line_count = 1;
static int cur_line = 0;
static int cur_col = 0;
static int saved_indicator = 0;
static CapBlobContext* blob = NULL;
static const char* active_filename = NULL;

static void render_editor(void) {
    axiom_draw_banner();
    
    // Status Header: Cyan background
    axiom_set_color(0x00101418, 0x0000D2FF);
    axiom_print("  [ AXE CODE EDITOR ]  File: ");
    axiom_print(active_filename ? active_filename : "untitled.txt");
    axiom_print("  |  Line: ");
    char pos_buf[16];
    int l = cur_line + 1;
    pos_buf[0] = (char)('0' + (l / 10));
    pos_buf[1] = (char)('0' + (l % 10));
    pos_buf[2] = ' ';
    pos_buf[3] = 'C';
    pos_buf[4] = 'o';
    pos_buf[5] = 'l';
    pos_buf[6] = ':';
    pos_buf[7] = ' ';
    int c = cur_col + 1;
    pos_buf[8] = (char)('0' + (c / 10));
    pos_buf[9] = (char)('0' + (c % 10));
    pos_buf[10] = '\n';
    pos_buf[11] = '\0';
    axiom_print(pos_buf);
    
    // Text buffer display with line numbers and cursor
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
        
        size_t llen = strlen(lines[i]);
        if (i == cur_line) {
            // Draw characters before cursor
            axiom_set_color(0x00E0E0E0, 0x00101418);
            for (int j = 0; j < cur_col && j < (int)llen; j++) {
                char s[2] = { lines[i][j], '\0' };
                axiom_print(s);
            }
            
            // Draw cursor character (inverted)
            axiom_set_color(0x00101418, 0x0000D2FF);
            char cur_ch[2] = { (cur_col < (int)llen) ? lines[i][cur_col] : ' ', '\0' };
            axiom_print(cur_ch);
            
            // Draw characters after cursor
            axiom_set_color(0x00E0E0E0, 0x00101418);
            for (int j = cur_col + 1; j < (int)llen; j++) {
                char s[2] = { lines[i][j], '\0' };
                axiom_print(s);
            }
        } else {
            axiom_set_color(0x00E0E0E0, 0x00101418);
            axiom_print(lines[i]);
        }
        axiom_print("\n");
    }
    
    // Footer Status
    if (saved_indicator) {
        axiom_set_color(0x002EE59D, 0x00161B22); // Emerald Green
        axiom_println("\n  --- [ OK ] Saved to Capability Object ---");
    } else {
        axiom_set_color(0x00FFD166, 0x00161B22); // Gold Yellow
        axiom_println("\n  --- [Ctrl+S: Save | Ctrl+Q: Exit | Arrows: Navigate] ---");
    }
    axiom_set_color(0x00E0E0E0, 0x00101418);
}

static void save_buffer(void) {
    if (!blob) return;
    uint32_t pos = 0;
    for (int i = 0; i < line_count; i++) {
        size_t len = strlen(lines[i]);
        for (size_t j = 0; j < len && pos < 1022; j++) {
            blob->data[pos++] = lines[i][j];
        }
        if (pos < 1023) {
            blob->data[pos++] = '\n';
        }
    }
    blob->data[pos] = '\0';
    blob->length = pos;
    blob->modified = 1;
}

int main(int argc, char** argv, CapKeyring* keyring) {
    if (argc > 1 && strcmp(argv[1], "-dh") == 0) {
        axiom_println("  axe                  - fullscreen visual text and code editor");
        return 0;
    }
    
    active_filename = (argc > 1) ? argv[1] : "new_file.txt";
    blob = (CapBlobContext*)keyring->storage_cap;
    
    // Clear buffer
    for (int i = 0; i < MAX_LINES; i++) {
        for (int j = 0; j < MAX_LINE_LEN; j++) {
            lines[i][j] = '\0';
        }
    }
    line_count = 0;
    cur_line = 0;
    cur_col = 0;
    saved_indicator = 0;
    
    // Load existing file if present
    if (blob && blob->length > 0) {
        int col = 0;
        for (uint32_t i = 0; i < blob->length && line_count < MAX_LINES; i++) {
            char c = blob->data[i];
            if (c == '\r') continue;
            if (c == '\n') {
                lines[line_count][col] = '\0';
                line_count++;
                col = 0;
            } else {
                if (col < MAX_LINE_LEN - 1) {
                    lines[line_count][col++] = c;
                }
            }
        }
        if (col > 0 && line_count < MAX_LINES) {
            lines[line_count][col] = '\0';
            line_count++;
        }
    }
    
    if (line_count == 0) {
        line_count = 1;
    }
    
    render_editor();
    
    // Event loop for interactive keystrokes
    while (1) {
        int ch = axiom_getchar();
        if (ch < 0) {
            asm volatile("pause");
            continue;
        }
        
        if (ch == KEY_CTRL_Q) { // Ctrl+Q: Exit
            axiom_draw_banner();
            axiom_println("\n[AXE] Exited editor.");
            break;
        } else if (ch == KEY_CTRL_S) { // Ctrl+S: Save
            save_buffer();
            saved_indicator = 1;
            render_editor();
        } else if (ch == KEY_UP) { // Up Arrow
            saved_indicator = 0;
            if (cur_line > 0) {
                cur_line--;
                size_t llen = strlen(lines[cur_line]);
                if (cur_col > (int)llen) cur_col = (int)llen;
                render_editor();
            }
        } else if (ch == KEY_DOWN) { // Down Arrow
            saved_indicator = 0;
            if (cur_line < line_count - 1) {
                cur_line++;
                size_t llen = strlen(lines[cur_line]);
                if (cur_col > (int)llen) cur_col = (int)llen;
                render_editor();
            }
        } else if (ch == KEY_LEFT) { // Left Arrow
            saved_indicator = 0;
            if (cur_col > 0) {
                cur_col--;
                render_editor();
            } else if (cur_line > 0) {
                cur_line--;
                cur_col = (int)strlen(lines[cur_line]);
                render_editor();
            }
        } else if (ch == KEY_RIGHT) { // Right Arrow
            saved_indicator = 0;
            size_t llen = strlen(lines[cur_line]);
            if (cur_col < (int)llen) {
                cur_col++;
                render_editor();
            } else if (cur_line < line_count - 1) {
                cur_line++;
                cur_col = 0;
                render_editor();
            }
        } else if (ch == KEY_HOME) { // Home
            saved_indicator = 0;
            cur_col = 0;
            render_editor();
        } else if (ch == KEY_END) { // End
            saved_indicator = 0;
            cur_col = (int)strlen(lines[cur_line]);
            render_editor();
        } else if (ch == '\n' || ch == 13) { // Enter
            saved_indicator = 0;
            if (line_count < MAX_LINES - 1) {
                for (int i = line_count; i > cur_line + 1; i--) {
                    strcpy(lines[i], lines[i - 1]);
                }
                strcpy(lines[cur_line + 1], &lines[cur_line][cur_col]);
                lines[cur_line][cur_col] = '\0';
                line_count++;
                cur_line++;
                cur_col = 0;
                render_editor();
            }
        } else if (ch == '\b' || ch == 8 || ch == 127) { // Backspace
            saved_indicator = 0;
            if (cur_col > 0) {
                size_t llen = strlen(lines[cur_line]);
                for (size_t i = cur_col - 1; i < llen; i++) {
                    lines[cur_line][i] = lines[cur_line][i + 1];
                }
                cur_col--;
                render_editor();
            } else if (cur_col == 0 && cur_line > 0) {
                size_t prev_len = strlen(lines[cur_line - 1]);
                size_t cur_len = strlen(lines[cur_line]);
                if (prev_len + cur_len < MAX_LINE_LEN - 1) {
                    strcat(lines[cur_line - 1], lines[cur_line]);
                    for (int i = cur_line; i < line_count - 1; i++) {
                        strcpy(lines[i], lines[i + 1]);
                    }
                    lines[line_count - 1][0] = '\0';
                    line_count--;
                    cur_line--;
                    cur_col = (int)prev_len;
                    render_editor();
                }
            }
        } else if (ch == KEY_DELETE) { // Delete
            saved_indicator = 0;
            size_t llen = strlen(lines[cur_line]);
            if (cur_col < (int)llen) {
                for (size_t i = cur_col; i < llen; i++) {
                    lines[cur_line][i] = lines[cur_line][i + 1];
                }
                render_editor();
            } else if (cur_col == (int)llen && cur_line < line_count - 1) {
                size_t next_len = strlen(lines[cur_line + 1]);
                if (llen + next_len < MAX_LINE_LEN - 1) {
                    strcat(lines[cur_line], lines[cur_line + 1]);
                    for (int i = cur_line + 1; i < line_count - 1; i++) {
                        strcpy(lines[i], lines[i + 1]);
                    }
                    lines[line_count - 1][0] = '\0';
                    line_count--;
                    render_editor();
                }
            }
        } else if (ch >= 32 && ch <= 126) { // Printable ASCII
            saved_indicator = 0;
            size_t llen = strlen(lines[cur_line]);
            if (llen < MAX_LINE_LEN - 2) {
                for (int i = (int)llen; i >= cur_col; i--) {
                    lines[cur_line][i + 1] = lines[cur_line][i];
                }
                lines[cur_line][cur_col] = (char)ch;
                cur_col++;
                render_editor();
            }
        }
    }
    
    return 0;
}
