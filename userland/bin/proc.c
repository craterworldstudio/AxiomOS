#include "../include/axiom.h"

static void print_uint(uint32_t val) {
    char buf[12];
    int idx = 0;
    if (val == 0) {
        axiom_print("0");
        return;
    }
    while (val > 0) {
        buf[idx++] = '0' + (val % 10);
        val /= 10;
    }
    for (int i = idx - 1; i >= 0; i--) {
        char ch[2] = { buf[i], '\0' };
        axiom_print(ch);
    }
}

static void print_hex64(uint64_t val) {
    axiom_print("0x");
    for (int i = 15; i >= 0; i--) {
        uint8_t nibble = (val >> (i * 4)) & 0xF;
        char c = (nibble < 10) ? ('0' + nibble) : ('a' + (nibble - 10));
        char ch[2] = { c, '\0' };
        axiom_print(ch);
    }
}

static void print_spaces(int count) {
    for (int i = 0; i < count; i++) {
        axiom_print(" ");
    }
}

static int parse_int(const char* s) {
    int res = 0;
    while (*s >= '0' && *s <= '9') {
        res = res * 10 + (*s - '0');
        s++;
    }
    return res;
}

static void show_help(void) {
    axiom_set_color(COLOR_GOLD, 0x00101418);
    axiom_println("Usage: proc [command] [options] (or ps)");
    axiom_set_color(COLOR_SILVER, 0x00101418);
    axiom_println("Sovereign Process Inventory & Task Lifecycle Manager.");
    axiom_println("Commands:");
    axiom_println("  list, ps           - Enumerate active processes across capability graph");
    axiom_println("  kill <pid>         - Terminate or reap target process by PID");
    axiom_println("  clean, reap        - Purge all terminated zombie tasks from table");
    axiom_println("  info <pid>         - Inspect isolated VSpace and execution state of PID");
    axiom_println("Options:");
    axiom_println("  -a, --all          - Show all tasks including kernel boot harnesses");
    axiom_println("  -h, --help         - Show this process reference manual");
    axiom_println("  -dh                - Display embedded system descriptor");
    axiom_println("Note: Use '&' in shell to run any task concurrently in background (e.g. worker 50 &).");
}

static void do_list(int show_all) {
    ProcessInfo list[32];
    int count = axiom_task_list(list, 32);
    if (count <= 0) {
        axiom_println("proc: no active processes found");
        return;
    }

    axiom_set_color(COLOR_GOLD, 0x00101418); // Gold header
    axiom_println("PID   PPID  STATE       VSPACE (CR3)        COMMAND");
    axiom_set_color(COLOR_DIM, 0x00101418);
    axiom_println("------------------------------------------------------------------");

    int shown = 0;
    for (int i = 0; i < count; i++) {
        ProcessInfo* p = &list[i];

        if (!show_all && p->pid == 0) {
            continue; // Filter internal kernel boot harnesses
        }

        // PID
        axiom_set_color(COLOR_CYAN, 0x00101418);
        print_uint(p->pid);
        if (p->pid < 10) print_spaces(5);
        else if (p->pid < 100) print_spaces(4);
        else print_spaces(3);

        // PPID
        axiom_set_color(COLOR_MUTED, 0x00101418);
        print_uint(p->ppid);
        if (p->ppid < 10) print_spaces(5);
        else if (p->ppid < 100) print_spaces(4);
        else print_spaces(3);

        // STATE badge
        switch (p->state) {
            case 0: // Ready
                axiom_set_color(COLOR_SILVER, 0x00101418);
                axiom_print("READY      ");
                break;
            case 1: // Running
                axiom_set_color(COLOR_EMERALD, 0x00101418);
                axiom_print("RUNNING    ");
                break;
            case 2: // BlockedSend
                axiom_set_color(COLOR_MUTED, 0x00101418);
                axiom_print("BLK_SND    ");
                break;
            case 3: // BlockedReceive
                axiom_set_color(COLOR_MUTED, 0x00101418);
                axiom_print("BLK_RCV    ");
                break;
            case 4: // Suspended
                axiom_set_color(COLOR_MUTED, 0x00101418);
                axiom_print("SUSPEND    ");
                break;
            case 5: // Terminated
                axiom_set_color(COLOR_CRIMSON, 0x00101418);
                axiom_print("ZOMBIE     ");
                break;
            default:
                axiom_set_color(COLOR_MUTED, 0x00101418);
                axiom_print("UNKNOWN    ");
                break;
        }

        // CR3
        axiom_set_color(COLOR_MUTED, 0x00101418);
        print_hex64(p->cr3);
        print_spaces(2);

        // COMMAND
        axiom_set_color(COLOR_SILVER, 0x00101418);
        if (p->name[0] != '\0') {
            axiom_println(p->name);
        } else {
            axiom_println("[kernel_task]");
        }
        shown++;
    }

    axiom_set_color(COLOR_MUTED, 0x00101418);
    axiom_println("------------------------------------------------------------------");
    print_uint((uint32_t)shown);
    axiom_println(" task(s) in capability graph (Zero Ambient Root)");
}

static void do_kill(uint32_t pid) {
    if (pid <= 1) {
        axiom_set_color(COLOR_CRIMSON, 0x00101418);
        axiom_println("proc: cannot terminate PID 1: sovereign root genesis task is protected");
        return;
    }

    ProcessInfo list[32];
    int count = axiom_task_list(list, 32);
    int was_zombie = 0;
    for (int i = 0; i < count; i++) {
        if (list[i].pid == pid && list[i].state == 5) {
            was_zombie = 1;
            break;
        }
    }

    int res = axiom_task_kill(pid);
    if (res == 0) {
        axiom_set_color(COLOR_EMERALD, 0x00101418);
        if (was_zombie) {
            axiom_print("[PROC] Purged zombie process PID ");
            print_uint(pid);
            axiom_println(" from capability table");
        } else {
            axiom_print("[PROC] Terminated process PID ");
            print_uint(pid);
            axiom_println(" (SIGKILL delivered)");
        }
    } else {
        axiom_set_color(COLOR_CRIMSON, 0x00101418);
        axiom_print("proc: failed to terminate PID ");
        print_uint(pid);
        axiom_println(" (process not found or invalid capability)");
    }
}

static void do_clean(void) {
    ProcessInfo list[32];
    int count = axiom_task_list(list, 32);
    int cleaned = 0;
    for (int i = 0; i < count; i++) {
        if (list[i].pid > 1 && list[i].state == 5) {
            if (axiom_task_kill(list[i].pid) == 0) {
                cleaned++;
            }
        }
    }
    axiom_set_color(COLOR_EMERALD, 0x00101418);
    axiom_print("[PROC] Purged ");
    print_uint((uint32_t)cleaned);
    axiom_println(" zombie task(s) from capability table.");
}

static void do_info(uint32_t pid) {
    ProcessInfo list[32];
    int count = axiom_task_list(list, 32);
    for (int i = 0; i < count; i++) {
        if (list[i].pid == pid) {
            ProcessInfo* p = &list[i];
            axiom_set_color(COLOR_GOLD, 0x00101418);
            axiom_println("--- Process Diagnostics ---");
            axiom_set_color(COLOR_SILVER, 0x00101418);
            axiom_print("PID:          "); print_uint(p->pid); axiom_println("");
            axiom_print("Parent PID:   "); print_uint(p->ppid); axiom_println("");
            axiom_print("Command:      "); 
            if (p->name[0] != '\0') axiom_println(p->name); else axiom_println("[kernel_task]");
            axiom_print("VSpace (CR3): "); print_hex64(p->cr3); axiom_println("");
            axiom_print("State:        ");
            switch (p->state) {
                case 0: axiom_println("READY"); break;
                case 1: axiom_println("RUNNING"); break;
                case 2: axiom_println("BLOCKED_SEND"); break;
                case 3: axiom_println("BLOCKED_RECEIVE"); break;
                case 4: axiom_println("SUSPENDED"); break;
                case 5: axiom_println("TERMINATED / ZOMBIE"); break;
                default: axiom_println("UNKNOWN"); break;
            }
            return;
        }
    }

    axiom_set_color(COLOR_CRIMSON, 0x00101418);
    axiom_print("proc: process PID ");
    print_uint(pid);
    axiom_println(" not found");
}

int main(int argc, char** argv, CapKeyring* keyring) {
    (void)keyring;

    if (argc > 1 && strcmp(argv[1], "-dh") == 0) {
        axiom_println("  proc                 - process inventory and task lifecycle management suite");
        return 0;
    }

    if (argc > 1 && (strcmp(argv[1], "-h") == 0 || strcmp(argv[1], "--help") == 0)) {
        show_help();
        return 0;
    }

    int show_all = 0;
    if (argc > 1 && (strcmp(argv[1], "-a") == 0 || strcmp(argv[1], "--all") == 0)) {
        show_all = 1;
    }
    if (argc > 2 && (strcmp(argv[2], "-a") == 0 || strcmp(argv[2], "--all") == 0)) {
        show_all = 1;
    }

    if (argc == 1 || strcmp(argv[1], "list") == 0 || strcmp(argv[1], "ps") == 0 || show_all) {
        do_list(show_all);
        return 0;
    }

    if (strcmp(argv[1], "clean") == 0 || strcmp(argv[1], "reap") == 0) {
        do_clean();
        return 0;
    }

    if (strcmp(argv[1], "kill") == 0) {
        if (argc < 3) {
            axiom_set_color(COLOR_CRIMSON, 0x00101418);
            axiom_println("Usage: proc kill <pid>");
            return 1;
        }
        uint32_t pid = (uint32_t)parse_int(argv[2]);
        do_kill(pid);
        return 0;
    }

    if (strcmp(argv[1], "info") == 0) {
        if (argc < 3) {
            axiom_set_color(COLOR_CRIMSON, 0x00101418);
            axiom_println("Usage: proc info <pid>");
            return 1;
        }
        uint32_t pid = (uint32_t)parse_int(argv[2]);
        do_info(pid);
        return 0;
    }

    axiom_set_color(COLOR_CRIMSON, 0x00101418);
    axiom_print("proc: unknown command: ");
    axiom_println(argv[1]);
    axiom_set_color(COLOR_SILVER, 0x00101418);
    axiom_println("Run 'proc --help' for usage.");
    return 1;
}
