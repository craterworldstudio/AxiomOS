#ifndef AXIOM_H
#define AXIOM_H

typedef unsigned long  uint64_t;
typedef unsigned int   uint32_t;
typedef unsigned short uint16_t;
typedef unsigned char  uint8_t;
typedef long           int64_t;
typedef int            int32_t;
typedef unsigned long  size_t;

#define NULL ((void*)0)

// Capability Slots in CSpace
#define ROOT_SLOT_CNODE    0x100000001ULL
#define ROOT_SLOT_VSPACE   0x100000002ULL
#define ROOT_SLOT_TCB      0x100000003ULL
#define ROOT_SLOT_UNTYPED  0x100000004ULL
#define ROOT_SLOT_CONSOLE  0x100000005ULL
#define ROOT_SLOT_STORAGE  0x100000007ULL

// Syscall Operations
#define OP_SEND            0
#define OP_RECEIVE         1
#define OP_CALL            2
#define OP_MINT            3
#define OP_REVOKE          4
#define OP_DELETE          5
#define OP_CONSOLE_WRITE   6
#define OP_CONSOLE_CLEAR   7
#define OP_CONSOLE_BANNER  8
#define OP_CONSOLE_COLOR   9
#define OP_CONSOLE_GETCHAR 10

// Capability Rights Bitmask
#define RIGHTS_NONE  0b0000
#define RIGHTS_READ  0b0001
#define RIGHTS_WRITE 0b0010
#define RIGHTS_EXEC  0b0100
#define RIGHTS_GRANT 0b1000
#define RIGHTS_ALL   0b1111

// Capability Keyring structure passed to user processes
typedef struct {
    uint32_t user_id;
    uint32_t rights;
    uint64_t storage_cap;
    uint64_t console_cap;
    char     user_name[32];
} CapKeyring;

// CapGraph directory entry header matching Genesis layout (56 bytes)
typedef struct {
    char        name[32];
    uint32_t    is_dir;
    uint32_t    data_len;
    uint8_t     rights;
    uint8_t     in_use;
    uint8_t     _pad[6];
    const void* data;
} CapEntryInfo;

// Capability Map Context structure (896 + 4 + 4 pad = 904 bytes)
typedef struct {
    CapEntryInfo entries[16];
    uint32_t     count;
} CapMapContext;

// Capability Blob Context for file content and editor writeback
typedef struct {
    char     data[1024];
    uint32_t length;
    uint32_t modified;
} CapBlobContext;

// Special navigation and control key definitions
#define KEY_UP     128
#define KEY_DOWN   129
#define KEY_LEFT   130
#define KEY_RIGHT  131
#define KEY_DELETE 132
#define KEY_HOME   133
#define KEY_END    134
#define KEY_CTRL_C 3
#define KEY_CTRL_Q 17
#define KEY_CTRL_S 19

// Hardware fast-path syscall gateway
static inline uint64_t axiom_syscall(uint64_t handle, uint64_t op, uint64_t arg1, uint64_t arg2) {
    uint64_t ret;
    register uint64_t r10 asm("r10") = arg2;
    asm volatile (
        "syscall"
        : "=a"(ret), "+D"(handle), "+S"(op), "+d"(arg1), "+r"(r10)
        :
        : "rcx", "r11", "r8", "r9", "memory"
    );
    return ret;
}

// Userland API
void     axiom_print(const char* str);
void     axiom_print_len(const char* str, size_t len);
void     axiom_println(const char* str);
void     axiom_set_color(uint32_t fg, uint32_t bg);
int      axiom_getchar(void);
void     axiom_clear(void);
void     axiom_draw_banner(void);

// Standard string helpers
size_t   strlen(const char* s);
int      strcmp(const char* s1, const char* s2);
int      strncmp(const char* s1, const char* s2, size_t n);
char*    strcpy(char* dest, const char* src);
char*    strncpy(char* dest, const char* src, size_t n);
char*    strcat(char* dest, const char* src);
const char* strchr(const char* s, int c);

#endif
