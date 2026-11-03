#include "../include/axiom.h"

static void print_uint(uint32_t val, int min_width) {
    char buf[16];
    int idx = 0;
    if (val == 0) {
        buf[idx++] = '0';
    } else {
        char temp[16];
        int t_idx = 0;
        while (val > 0) {
            temp[t_idx++] = (char)('0' + (val % 10));
            val /= 10;
        }
        while (t_idx > 0) {
            buf[idx++] = temp[--t_idx];
        }
    }
    buf[idx] = '\0';
    while (idx < min_width) {
        axiom_print(" ");
        min_width--;
    }
    axiom_print(buf);
}

static void print_human_size(uint32_t bytes) {
    if (bytes < 1024) {
        print_uint(bytes, 4);
        axiom_print(" B");
    } else if (bytes < 1024 * 1024) {
        uint32_t kb = bytes / 1024;
        print_uint(kb, 4);
        axiom_print(" K");
    } else {
        uint32_t mb = bytes / (1024 * 1024);
        print_uint(mb, 4);
        axiom_print(" M");
    }
}

static void print_size(uint32_t bytes, int human) {
    if (human) {
        print_human_size(bytes);
    } else {
        print_uint(bytes, 7);
        axiom_print(" B");
    }
}

static void print_long_entry(const char* name, uint32_t is_dir, uint32_t data_len, uint8_t rights, int opt_h) {
    // 1. CapObject Kind Badge (MAP, EDP, OBJ)
    if (is_dir == 1) {
        axiom_set_color(COLOR_CYAN, 0x00101418); // Electric Cyan
        axiom_print("MAP  ");
    } else if (is_dir == 2) {
        axiom_set_color(COLOR_GOLD, 0x00101418); // Gold Yellow
        axiom_print("EDP  ");
    } else {
        axiom_set_color(COLOR_SILVER, 0x00101418); // Crisp Silver
        axiom_print("OBJ  ");
    }

    // 2. Sovereign Capability Rights Attenuation Bracket: <RWXG>
    char rights_buf[7];
    rights_buf[0] = '<';
    rights_buf[1] = (rights & RIGHTS_READ)  ? 'R' : '-';
    rights_buf[2] = (rights & RIGHTS_WRITE) ? 'W' : '-';
    rights_buf[3] = (rights & RIGHTS_EXEC)  ? 'X' : '-';
    rights_buf[4] = (rights & RIGHTS_GRANT) ? 'G' : '-';
    rights_buf[5] = '>';
    rights_buf[6] = '\0';

    axiom_set_color(COLOR_EMERALD, 0x00101418); // Sovereign Emerald
    axiom_print(rights_buf);
    axiom_print(" ");

    // 3. Object Extent Size
    axiom_set_color(COLOR_SILVER, 0x00101418);
    print_size(data_len, opt_h);
    axiom_print("   ");

    // 4. Object Name with CapGraph Type Indicator
    if (is_dir == 1) {
        axiom_set_color(COLOR_CYAN, 0x00101418); // Electric Cyan
        axiom_print(name);
        axiom_println("/");
    } else if (is_dir == 2) {
        axiom_set_color(COLOR_GOLD, 0x00101418); // Gold Yellow
        axiom_print(name);
        axiom_println("@");
    } else {
        axiom_set_color(COLOR_SILVER, 0x00101418); // Crisp Silver
        axiom_println(name);
    }
}

int main(int argc, char** argv, CapKeyring* keyring) {
    int opt_l = 0;
    int opt_a = 0;
    int opt_h = 0;
    int opt_help = 0;

    for (int i = 1; i < argc; i++) {
        const char* arg = argv[i];
        if (strcmp(arg, "--help") == 0) {
            opt_help = 1;
        } else if (strcmp(arg, "-dh") == 0) {
            axiom_println("  ls                   - list directory contents");
            return 0;
        } else if (strcmp(arg, "--all") == 0) {
            opt_a = 1;
        } else if (strcmp(arg, "--human") == 0) {
            opt_h = 1;
        } else if (arg[0] == '-' && arg[1] != '\0') {
            for (int j = 1; arg[j] != '\0'; j++) {
                if (arg[j] == 'l') opt_l = 1;
                else if (arg[j] == 'a') opt_a = 1;
                else if (arg[j] == 'h') opt_h = 1;
            }
        }
    }

    if (opt_help || (opt_h && !opt_l && !opt_a)) {
        axiom_set_color(COLOR_GOLD, 0x00101418);
        axiom_println("Usage: ls [options] [path]");
        axiom_set_color(COLOR_SILVER, 0x00101418);
        axiom_println("List capability map entries, file blobs, and hardware endpoints.");
        axiom_println("Options:");
        axiom_println("  -a, --all         Do not ignore entries starting with . (includes . and ..)");
        axiom_println("  -l                Use Sovereign CapObject listing format (<RWXG>, kind, size)");
        axiom_println("  -h, --human       Print human-readable sizes (with -l) or help");
        axiom_println("  --help            Show this command reference manual");
        return 0;
    }

    CapMapContext* map = (CapMapContext*)keyring->storage_cap;
    if (!map) {
        axiom_println("ls: cannot access directory");
        return 1;
    }

    int printed = 0;

    // Synthesize '.' and '..' if -a is enabled
    if (opt_a) {
        if (opt_l) {
            print_long_entry(".", 1, 512, RIGHTS_ALL, opt_h);
            print_long_entry("..", 1, 512, RIGHTS_ALL, opt_h);
        } else {
            axiom_set_color(COLOR_CYAN, 0x00101418);
            axiom_print(".  ..  ");
        }
        printed += 2;
    }

    for (uint32_t i = 0; i < map->count && i < 16; i++) {
        CapEntryInfo* ent = &map->entries[i];
        if (!ent->in_use) continue;
        if (!opt_a && ent->name[0] == '.') continue;

        if (opt_l) {
            print_long_entry(ent->name, ent->is_dir, ent->data_len, ent->rights, opt_h);
        } else {
            if (ent->is_dir == 1) {
                axiom_set_color(COLOR_CYAN, 0x00101418); // Electric Cyan (Directory Map)
                axiom_print(ent->name);
                axiom_print("/  ");
            } else if (ent->is_dir == 2) {
                axiom_set_color(COLOR_GOLD, 0x00101418); // Gold Yellow (Capability Endpoint)
                axiom_print(ent->name);
                axiom_print("@  ");
            } else {
                axiom_set_color(COLOR_SILVER, 0x00101418); // Crisp Silver (File Blob)
                axiom_print(ent->name);
                axiom_print("  ");
            }
        }
        printed++;
    }

    if (!opt_l) {
        if (printed > 0) {
            axiom_println("");
        } else {
            axiom_println("(empty)");
        }
    } else if (printed == 0) {
        axiom_println("(empty)");
    }

    axiom_set_color(COLOR_SILVER, 0x00101418);
    return 0;
}
