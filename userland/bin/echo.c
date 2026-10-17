#include "../include/axiom.h"

int main(int argc, char** argv, CapKeyring* keyring) {
    (void)keyring;
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
