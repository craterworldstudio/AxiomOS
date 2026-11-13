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

static int parse_int(const char* s) {
    int res = 0;
    while (*s >= '0' && *s <= '9') {
        res = res * 10 + (*s - '0');
        s++;
    }
    return res;
}

int main(int argc, char** argv, CapKeyring* keyring) {
    (void)keyring;

    if (argc > 1 && strcmp(argv[1], "-dh") == 0) {
        axiom_println("  worker               - background worker workload daemon");
        return 0;
    }

    if (argc > 1 && (strcmp(argv[1], "-h") == 0 || strcmp(argv[1], "--help") == 0)) {
        axiom_set_color(COLOR_GOLD, 0x00101418);
        axiom_println("Usage: worker [iterations] [options]");
        axiom_set_color(COLOR_SILVER, 0x00101418);
        axiom_println("Background worker daemon demonstrating concurrent execution.");
        axiom_println("Options:");
        axiom_println("  -h, --help    Show this reference manual");
        axiom_println("  -dh           Show system descriptor");
        return 0;
    }

    uint32_t max_iters = 30;
    if (argc > 1) {
        int parsed = parse_int(argv[1]);
        if (parsed > 0) {
            max_iters = (uint32_t)parsed;
        }
    }

    axiom_set_color(COLOR_CYAN, 0x00101418);
    axiom_print("[WORKER] Background daemon started (");
    print_uint(max_iters);
    axiom_println(" iterations). Running concurrently...");

    for (uint32_t i = 1; i <= max_iters; i++) {
        // Sleep/delay loop yielding CPU to simulate realistic workload
        for (volatile uint64_t w = 0; w < 40000000ULL; w++) {
            asm volatile ("pause");
        }

        if (i % 10 == 0 || i == max_iters) {
            axiom_set_color(COLOR_EMERALD, 0x00101418);
            axiom_print("[WORKER] Heartbeat: iteration ");
            print_uint(i);
            axiom_print("/");
            print_uint(max_iters);
            axiom_println(" completed.");
        }
    }

    axiom_set_color(COLOR_CYAN, 0x00101418);
    axiom_println("[WORKER] Workload finished cleanly. Exiting.");
    return 0;
}
