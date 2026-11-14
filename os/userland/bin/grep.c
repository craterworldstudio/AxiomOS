#include "../include/axiom.h"

static void print_highlighted_line(const char* line, const char* pattern) {
    size_t pat_len = strlen(pattern);
    const char* curr = line;
    const char* match = strstr(curr, pattern);
    if (!match) return;
    while (match) {
        if (match > curr) {
            axiom_print_len(curr, (size_t)(match - curr));
        }
        axiom_set_color(COLOR_GOLD, 0x00101418);
        axiom_print_len(match, pat_len);
        axiom_set_color(COLOR_SILVER, 0x00101418);
        curr = match + pat_len;
        match = strstr(curr, pattern);
    }
    if (*curr) {
        axiom_print(curr);
    }
    axiom_print("\n");
}

int main(int argc, char** argv, CapKeyring* keyring) {
    if (argc > 1 && (strcmp(argv[1], "-h") == 0 || strcmp(argv[1], "--help") == 0)) {
        axiom_set_color(COLOR_GOLD, 0x00101418);
        axiom_println("Usage: grep <pattern> [file]");
        axiom_set_color(COLOR_SILVER, 0x00101418);
        axiom_println("Filter and display lines matching pattern from standard input or capability file.");
        axiom_println("Options:");
        axiom_println("  -h, --help    Show this command reference manual");
        return 0;
    }
    if (argc > 1 && strcmp(argv[1], "-dh") == 0) {
        axiom_println("  grep                 - filter lines matching pattern");
        return 0;
    }
    if (argc < 2) {
        axiom_println("grep: missing search pattern");
        return 1;
    }

    const char* pattern = argv[1];

    // If file operand was given and passed in blob context
    if (argc >= 3) {
        CapBlobContext* blob = (CapBlobContext*)keyring->storage_cap;
        if (blob && blob->length > 0) {
            char line[512];
            size_t line_pos = 0;
            for (size_t i = 0; i < blob->length; i++) {
                char ch = blob->data[i];
                if (ch == '\r') continue;
                if (ch == '\n') {
                    line[line_pos] = '\0';
                    if (strstr(line, pattern)) {
                        print_highlighted_line(line, pattern);
                    }
                    line_pos = 0;
                } else if (line_pos < sizeof(line) - 1) {
                    line[line_pos++] = ch;
                }
            }
            if (line_pos > 0) {
                line[line_pos] = '\0';
                if (strstr(line, pattern)) {
                    print_highlighted_line(line, pattern);
                }
            }
            return 0;
        } else {
            axiom_print("grep: ");
            axiom_print(argv[2]);
            axiom_println(": No such file or directory");
            return 1;
        }
    }

    // Default: read from standard input (Slot 6 / STDIN)
    char line[512];
    while (axiom_getline(line, sizeof(line)) >= 0) {
        if (strstr(line, pattern)) {
            print_highlighted_line(line, pattern);
        }
    }

    return 0;
}
