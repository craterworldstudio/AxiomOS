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

char* strcat(char* dest, const char* src) {
    char* d = dest;
    while (*d) d++;
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

void axiom_print_len(const char* str, size_t len) {
    if (!str || len == 0) return;
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

void axiom_print(const char* str) {
    if (!str) return;
    axiom_print_len(str, strlen(str));
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
    if (res != 0 && res <= 255) {
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

void axiom_set_cursor(uint32_t row, uint32_t col) {
    axiom_syscall(ROOT_SLOT_CONSOLE, OP_CONSOLE_SET_CURSOR, row, col);
}

int axiom_storage_read(uint32_t lba, void* buf) {
    uint64_t res = axiom_syscall(ROOT_SLOT_STORAGE, OP_STORAGE_READ, (uint64_t)lba, (uint64_t)buf);
    return (res == 0) ? 0 : -1;
}

int axiom_storage_write(uint32_t lba, const void* buf) {
    uint64_t res = axiom_syscall(ROOT_SLOT_STORAGE, OP_STORAGE_WRITE, (uint64_t)lba, (uint64_t)buf);
    return (res == 0) ? 0 : -1;
}

int axiom_task_list(ProcessInfo* list, uint32_t max_count) {
    uint64_t res = axiom_syscall(ROOT_SLOT_TCB, OP_TASK_LIST, (uint64_t)list, (uint64_t)max_count);
    return (int)res;
}

int axiom_task_kill(uint32_t pid) {
    uint64_t res = axiom_syscall(ROOT_SLOT_TCB, OP_TASK_KILL, (uint64_t)pid, 9);
    return (res == 0) ? 0 : -1;
}

int axiom_read(char* buf, size_t count) {
    if (!buf || count == 0) return 0;
    uint64_t res = axiom_syscall(ROOT_SLOT_STDIN, OP_CHANNEL_READ, (uint64_t)buf, count);
    return (int)res;
}

int axiom_getline(char* buf, size_t max_len) {
    if (!buf || max_len <= 1) return -1;
    size_t i = 0;
    while (i < max_len - 1) {
        char ch;
        int n = axiom_read(&ch, 1);
        if (n <= 0) {
            if (i == 0) return -1;
            break;
        }
        if (ch == '\r') continue;
        if (ch == '\n') break;
        buf[i++] = ch;
    }
    buf[i] = '\0';
    return (int)i;
}

int axiom_channel_create(uint64_t* read_cap, uint64_t* write_cap) {
    uint64_t handles[2] = {0, 0};
    uint64_t res = axiom_syscall(ROOT_SLOT_CNODE, OP_CHANNEL_CREATE, (uint64_t)handles, 0);
    if (res == 0) {
        if (read_cap) *read_cap = handles[0];
        if (write_cap) *write_cap = handles[1];
        return 0;
    }
    return -1;
}

char* strstr(const char* haystack, const char* needle) {
    if (!needle || !*needle) return (char*)haystack;
    for (; *haystack; haystack++) {
        if (*haystack == *needle) {
            const char* h = haystack;
            const char* n = needle;
            while (*h && *n && *h == *n) {
                h++;
                n++;
            }
            if (!*n) return (char*)haystack;
        }
    }
    return NULL;
}

void* memset(void* s, int c, size_t n) {
    unsigned char* p = (unsigned char*)s;
    unsigned char val = (unsigned char)c;
    for (size_t i = 0; i < n; i++) {
        p[i] = val;
    }
    return s;
}

void* memcpy(void* dest, const void* src, size_t n) {
    unsigned char* d = (unsigned char*)dest;
    const unsigned char* s = (const unsigned char*)src;
    for (size_t i = 0; i < n; i++) {
        d[i] = s[i];
    }
    return dest;
}

void* axiom_sbrk(intptr_t increment) {
    uint64_t res = axiom_syscall(0, OP_MEM_EXPAND, (uint64_t)increment, 0);
    if (res == 0) {
        return (void*)-1;
    }
    return (void*)res;
}

typedef struct BlockHeader {
    size_t size;               // Data payload size
    int is_free;               // 1 if free, 0 if allocated
    int _pad;                  // 16-byte alignment padding
    struct BlockHeader* next;  // Next block
    struct BlockHeader* prev;  // Prev block
} BlockHeader;

#define ALIGN16(x) (((x) + 15) & ~15)
#define BLOCK_HEADER_SIZE ALIGN16(sizeof(BlockHeader))
#define MIN_CHUNK_SIZE (64 * 1024)

static BlockHeader* heap_head = NULL;

void* malloc(size_t size) {
    if (size == 0) return NULL;
    size = ALIGN16(size);

    // 1. First-fit search across existing blocks
    BlockHeader* curr = heap_head;
    while (curr) {
        if (curr->is_free && curr->size >= size) {
            if (curr->size >= size + BLOCK_HEADER_SIZE + 16) {
                size_t remaining = curr->size - size - BLOCK_HEADER_SIZE;
                BlockHeader* split = (BlockHeader*)((uint8_t*)(curr + 1) + size);
                split->size = remaining;
                split->is_free = 1;
                split->next = curr->next;
                split->prev = curr;
                if (curr->next) {
                    curr->next->prev = split;
                }
                curr->next = split;
                curr->size = size;
            }
            curr->is_free = 0;
            return (void*)(curr + 1);
        }
        curr = curr->next;
    }

    // 2. Request new memory chunk from microkernel
    size_t total_needed = size + BLOCK_HEADER_SIZE;
    size_t chunk_size = total_needed;
    if (chunk_size < MIN_CHUNK_SIZE) {
        chunk_size = MIN_CHUNK_SIZE;
    }
    chunk_size = (chunk_size + 4095) & ~4095;

    void* raw_mem = axiom_sbrk((intptr_t)chunk_size);
    if (raw_mem == (void*)-1) {
        return NULL;
    }

    BlockHeader* new_block = (BlockHeader*)raw_mem;
    new_block->size = chunk_size - BLOCK_HEADER_SIZE;
    new_block->is_free = 1;
    new_block->next = NULL;
    new_block->prev = NULL;

    if (!heap_head) {
        heap_head = new_block;
    } else {
        BlockHeader* last = heap_head;
        while (last->next) {
            last = last->next;
        }
        last->next = new_block;
        new_block->prev = last;

        if (last->is_free && ((uint8_t*)(last + 1) + last->size == (uint8_t*)new_block)) {
            last->size += BLOCK_HEADER_SIZE + new_block->size;
            last->next = NULL;
            new_block = last;
        }
    }

    if (new_block->size >= size + BLOCK_HEADER_SIZE + 16) {
        size_t remaining = new_block->size - size - BLOCK_HEADER_SIZE;
        BlockHeader* split = (BlockHeader*)((uint8_t*)(new_block + 1) + size);
        split->size = remaining;
        split->is_free = 1;
        split->next = new_block->next;
        split->prev = new_block;
        if (new_block->next) {
            new_block->next->prev = split;
        }
        new_block->next = split;
        new_block->size = size;
    }

    new_block->is_free = 0;
    return (void*)(new_block + 1);
}

void free(void* ptr) {
    if (!ptr) return;

    BlockHeader* block = ((BlockHeader*)ptr) - 1;
    block->is_free = 1;

    // Forward merge
    if (block->next && block->next->is_free) {
        block->size += BLOCK_HEADER_SIZE + block->next->size;
        block->next = block->next->next;
        if (block->next) {
            block->next->prev = block;
        }
    }

    // Backward merge
    if (block->prev && block->prev->is_free) {
        block->prev->size += BLOCK_HEADER_SIZE + block->size;
        block->prev->next = block->next;
        if (block->next) {
            block->next->prev = block->prev;
        }
    }
}

void* calloc(size_t nmemb, size_t size) {
    if (nmemb == 0 || size == 0) return NULL;
    size_t total = nmemb * size;
    if (total / nmemb != size) return NULL;
    void* ptr = malloc(total);
    if (ptr) {
        memset(ptr, 0, total);
    }
    return ptr;
}

void* realloc(void* ptr, size_t size) {
    if (!ptr) return malloc(size);
    if (size == 0) {
        free(ptr);
        return NULL;
    }

    size = ALIGN16(size);
    BlockHeader* block = ((BlockHeader*)ptr) - 1;
    if (block->size >= size) {
        return ptr;
    }

    if (block->next && block->next->is_free) {
        size_t combined = block->size + BLOCK_HEADER_SIZE + block->next->size;
        if (combined >= size) {
            block->next = block->next->next;
            if (block->next) {
                block->next->prev = block;
            }
            block->size = combined;
            return ptr;
        }
    }

    void* new_ptr = malloc(size);
    if (!new_ptr) return NULL;
    size_t copy_len = (block->size < size) ? block->size : size;
    memcpy(new_ptr, ptr, copy_len);
    free(ptr);
    return new_ptr;
}

