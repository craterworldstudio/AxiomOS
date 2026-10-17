#include "../include/axiom.h"

int main(int argc, char** argv, CapKeyring* keyring) {
    if (argc > 1 && strcmp(argv[1], "-dh") == 0) {
        axiom_println("  cat                  - print file contents");
        return 0;
    }
    if (argc < 2) {
        axiom_println("cat: missing file operand");
        return 1;
    }
    
    const char* content = (const char*)keyring->storage_cap;
    if (content && *content) {
        axiom_print(content);
        size_t len = strlen(content);
        if (len > 0 && content[len - 1] != '\n') {
            axiom_println("");
        }
        return 0;
    }
    
    axiom_print("cat: ");
    axiom_print(argv[1]);
    axiom_println(": No such file or directory in capability graph");
    return 1;
}
