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

// Selection & Clipboard State
static int is_selecting = 0;
static int sel_anchor_line = 0;
static int sel_anchor_col = 0;
static char clipboard[1024];
static int clipboard_len = 0;
static char status_msg[64] = "";

static void copy_selection(void) {
    if (!is_selecting) return;
    int s_line = sel_anchor_line, s_col = sel_anchor_col;
    int e_line = cur_line, e_col = cur_col;
    if (s_line > e_line || (s_line == e_line && s_col > e_col)) {
        int tl = s_line; s_line = e_line; e_line = tl;
        int tc = s_col; s_col = e_col; e_col = tc;
    }
    int pos = 0;
    for (int i = s_line; i <= e_line && i < line_count; i++) {
        size_t llen = strlen(lines[i]);
        int start_j = (i == s_line) ? s_col : 0;
        int end_j = (i == e_line) ? e_col : (int)llen;
        for (int j = start_j; j < end_j && j < (int)llen && pos < 1022; j++) {
            clipboard[pos++] = lines[i][j];
        }
        if (i < e_line && pos < 1022) {
            clipboard[pos++] = '\n';
        }
    }
    clipboard[pos] = '\0';
    clipboard_len = pos;
    strcpy(status_msg, "COPIED TO CLIPBOARD");
}

static void paste_clipboard(void) {
    if (clipboard_len == 0) return;
    is_selecting = 0;
    for (int idx = 0; idx < clipboard_len; idx++) {
        char ch = clipboard[idx];
        if (ch == '\r') continue;
        if (ch == '\n') {
            if (line_count < MAX_LINES - 1) {
                for (int i = line_count; i > cur_line + 1; i--) {
                    strcpy(lines[i], lines[i - 1]);
                }
                strcpy(lines[cur_line + 1], &lines[cur_line][cur_col]);
                lines[cur_line][cur_col] = '\0';
                line_count++;
                cur_line++;
                cur_col = 0;
            }
        } else {
            size_t llen = strlen(lines[cur_line]);
            if (llen < MAX_LINE_LEN - 2) {
                for (int i = (int)llen; i >= cur_col; i--) {
                    lines[cur_line][i + 1] = lines[cur_line][i];
                }
                lines[cur_line][cur_col] = ch;
                cur_col++;
            }
        }
    }
    strcpy(status_msg, "PASTED FROM CLIPBOARD");
}

static void render_editor_inplace(void) {
    // 1. Status Header: row 1
    axiom_set_cursor(1, 0);
    axiom_set_color(0x00101418, 0x0000D2FF); // Electric Cyan
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
    pos_buf[10] = '\0';
    axiom_print(pos_buf);

    if (is_selecting) {
        axiom_print("  [SEL]   ");
    } else {
        axiom_print("          ");
    }
    for (int p = 0; p < 25; p++) {
        axiom_print(" ");
    }

    // Determine normalized selection range
    int s_line = sel_anchor_line, s_col = sel_anchor_col;
    int e_line = cur_line, e_col = cur_col;
    if (s_line > e_line || (s_line == e_line && s_col > e_col)) {
        int tl = s_line; s_line = e_line; e_line = tl;
        int tc = s_col; s_col = e_col; e_col = tc;
    }

    // 2. Render each line in place (rows 2 .. 2 + line_count + 1)
    for (int i = 0; i < MAX_LINES; i++) {
        if (i < line_count) {
            axiom_set_cursor(2 + i, 0);
            
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
            int max_c = (i == cur_line && cur_col >= (int)llen) ? cur_col + 1 : (int)llen;

            for (int j = 0; j < max_c; j++) {
                char ch = (j < (int)llen) ? lines[i][j] : ' ';
                int is_sel = 0;
                if (is_selecting) {
                    if (i > s_line && i < e_line) is_sel = 1;
                    else if (i == s_line && i == e_line && j >= s_col && j < e_col) is_sel = 1;
                    else if (i == s_line && i < e_line && j >= s_col) is_sel = 1;
                    else if (i == e_line && i > s_line && j < e_col) is_sel = 1;
                }

                if (i == cur_line && j == cur_col) {
                    // Visual Cursor: Inverted Electric Cyan
                    axiom_set_color(0x00101418, 0x0000D2FF);
                } else if (is_sel) {
                    // Text Selection: Obsidian Sapphire
                    axiom_set_color(0x00FFFFFF, 0x00005F87);
                } else {
                    // Normal text
                    axiom_set_color(0x00E0E0E0, 0x00101418);
                }
                char s[2] = { ch, '\0' };
                axiom_print(s);
            }

            // Pad remainder of line to wipe previous longer contents
            axiom_set_color(0x00E0E0E0, 0x00101418);
            for (int k = max_c; k < 82; k++) {
                axiom_print(" ");
            }
        } else if (i < line_count + 2) {
            // Erase any newly deleted lines
            axiom_set_cursor(2 + i, 0);
            axiom_set_color(0x00E0E0E0, 0x00101418);
            for (int k = 0; k < 88; k++) {
                axiom_print(" ");
            }
        }
    }

    // 3. Status Footer at row 2 + line_count + 1
    axiom_set_cursor(2 + line_count + 1, 0);
    if (status_msg[0] != '\0') {
        axiom_set_color(0x0000D2FF, 0x00161B22);
        axiom_print("  --- [ ");
        axiom_print(status_msg);
        axiom_print(" ] ---");
    } else if (saved_indicator) {
        axiom_set_color(0x002EE59D, 0x00161B22); // Emerald Green
        axiom_print("  --- [ OK ] Saved to Sovereign Capability Object ---");
    } else {
        axiom_set_color(0x00FFD166, 0x00161B22); // Gold Yellow
        axiom_print("  --- [Ctrl+S: Save | Ctrl+Q: Exit | Ctrl+C/V: Clip | Shift+Arrows: Select] ---");
    }
    axiom_set_color(0x00E0E0E0, 0x00101418);
    for (int k = 0; k < 20; k++) {
        axiom_print(" ");
    }
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
    strcpy(status_msg, "SAVED TO CAPABILITY OBJECT");
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
    is_selecting = 0;
    clipboard_len = 0;
    status_msg[0] = '\0';

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

    // Draw full desktop banner ONCE on launch
    axiom_draw_banner();
    render_editor_inplace();

    // Event loop for interactive keystrokes
    while (1) {
        int ch = axiom_getchar();
        if (ch < 0) {
            asm volatile("pause");
            continue;
        }

        // Reset status message on next interaction
        if (status_msg[0] != '\0' && ch != KEY_CTRL_C && ch != KEY_CTRL_V && ch != KEY_CTRL_A) {
            status_msg[0] = '\0';
        }

        if (ch == KEY_CTRL_Q) { // Ctrl+Q: Exit
            axiom_draw_banner();
            axiom_println("\n[AXE] Exited editor.");
            break;
        } else if (ch == KEY_CTRL_S) { // Ctrl+S: Save
            save_buffer();
            saved_indicator = 1;
            render_editor_inplace();
        } else if (ch == KEY_CTRL_A) { // Ctrl+A: Select All
            is_selecting = 1;
            sel_anchor_line = 0;
            sel_anchor_col = 0;
            cur_line = line_count - 1;
            cur_col = (int)strlen(lines[cur_line]);
            strcpy(status_msg, "SELECT ALL");
            render_editor_inplace();
        } else if (ch == KEY_CTRL_C) { // Ctrl+C: Copy
            copy_selection();
            render_editor_inplace();
        } else if (ch == KEY_CTRL_V) { // Ctrl+V: Paste
            paste_clipboard();
            saved_indicator = 0;
            render_editor_inplace();
        } else if (ch == KEY_SHIFT_UP) { // Shift+Up Arrow
            if (!is_selecting) {
                is_selecting = 1;
                sel_anchor_line = cur_line;
                sel_anchor_col = cur_col;
            }
            if (cur_line > 0) {
                cur_line--;
                size_t llen = strlen(lines[cur_line]);
                if (cur_col > (int)llen) cur_col = (int)llen;
            }
            render_editor_inplace();
        } else if (ch == KEY_SHIFT_DOWN) { // Shift+Down Arrow
            if (!is_selecting) {
                is_selecting = 1;
                sel_anchor_line = cur_line;
                sel_anchor_col = cur_col;
            }
            if (cur_line < line_count - 1) {
                cur_line++;
                size_t llen = strlen(lines[cur_line]);
                if (cur_col > (int)llen) cur_col = (int)llen;
            }
            render_editor_inplace();
        } else if (ch == KEY_SHIFT_LEFT) { // Shift+Left Arrow
            if (!is_selecting) {
                is_selecting = 1;
                sel_anchor_line = cur_line;
                sel_anchor_col = cur_col;
            }
            if (cur_col > 0) {
                cur_col--;
            } else if (cur_line > 0) {
                cur_line--;
                cur_col = (int)strlen(lines[cur_line]);
            }
            render_editor_inplace();
        } else if (ch == KEY_SHIFT_RIGHT) { // Shift+Right Arrow
            if (!is_selecting) {
                is_selecting = 1;
                sel_anchor_line = cur_line;
                sel_anchor_col = cur_col;
            }
            size_t llen = strlen(lines[cur_line]);
            if (cur_col < (int)llen) {
                cur_col++;
            } else if (cur_line < line_count - 1) {
                cur_line++;
                cur_col = 0;
            }
            render_editor_inplace();
        } else if (ch == KEY_UP) { // Up Arrow (clears selection)
            is_selecting = 0;
            saved_indicator = 0;
            if (cur_line > 0) {
                cur_line--;
                size_t llen = strlen(lines[cur_line]);
                if (cur_col > (int)llen) cur_col = (int)llen;
                render_editor_inplace();
            }
        } else if (ch == KEY_DOWN) { // Down Arrow (clears selection)
            is_selecting = 0;
            saved_indicator = 0;
            if (cur_line < line_count - 1) {
                cur_line++;
                size_t llen = strlen(lines[cur_line]);
                if (cur_col > (int)llen) cur_col = (int)llen;
                render_editor_inplace();
            }
        } else if (ch == KEY_LEFT) { // Left Arrow (clears selection)
            is_selecting = 0;
            saved_indicator = 0;
            if (cur_col > 0) {
                cur_col--;
                render_editor_inplace();
            } else if (cur_line > 0) {
                cur_line--;
                cur_col = (int)strlen(lines[cur_line]);
                render_editor_inplace();
            }
        } else if (ch == KEY_RIGHT) { // Right Arrow (clears selection)
            is_selecting = 0;
            saved_indicator = 0;
            size_t llen = strlen(lines[cur_line]);
            if (cur_col < (int)llen) {
                cur_col++;
                render_editor_inplace();
            } else if (cur_line < line_count - 1) {
                cur_line++;
                cur_col = 0;
                render_editor_inplace();
            }
        } else if (ch == KEY_HOME) { // Home
            is_selecting = 0;
            saved_indicator = 0;
            cur_col = 0;
            render_editor_inplace();
        } else if (ch == KEY_END) { // End
            is_selecting = 0;
            saved_indicator = 0;
            cur_col = (int)strlen(lines[cur_line]);
            render_editor_inplace();
        } else if (ch == '\n' || ch == 13) { // Enter
            is_selecting = 0;
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
                render_editor_inplace();
            }
        } else if (ch == '\b' || ch == 8 || ch == 127) { // Backspace
            is_selecting = 0;
            saved_indicator = 0;
            if (cur_col > 0) {
                size_t llen = strlen(lines[cur_line]);
                for (size_t i = cur_col - 1; i < llen; i++) {
                    lines[cur_line][i] = lines[cur_line][i + 1];
                }
                cur_col--;
                render_editor_inplace();
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
                    render_editor_inplace();
                }
            }
        } else if (ch == KEY_DELETE) { // Delete
            is_selecting = 0;
            saved_indicator = 0;
            size_t llen = strlen(lines[cur_line]);
            if (cur_col < (int)llen) {
                for (size_t i = cur_col; i < llen; i++) {
                    lines[cur_line][i] = lines[cur_line][i + 1];
                }
                render_editor_inplace();
            } else if (cur_col == (int)llen && cur_line < line_count - 1) {
                size_t next_len = strlen(lines[cur_line + 1]);
                if (llen + next_len < MAX_LINE_LEN - 1) {
                    strcat(lines[cur_line], lines[cur_line + 1]);
                    for (int i = cur_line + 1; i < line_count - 1; i++) {
                        strcpy(lines[i], lines[i + 1]);
                    }
                    lines[line_count - 1][0] = '\0';
                    line_count--;
                    render_editor_inplace();
                }
            }
        } else if (ch >= 32 && ch <= 126) { // Printable ASCII
            is_selecting = 0;
            saved_indicator = 0;
            size_t llen = strlen(lines[cur_line]);
            if (llen < MAX_LINE_LEN - 2) {
                for (int i = (int)llen; i >= cur_col; i--) {
                    lines[cur_line][i + 1] = lines[cur_line][i];
                }
                lines[cur_line][cur_col] = (char)ch;
                cur_col++;
                render_editor_inplace();
            }
        }
    }

    return 0;
}
