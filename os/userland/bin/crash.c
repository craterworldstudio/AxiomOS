#include "../include/axiom.h"

int main(int argc, char** argv, CapKeyring* keyring) {
    (void)keyring;

    if (argc > 1 && strcmp(argv[1], "-dh") == 0) {
        axiom_println("  crash                - crash containment diagnostic test utility");
        return 0;
    }

    if (argc < 2 || strcmp(argv[1], "-h") == 0 || strcmp(argv[1], "--help") == 0) {
        axiom_set_color(COLOR_GOLD, 0x00101418);
        axiom_println("Usage: crash <mode>");
        axiom_set_color(COLOR_SILVER, 0x00101418);
        axiom_println("Fault Endpoints & Crash Containment Diagnostic Utility.");
        axiom_println("Modes:");
        axiom_println("  segv       Trigger Page Fault (#14) via NULL pointer dereference");
        axiom_println("  div0       Trigger Divide By Zero (#0) via integer division");
        axiom_println("  ill        Trigger Illegal Instruction (#6) via UD2 opcode");
        axiom_println("  gpf        Trigger General Protection Fault (#13) via privileged CLI");
        axiom_println("  -dh        Show system descriptor");
        axiom_println("  -h, --help Show this manual");
        return 0;
    }

    const char* mode = argv[1];

    if (strcmp(mode, "segv") == 0) {
        axiom_set_color(COLOR_CYAN, 0x00101418);
        axiom_println("[CRASH] Triggering Page Fault (#14): writing to NULL pointer (0x0)...");
        axiom_set_color(COLOR_SILVER, 0x00101418);
        volatile int* null_ptr = (volatile int*)0x0;
        *null_ptr = 0xDEADBEEF;
    } else if (strcmp(mode, "div0") == 0 || strcmp(mode, "div") == 0 || strcmp(mode, "fpe") == 0) {
        axiom_set_color(COLOR_CYAN, 0x00101418);
        axiom_println("[CRASH] Triggering Divide By Zero (#0): dividing 42 by 0...");
        axiom_set_color(COLOR_SILVER, 0x00101418);
        volatile int a = 42;
        volatile int b = 0;
        volatile int c = a / b;
        (void)c;
    } else if (strcmp(mode, "ill") == 0) {
        axiom_set_color(COLOR_CYAN, 0x00101418);
        axiom_println("[CRASH] Triggering Illegal Opcode (#6): executing UD2 instruction...");
        axiom_set_color(COLOR_SILVER, 0x00101418);
        __asm__ volatile ("ud2");
    } else if (strcmp(mode, "gpf") == 0) {
        axiom_set_color(COLOR_CYAN, 0x00101418);
        axiom_println("[CRASH] Triggering General Protection Fault (#13): executing privileged CLI in Ring 3...");
        axiom_set_color(COLOR_SILVER, 0x00101418);
        __asm__ volatile ("cli");
    } else {
        axiom_set_color(0x00FF4B4B, 0x00101418);
        axiom_print("crash: unknown fault mode '");
        axiom_print(mode);
        axiom_println("'. Run 'crash -h' for usage.");
        axiom_set_color(COLOR_SILVER, 0x00101418);
        return 1;
    }

    return 0;
}
