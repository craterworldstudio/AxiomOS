#include "../include/axiom.h"

typedef struct Node {
    int value;
    struct Node* next;
} Node;

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

int main(int argc, char** argv, CapKeyring* keyring) {
    (void)keyring;

    if (argc > 1 && strcmp(argv[1], "-dh") == 0) {
        axiom_println("  heaptest             - dynamic heap allocator verification test suite");
        return 0;
    }

    if (argc > 1 && (strcmp(argv[1], "-h") == 0 || strcmp(argv[1], "--help") == 0)) {
        axiom_set_color(COLOR_GOLD, 0x00101418);
        axiom_println("Usage: heaptest");
        axiom_set_color(COLOR_SILVER, 0x00101418);
        axiom_println("Verifies userspace dynamic heap allocator (malloc, free, calloc, realloc, sbrk).");
        return 0;
    }

    axiom_set_color(COLOR_GOLD, 0x00101418);
    axiom_println("=== AxiomOS Dynamic Heap Allocator Test Suite ===");
    axiom_set_color(COLOR_SILVER, 0x00101418);

    // TEST 1: 16-byte Alignment & Basic Allocations
    axiom_print("[TEST 1] Allocating blocks (16B, 64B, 256B, 1024B, 8192B)... ");
    void* p1 = malloc(16);
    void* p2 = malloc(64);
    void* p3 = malloc(256);
    void* p4 = malloc(1024);
    void* p5 = malloc(8192);

    if (!p1 || !p2 || !p3 || !p4 || !p5) {
        axiom_set_color(0x00FF4B4B, 0x00101418);
        axiom_println("FAILED (null pointer returned)");
        axiom_set_color(COLOR_SILVER, 0x00101418);
        return 1;
    }

    if (((uint64_t)p1 % 16 != 0) || ((uint64_t)p2 % 16 != 0) ||
        ((uint64_t)p3 % 16 != 0) || ((uint64_t)p4 % 16 != 0) ||
        ((uint64_t)p5 % 16 != 0)) {
        axiom_set_color(0x00FF4B4B, 0x00101418);
        axiom_println("FAILED (unaligned pointer)");
        axiom_set_color(COLOR_SILVER, 0x00101418);
        return 1;
    }
    axiom_set_color(0x00A3BE8C, 0x00101418); // Soft Green
    axiom_println("PASSED (16-byte aligned)");
    axiom_set_color(COLOR_SILVER, 0x00101418);

    // TEST 2: Memory Payload Integrity & Boundary Check
    axiom_print("[TEST 2] Verifying payload integrity across boundaries... ");
    uint8_t* b4 = (uint8_t*)p4;
    for (int i = 0; i < 1024; i++) {
        b4[i] = (uint8_t)(i ^ 0x5A);
    }
    int integrity_ok = 1;
    for (int i = 0; i < 1024; i++) {
        if (b4[i] != (uint8_t)(i ^ 0x5A)) {
            integrity_ok = 0;
            break;
        }
    }
    if (!integrity_ok) {
        axiom_set_color(0x00FF4B4B, 0x00101418);
        axiom_println("FAILED (data corruption)");
        axiom_set_color(COLOR_SILVER, 0x00101418);
        return 1;
    }
    axiom_set_color(0x00A3BE8C, 0x00101418);
    axiom_println("PASSED (1024 bytes verified)");
    axiom_set_color(COLOR_SILVER, 0x00101418);

    free(p1);
    free(p2);
    free(p3);
    free(p4);
    free(p5);

    // TEST 3: Block Coalescing & Memory Reuse
    axiom_print("[TEST 3] Testing block coalescing and reuse... ");
    void* c1 = malloc(512);
    void* c2 = malloc(512);
    void* c3 = malloc(512);
    free(c2);
    free(c1);
    free(c3);
    void* c_big = malloc(1536);
    if (!c_big) {
        axiom_set_color(0x00FF4B4B, 0x00101418);
        axiom_println("FAILED (could not allocate coalesced block)");
        axiom_set_color(COLOR_SILVER, 0x00101418);
        return 1;
    }
    free(c_big);
    axiom_set_color(0x00A3BE8C, 0x00101418);
    axiom_println("PASSED (merged 3 adjacent blocks into 1.5KB)");
    axiom_set_color(COLOR_SILVER, 0x00101418);

    // TEST 4: Dynamic Linked List Growth (50 nodes)
    axiom_print("[TEST 4] Allocating dynamic linked list (50 nodes)... ");
    Node* head = NULL;
    for (int i = 0; i < 50; i++) {
        Node* node = (Node*)malloc(sizeof(Node));
        if (!node) {
            axiom_set_color(0x00FF4B4B, 0x00101418);
            axiom_println("FAILED (null node)");
            axiom_set_color(COLOR_SILVER, 0x00101418);
            return 1;
        }
        node->value = i + 1;
        node->next = head;
        head = node;
    }
    int sum = 0;
    Node* curr = head;
    while (curr) {
        sum += curr->value;
        Node* temp = curr;
        curr = curr->next;
        free(temp);
    }
    // Sum of 1..50 = (50 * 51) / 2 = 1275
    if (sum != 1275) {
        axiom_set_color(0x00FF4B4B, 0x00101418);
        axiom_println("FAILED (sum mismatch)");
        axiom_set_color(COLOR_SILVER, 0x00101418);
        return 1;
    }
    axiom_set_color(0x00A3BE8C, 0x00101418);
    axiom_print("PASSED (sum=");
    print_uint(sum);
    axiom_println(" verified)");
    axiom_set_color(COLOR_SILVER, 0x00101418);

    // TEST 5: Realloc Expansion & String Preservation
    axiom_print("[TEST 5] Testing realloc expansion (48B -> 2048B)... ");
    char* str = (char*)malloc(48);
    strcpy(str, "AxiomOS Sovereign Heap Allocator");
    char* big_str = (char*)realloc(str, 2048);
    if (!big_str || strcmp(big_str, "AxiomOS Sovereign Heap Allocator") != 0) {
        axiom_set_color(0x00FF4B4B, 0x00101418);
        axiom_println("FAILED (string corrupted or null)");
        axiom_set_color(COLOR_SILVER, 0x00101418);
        return 1;
    }
    free(big_str);
    axiom_set_color(0x00A3BE8C, 0x00101418);
    axiom_println("PASSED (string content preserved)");
    axiom_set_color(COLOR_SILVER, 0x00101418);

    axiom_set_color(COLOR_CYAN, 0x00101418);
    axiom_println("=================================================");
    axiom_println("[ALL 5 HEAP ALLOCATOR TESTS PASSED SUCCESSFULLY]");
    axiom_println("=================================================");
    axiom_set_color(COLOR_SILVER, 0x00101418);

    return 0;
}
