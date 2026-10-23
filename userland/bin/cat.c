#include "../include/axiom.h"

int main(int argc, char** argv, CapKeyring* keyring) {
    if (argc > 1 && (strcmp(argv[1], "-h") == 0 || strcmp(argv[1], "--help") == 0)) {
        axiom_set_color(COLOR_GOLD, 0x00101418);
        axiom_println("Usage: cat <file> [options]");
        axiom_set_color(COLOR_SILVER, 0x00101418);
        axiom_println("Concatenate and display capability file contents or endpoint specs.");
        axiom_println("Options:");
        axiom_println("  -h, --help    Show this command reference manual");
        return 0;
    }
    if (argc > 1 && strcmp(argv[1], "-dh") == 0) {
        axiom_println("  cat                  - print file contents");
        return 0;
    }
    if (argc < 2) {
        axiom_println("cat: missing file operand");
        return 1;
    }
    
    CapBlobContext* blob = (CapBlobContext*)keyring->storage_cap;
    if (blob && blob->length > 0) {
        axiom_print_len(blob->data, blob->length);
        if (blob->data[blob->length - 1] != '\n') {
            axiom_println("");
        }
        return 0;
    }
    
    axiom_print("cat: ");
    axiom_print(argv[1]);
    axiom_println(": No such file or directory");
    return 1;
}
