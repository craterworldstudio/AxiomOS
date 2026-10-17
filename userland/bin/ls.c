#include "../include/axiom.h"

int main(int argc, char** argv, CapKeyring* keyring) {
    if (argc > 1 && strcmp(argv[1], "-dh") == 0) {
        axiom_println("  ls                   - list directory contents");
        return 0;
    }
    
    CapMapContext* map = (CapMapContext*)keyring->storage_cap;
    if (!map) {
        axiom_println("ls: cannot access directory");
        return 1;
    }
    
    int printed = 0;
    for (uint32_t i = 0; i < map->count && i < 16; i++) {
        CapEntryInfo* ent = &map->entries[i];
        if (!ent->in_use) continue;
        
        if (ent->is_dir) {
            axiom_set_color(0x0000D2FF, 0x00101418); // Electric Cyan
            axiom_print(ent->name);
            axiom_print("/  ");
        } else {
            axiom_set_color(0x00E0E0E0, 0x00101418); // Crisp Silver
            axiom_print(ent->name);
            axiom_print("  ");
        }
        printed++;
    }
    
    if (printed > 0) {
        axiom_println("");
    } else {
        axiom_println("(empty)");
    }
    
    axiom_set_color(0x00E0E0E0, 0x00101418);
    return 0;
}
