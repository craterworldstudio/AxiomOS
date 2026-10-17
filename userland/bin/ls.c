#include "../include/axiom.h"

// CapGraph directory entry header matching userland layout
typedef struct {
    char     name[32];
    uint32_t is_dir;
    uint32_t data_len;
    uint8_t  rights;
    uint8_t  in_use;
} CapEntryInfo;

typedef struct {
    CapEntryInfo entries[16];
    uint32_t     count;
} CapMapContext;

int main(int argc, char** argv, CapKeyring* keyring) {
    (void)argc;
    (void)argv;
    
    CapMapContext* map = (CapMapContext*)keyring->storage_cap;
    if (!map) {
        axiom_println("ls: capability map unavailable");
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
        axiom_println("(empty capability map)");
    }
    
    axiom_set_color(0x00E0E0E0, 0x00101418);
    return 0;
}
