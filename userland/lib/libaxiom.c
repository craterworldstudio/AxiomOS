#include "../include/axiom.h"

size_t strlen(const char* s) {
    size_t len = 0;
    while (s && s[len]) {
        len++;
    }
    return len;
}

int strcmp(const char* s1, const char* s2) {
    while (*s1 && (*s1 == *s2)) {
        s1++;
        s2++;
    }
    return *(const unsigned char*)s1 - *(const unsigned char*)s2;
}

int strncmp(const char* s1, const char* s2, size_t n) {
    for (size_t i = 0; i < n; i++) {
        if (s1[i] != s2[i] || s1[i] == '\0') {
            return (unsigned char)s1[i] - (unsigned char)s2[i];
        }
    }
    return 0;
}

char* strcpy(char* dest, const char* src) {
    char* d = dest;
    while ((*d++ = *src++));
    return dest;
}

char* strncpy(char* dest, const char* src, size_t n) {
    size_t i;
    for (i = 0; i < n && src[i] != '\0'; i++)
        dest[i] = src[i];
    for (; i < n; i++)
        dest[i] = '\0';
    return dest;
}

const char* strchr(const char* s, int c) {
    while (*s) {
        if (*s == (char)c) return s;
        s++;
    }
    return (c == 0) ? s : NULL;
}

void axiom_print(const char* str) {
    if (!str) return;
    size_t len = strlen(str);
    size_t i = 0;
    while (i < len) {
        size_t chunk_len = len - i;
        if (chunk_len > 8) chunk_len = 8;
        uint64_t chunk = 0;
        for (size_t j = 0; j < chunk_len; j++) {
            chunk |= ((uint64_t)(uint8_t)str[i + j]) << (j * 8);
        }
        axiom_syscall(ROOT_SLOT_CONSOLE, OP_CONSOLE_WRITE, chunk, chunk_len);
        i += chunk_len;
    }
}

void axiom_println(const char* str) {
    if (str) {
        axiom_print(str);
    }
    axiom_print("\n");
}

void axiom_set_color(uint32_t fg, uint32_t bg) {
    axiom_syscall(ROOT_SLOT_CONSOLE, OP_CONSOLE_COLOR, fg, bg);
}

int axiom_getchar(void) {
    uint64_t res = axiom_syscall(ROOT_SLOT_CONSOLE, OP_CONSOLE_GETCHAR, 0, 0);
    if (res != 0 && res < 128) {
        return (int)res;
    }
    return -1;
}

void axiom_clear(void) {
    axiom_syscall(ROOT_SLOT_CONSOLE, OP_CONSOLE_CLEAR, 0, 0);
}

void axiom_draw_banner(void) {
    axiom_syscall(ROOT_SLOT_CONSOLE, OP_CONSOLE_BANNER, 0, 0);
}
