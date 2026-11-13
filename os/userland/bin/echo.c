#include "../include/axiom.h"

int main(int argc, char** argv, CapKeyring* keyring) {
    (void)keyring;
    if (argc > 1 && (strcmp(argv[1], "-h") == 0 || strcmp(argv[1], "--help") == 0)) {
        axiom_set_color(COLOR_GOLD, 0x00101418);
        axiom_println("Usage: echo [text] [> file] [options]");
        axiom_set_color(COLOR_SILVER, 0x00101418);
        axiom_println("Print text to console or write/redirect into a file blob.");
        axiom_println("Options:");
        axiom_println("  -h, --help    Show this command reference manual");
        return 0;
    }
    if (argc > 1 && strcmp(argv[1], "-dh") == 0) {
        axiom_println("  echo                 - print arguments to console or redirect to file");
        return 0;
    }
    for (int i = 1; i < argc; i++) {
        const char* arg = argv[i];
        size_t len = strlen(arg);
        
        // Strip surrounding double quotes or single quotes
        if (len >= 2 && ((arg[0] == '"' && arg[len - 1] == '"') || (arg[0] == '\'' && arg[len - 1] == '\''))) {
            for (size_t j = 1; j < len - 1; j++) {
                char ch[2] = { arg[j], '\0' };
                axiom_print(ch);
            }
        } else {
            axiom_print(arg);
        }
        
        if (i + 1 < argc) {
            axiom_print(" ");
        }
    }
    axiom_print("\n");
    return 0;
}
