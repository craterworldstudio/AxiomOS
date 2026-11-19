#include "../include/axiom.h"
// ----------------------------------------------------------------------------
// Sovereign C Compiler & Bytecode Virtual Machine (crun)
// Compiles C source code on-device into bytecode and executes it in Ring 3
// ----------------------------------------------------------------------------

// Virtual Machine Instructions
enum {
    LEA, IMM, JMP, JZ, JNZ, CALL, NATIVE, ENT, ADJ, LEV,
    LI, LC, SI, SC, PUSH,
    OR, XOR, AND, EQ, NE, LT, GT, LE, GE, SHL, SHR,
    ADD, SUB, MUL, DIV, MOD,
    EXIT
};

// Tokens
enum {
    Num = 128, Id, Str, CharLit,
    Char, Int, Void, If, Else, While, For, Return, Sizeof
};

// Types
enum { CHAR, INT, PTR };

// Symbol Table Entry
typedef struct {
    int token;
    int hash;
    char name[32];
    int class; // Local, Global, Sys
    int type;  // CHAR, INT, PTR
    intptr_t val; // address or value
} Symbol;

#define MAX_SYMS 512
static Symbol symtab[MAX_SYMS];
static int sym_count = 0;

// Compiler State
static const char* src_ptr;
static int token;
static intptr_t token_val;
static char token_str[128];
static int line_num = 1;

#define MAX_TEXT 8192
#define MAX_DATA 8192
#define MAX_STACK 4096

static int64_t text[MAX_TEXT];
static char    data[MAX_DATA];
static int64_t stack[MAX_STACK];

static int text_idx = 0;
static int data_idx = 0;

static int loc_offset = 0; // Local variable stack frame offset
static int expr_type = 0;  // Result type of current expression

// Forward declarations
static void statement(void);
static void expr(int level);
static void next_tok(void);

static void error(const char* msg) {
    axiom_set_color(COLOR_CRIMSON, COLOR_BG);
    axiom_print("crun: line ");
    char num_buf[16];
    int n = line_num;
    int idx = 0;
    if (n == 0) { num_buf[idx++] = '0'; }
    else {
        char tmp[16]; int ti = 0;
        while (n > 0) { tmp[ti++] = '0' + (n % 10); n /= 10; }
        for (int i = ti - 1; i >= 0; i--) num_buf[idx++] = tmp[i];
    }
    num_buf[idx] = '\0';
    axiom_print(num_buf);
    axiom_print(": error: ");
    axiom_println(msg);
    axiom_set_color(COLOR_SILVER, COLOR_BG);
    axiom_exit(1);
}

static Symbol* lookup_symbol(const char* name) {
    for (int i = 0; i < sym_count; i++) {
        if (strcmp(symtab[i].name, name) == 0) {
            return &symtab[i];
        }
    }
    if (sym_count < MAX_SYMS) {
        Symbol* s = &symtab[sym_count++];
        memset(s, 0, sizeof(Symbol));
        strncpy(s->name, name, 31);
        s->token = Id;
        return s;
    }
    error("symbol table overflow");
    return NULL;
}

static void emit(int64_t op) {
    if (text_idx >= MAX_TEXT) error("code buffer overflow");
    text[text_idx++] = op;
}

static void next_tok(void) {
    while (*src_ptr == ' ' || *src_ptr == '\t' || *src_ptr == '\r' || *src_ptr == '\n') {
        if (*src_ptr == '\n') line_num++;
        src_ptr++;
    }

    if (*src_ptr == '\0') {
        token = 0;
        return;
    }

    // Skip single-line comments
    if (src_ptr[0] == '/' && src_ptr[1] == '/') {
        while (*src_ptr && *src_ptr != '\n') src_ptr++;
        next_tok();
        return;
    }

    // Skip multi-line comments
    if (src_ptr[0] == '/' && src_ptr[1] == '*') {
        src_ptr += 2;
        while (*src_ptr && !(src_ptr[0] == '*' && src_ptr[1] == '/')) {
            if (*src_ptr == '\n') line_num++;
            src_ptr++;
        }
        if (*src_ptr) src_ptr += 2;
        next_tok();
        return;
    }

    // Identifiers and Keywords
    if ((*src_ptr >= 'a' && *src_ptr <= 'z') || (*src_ptr >= 'A' && *src_ptr <= 'Z') || *src_ptr == '_') {
        int i = 0;
        while ((*src_ptr >= 'a' && *src_ptr <= 'z') || (*src_ptr >= 'A' && *src_ptr <= 'Z') ||
               (*src_ptr >= '0' && *src_ptr <= '9') || *src_ptr == '_') {
            if (i < 127) token_str[i++] = *src_ptr;
            src_ptr++;
        }
        token_str[i] = '\0';

        if (strcmp(token_str, "int") == 0)    { token = Int; return; }
        if (strcmp(token_str, "char") == 0)   { token = Char; return; }
        if (strcmp(token_str, "void") == 0)   { token = Void; return; }
        if (strcmp(token_str, "if") == 0)     { token = If; return; }
        if (strcmp(token_str, "else") == 0)   { token = Else; return; }
        if (strcmp(token_str, "while") == 0)  { token = While; return; }
        if (strcmp(token_str, "for") == 0)    { token = For; return; }
        if (strcmp(token_str, "return") == 0) { token = Return; return; }
        if (strcmp(token_str, "sizeof") == 0) { token = Sizeof; return; }

        token = Id;
        return;
    }

    // Numbers
    if (*src_ptr >= '0' && *src_ptr <= '9') {
        int val = 0;
        if (src_ptr[0] == '0' && (src_ptr[1] == 'x' || src_ptr[1] == 'X')) {
            src_ptr += 2;
            while (1) {
                if (*src_ptr >= '0' && *src_ptr <= '9') val = val * 16 + (*src_ptr - '0');
                else if (*src_ptr >= 'a' && *src_ptr <= 'f') val = val * 16 + (*src_ptr - 'a' + 10);
                else if (*src_ptr >= 'A' && *src_ptr <= 'F') val = val * 16 + (*src_ptr - 'A' + 10);
                else break;
                src_ptr++;
            }
        } else {
            while (*src_ptr >= '0' && *src_ptr <= '9') {
                val = val * 10 + (*src_ptr - '0');
                src_ptr++;
            }
        }
        token = Num;
        token_val = val;
        return;
    }

    // String literals
    if (*src_ptr == '"') {
        src_ptr++;
        int start = data_idx;
        while (*src_ptr && *src_ptr != '"') {
            char ch = *src_ptr++;
            if (ch == '\\' && *src_ptr) {
                char esc = *src_ptr++;
                if (esc == 'n') ch = '\n';
                else if (esc == 't') ch = '\t';
                else if (esc == 'r') ch = '\r';
                else if (esc == '0') ch = '\0';
                else ch = esc;
            }
            if (data_idx < MAX_DATA - 1) data[data_idx++] = ch;
        }
        if (*src_ptr == '"') src_ptr++;
        data[data_idx++] = '\0';
        token = Str;
        token_val = (intptr_t)&data[start];
        return;
    }

    // Character literals
    if (*src_ptr == '\'') {
        src_ptr++;
        char ch = *src_ptr++;
        if (ch == '\\' && *src_ptr) {
            char esc = *src_ptr++;
            if (esc == 'n') ch = '\n';
            else if (esc == 't') ch = '\t';
            else if (esc == 'r') ch = '\r';
            else if (esc == '0') ch = '\0';
            else ch = esc;
        }
        if (*src_ptr == '\'') src_ptr++;
        token = Num;
        token_val = (int)ch;
        return;
    }

    // Two-character operators
    if (src_ptr[0] == '=' && src_ptr[1] == '=') { src_ptr += 2; token = EQ; return; }
    if (src_ptr[0] == '!' && src_ptr[1] == '=') { src_ptr += 2; token = NE; return; }
    if (src_ptr[0] == '<' && src_ptr[1] == '=') { src_ptr += 2; token = LE; return; }
    if (src_ptr[0] == '>' && src_ptr[1] == '=') { src_ptr += 2; token = GE; return; }
    if (src_ptr[0] == '<' && src_ptr[1] == '<') { src_ptr += 2; token = SHL; return; }
    if (src_ptr[0] == '>' && src_ptr[1] == '>') { src_ptr += 2; token = SHR; return; }
    if (src_ptr[0] == '&' && src_ptr[1] == '&') { src_ptr += 2; token = AND; return; }
    if (src_ptr[0] == '|' && src_ptr[1] == '|') { src_ptr += 2; token = OR; return; }

    token = *src_ptr++;
}

static void match(int expected) {
    if (token == expected) {
        next_tok();
    } else {
        error("syntax error: unexpected token");
    }
}

enum {
    NAT_PRINTF = 1,
    NAT_MALLOC,
    NAT_FREE,
    NAT_EXIT,
    NAT_PRINT,
    NAT_PRINTLN,
    NAT_STRCPY
};

static void factor(void) {
    if (token == Num) {
        emit(IMM);
        emit(token_val);
        expr_type = INT;
        next_tok();
    } else if (token == Str) {
        emit(IMM);
        emit(token_val);
        expr_type = PTR;
        next_tok();
    } else if (token == Sizeof) {
        next_tok();
        match('(');
        int sz = 8;
        if (token == Int) sz = 8;
        else if (token == Char) sz = 1;
        next_tok();
        while (token == '*') { sz = 8; next_tok(); }
        match(')');
        emit(IMM);
        emit(sz);
        expr_type = INT;
    } else if (token == Id) {
        char name[32];
        strncpy(name, token_str, 31);
        Symbol* s = lookup_symbol(name);
        next_tok();

        // Function call
        if (token == '(') {
            match('(');
            int arg_count = 0;
            while (token != ')') {
                expr(0);
                emit(PUSH);
                arg_count++;
                if (token == ',') next_tok();
            }
            match(')');

            if (s->class == 100) { // Native function
                emit(NATIVE);
                emit(s->val);
                emit(ADJ);
                emit(arg_count);
            } else if (s->class == 2) { // User function
                emit(CALL);
                emit(s->val);
                emit(ADJ);
                emit(arg_count);
            } else {
                error("call to undeclared function");
            }
            expr_type = INT;
        } else {
            // Variable reference (Local or Global)
            if (s->class == 1) { // Local variable / parameter
                emit(LEA);
                emit(s->val);
            } else if (s->class == 3) { // Global variable
                emit(IMM);
                emit((intptr_t)&data[s->val]);
            } else {
                error("undefined variable");
            }

            expr_type = s->type;
            if (expr_type == CHAR) emit(LC);
            else emit(LI);
        }
    } else if (token == '(') {
        next_tok();
        expr(0);
        match(')');
    } else if (token == '*') { // Dereference pointer
        next_tok();
        factor();
        expr_type = INT;
        emit(LI);
    } else if (token == '&') { // Address of
        next_tok();
        if (token != Id) error("lvalue required for '&'");
        Symbol* s = lookup_symbol(token_str);
        next_tok();
        if (s->class == 1) {
            emit(LEA);
            emit(s->val);
        } else if (s->class == 3) {
            emit(IMM);
            emit((intptr_t)&data[s->val]);
        } else {
            error("cannot take address of non-variable");
        }
        expr_type = PTR;
    } else if (token == '!') {
        next_tok();
        factor();
        emit(IMM); emit(0);
        emit(EQ);
        expr_type = INT;
    } else if (token == '-') {
        next_tok();
        emit(IMM); emit(0);
        emit(PUSH);
        factor();
        emit(SUB);
        expr_type = INT;
    } else {
        error("syntax error in factor");
    }
}


static void expr_assign(void);

static void expr_mul(void) {
    factor();
    while (token == '*' || token == '/' || token == '%') {
        int op = token;
        next_tok();
        emit(PUSH);
        factor();
        if (op == '*') emit(MUL);
        else if (op == '/') emit(DIV);
        else emit(MOD);
    }
}

static void expr_add(void) {
    expr_mul();
    while (token == '+' || token == '-') {
        int op = token;
        next_tok();
        emit(PUSH);
        expr_mul();
        if (op == '+') emit(ADD);
        else emit(SUB);
    }
}

static void expr_rel(void) {
    expr_add();
    while (token == '<' || token == LE || token == '>' || token == GE) {
        int op = token;
        next_tok();
        emit(PUSH);
        expr_add();
        if (op == '<') emit(LT);
        else if (op == LE) emit(LE);
        else if (op == '>') emit(GT);
        else emit(GE);
    }
}

static void expr_eq(void) {
    expr_rel();
    while (token == EQ || token == NE) {
        int op = token;
        next_tok();
        emit(PUSH);
        expr_rel();
        if (op == EQ) emit(EQ);
        else emit(NE);
    }
}

static void expr_and(void) {
    expr_eq();
    while (token == AND) {
        next_tok();
        emit(PUSH);
        expr_eq();
        emit(AND);
    }
}

static void expr_or(void) {
    expr_and();
    while (token == OR) {
        next_tok();
        emit(PUSH);
        expr_and();
        emit(OR);
    }
}

static void expr_assign(void) {
    expr_or();
    if (token == '=') {
        next_tok();
        if (text[text_idx - 1] == LI || text[text_idx - 1] == LC) {
            int load_op = text[text_idx - 1];
            text_idx--;
            emit(PUSH);
            expr_assign();
            if (load_op == LC) emit(SC);
            else emit(SI);
        } else {
            error("invalid lvalue for assignment");
        }
    }
}

static void expr(int dummy) {
    (void)dummy;
    expr_assign();
}
static void statement(void) {
    if (token == '{') {
        match('{');
        while (token != '}' && token != 0) {
            statement();
        }
        match('}');
    } else if (token == If) {
        match(If);
        match('(');
        expr(0);
        match(')');
        emit(JZ);
        int jz_addr = text_idx++;
        statement();
        if (token == Else) {
            match(Else);
            text[jz_addr] = text_idx + 2;
            emit(JMP);
            int jmp_addr = text_idx++;
            statement();
            text[jmp_addr] = text_idx;
        } else {
            text[jz_addr] = text_idx;
        }
    } else if (token == While) {
        match(While);
        int loop_start = text_idx;
        match('(');
        expr(0);
        match(')');
        emit(JZ);
        int jz_addr = text_idx++;
        statement();
        emit(JMP);
        emit(loop_start);
        text[jz_addr] = text_idx;
    } else if (token == For) {
        match(For);
        match('(');
        if (token != ';') expr(0);
        match(';');
        int cond_start = text_idx;
        int cond_jz = 0;
        if (token != ';') {
            expr(0);
            emit(JZ);
            cond_jz = text_idx++;
        }
        match(';');

        int step_start = 0;
        int step_jmp = 0;
        if (token != ')') {
            emit(JMP);
            step_jmp = text_idx++;
            step_start = text_idx;
            expr(0);
            emit(JMP);
            emit(cond_start);
            text[step_jmp] = text_idx;
        }
        match(')');

        statement();

        if (step_start != 0) {
            emit(JMP);
            emit(step_start);
        } else {
            emit(JMP);
            emit(cond_start);
        }
        if (cond_jz != 0) {
            text[cond_jz] = text_idx;
        }
    } else if (token == Return) {
        match(Return);
        if (token != ';') {
            expr(0);
        }
        match(';');
        emit(LEV);
    } else if (token == ';') {
        match(';');
    } else {
        expr(0);
        match(';');
    }
}

static void register_native(const char* name, int id) {
    Symbol* s = lookup_symbol(name);
    s->class = 100;
    s->val = id;
    s->type = INT;
}

static void parse_program(void) {
    register_native("printf", NAT_PRINTF);
    register_native("malloc", NAT_MALLOC);
    register_native("free", NAT_FREE);
    register_native("exit", NAT_EXIT);
    register_native("print", NAT_PRINT);
    register_native("println", NAT_PRINTLN);
    register_native("strcpy", NAT_STRCPY);

    next_tok();
    while (token != 0) {
        int base_type = INT;
        if (token == Int) { base_type = INT; next_tok(); }
        else if (token == Char) { base_type = CHAR; next_tok(); }
        else if (token == Void) { base_type = INT; next_tok(); }

        while (token != ';') {
            int type = base_type;
            while (token == '*') { type = PTR; next_tok(); }

            if (token != Id) error("identifier expected");
            char name[32];
            strncpy(name, token_str, 31);
            Symbol* s = lookup_symbol(name);
            next_tok();

            if (token == '(') { // Function Definition
                s->class = 2;
                s->val = text_idx;
                s->type = type;

                match('(');
                loc_offset = 0;
                int param_count = 0;
                while (token != ')') {
                    int ptype = INT;
                    if (token == Int) { ptype = INT; next_tok(); }
                    else if (token == Char) { ptype = CHAR; next_tok(); }
                    while (token == '*') { ptype = PTR; next_tok(); }

                    if (token != Id) error("parameter name expected");
                    Symbol* ps = lookup_symbol(token_str);
                    ps->class = 1;
                    ps->type = ptype;
                    // Parameters are at [bp + 2 + param_count]
                    ps->val = -(param_count + 2);
                    param_count++;
                    next_tok();
                    if (token == ',') next_tok();
                }
                match(')');

                match('{');
                emit(ENT);
                int ent_idx = text_idx++;
                int local_vars = 0;

                while (token == Int || token == Char) {
                    int ltype = (token == Char) ? CHAR : INT;
                    next_tok();
                    while (1) {
                        int vtype = ltype;
                        while (token == '*') { vtype = PTR; next_tok(); }
                        if (token != Id) error("variable name expected");
                        Symbol* ls = lookup_symbol(token_str);
                        local_vars++;
                        ls->class = 1;
                        ls->type = vtype;
                        // Local variables are at [bp - local_vars]
                        ls->val = local_vars;
                        next_tok();
                        if (token == '=') {
                            next_tok();
                            emit(LEA);
                            emit(ls->val);
                            emit(PUSH);
                            expr(0);
                            if (vtype == CHAR) emit(SC);
                            else emit(SI);
                        }
                        if (token == ';') { next_tok(); break; }
                        match(',');
                    }
                }
                text[ent_idx] = local_vars;

                while (token != '}' && token != 0) {
                    statement();
                }
                match('}');
                emit(LEV);
                break;
            } else {
                s->class = 3;
                s->type = type;
                s->val = data_idx;
                data_idx += 8;
                if (token == ';') { next_tok(); break; }
                match(',');
            }
        }
    }
}

// ----------------------------------------------------------------------------
// Bytecode Virtual Machine Engine
// ----------------------------------------------------------------------------
static int run_vm(int main_addr) {
    // Reserve an EXIT instruction at the end of text segment
    text[text_idx] = EXIT;
    int64_t* exit_addr = &text[text_idx++];

    int64_t* pc = &text[main_addr];
    int64_t* sp = &stack[MAX_STACK - 1];
    
    // Push return address for main() pointing to EXIT instruction
    *--sp = (intptr_t)exit_addr;
    int64_t* bp = sp;
    int64_t ax = 0;

    while (1) {
        int64_t op = *pc++;
        switch (op) {
            case IMM:   ax = *pc++; break;
            case LEA: {
                int offset = (int)*pc++;
                // Local variables are positive offset (bp - offset)
                // Parameters are negative offset (bp - offset = bp + |offset|)
                ax = (intptr_t)(bp - offset);
                break;
            }
            case JMP:   pc = &text[*pc]; break;
            case JZ:    pc = (ax == 0) ? &text[*pc] : pc + 1; break;
            case JNZ:   pc = (ax != 0) ? &text[*pc] : pc + 1; break;
            case CALL: {
                int64_t target = *pc++;
                *--sp = (intptr_t)pc;
                pc = &text[target];
                break;
            }
            case ENT:   *--sp = (intptr_t)bp; bp = sp; sp -= *pc++; break;
            case ADJ:   sp += *pc++; break;
            case LEV:   sp = bp; bp = (int64_t*)*sp++; pc = (int64_t*)*sp++; break;
            case LI:    ax = *(int64_t*)ax; break;
            case LC:    ax = *(char*)ax; break;
            case SI:    *(int64_t*)*sp++ = ax; break;
            case SC:    *(char*)*sp++ = (char)ax; break;
            case PUSH:  *--sp = ax; break;

            case OR:    ax = *sp++ | ax; break;
            case XOR:   ax = *sp++ ^ ax; break;
            case AND:   ax = *sp++ & ax; break;
            case EQ:    ax = (*sp++ == ax); break;
            case NE:    ax = (*sp++ != ax); break;
            case LT:    ax = (*sp++ < ax); break;
            case GT:    ax = (*sp++ > ax); break;
            case LE:    ax = (*sp++ <= ax); break;
            case GE:    ax = (*sp++ >= ax); break;
            case SHL:   ax = *sp++ << ax; break;
            case SHR:   ax = *sp++ >> ax; break;
            case ADD:   ax = *sp++ + ax; break;
            case SUB:   ax = *sp++ - ax; break;
            case MUL:   ax = *sp++ * ax; break;
            case DIV:   if (ax == 0) { axiom_println("crun: division by zero"); return 1; } ax = *sp++ / ax; break;
            case MOD:   if (ax == 0) { axiom_println("crun: division by zero"); return 1; } ax = *sp++ % ax; break;

            case NATIVE: {
                int nat_id = *pc++;
                switch (nat_id) {
                    case NAT_PRINTF: {
                        int num_args = (int)*(pc + 1); // ADJ operand has arg_count
                        const char* fmt = (const char*)sp[num_args - 1];
                        int arg_idx = 1;
                        if (fmt) {
                            while (*fmt) {
                                if (*fmt == '%' && *(fmt + 1)) {
                                    fmt++;
                                    if (*fmt == 'd' || *fmt == 'i') {
                                        int64_t val = sp[num_args - 1 - arg_idx++];
                                        char num_buf[24];
                                        int nidx = 0;
                                        if (val == 0) num_buf[nidx++] = '0';
                                        else {
                                            int neg = 0;
                                            if (val < 0) { neg = 1; val = -val; }
                                            char tmp[24]; int ti = 0;
                                            while (val > 0) { tmp[ti++] = '0' + (val % 10); val /= 10; }
                                            if (neg) num_buf[nidx++] = '-';
                                            for (int i = ti - 1; i >= 0; i--) num_buf[nidx++] = tmp[i];
                                        }
                                        num_buf[nidx] = '\0';
                                        axiom_print(num_buf);
                                    } else if (*fmt == 's') {
                                        const char* s = (const char*)sp[num_args - 1 - arg_idx++];
                                        if (s) axiom_print(s); else axiom_print("(null)");
                                    } else if (*fmt == 'c') {
                                        char ch = (char)sp[num_args - 1 - arg_idx++];
                                        char ch_str[2] = { ch, '\0' };
                                        axiom_print(ch_str);
                                    } else if (*fmt == 'x' || *fmt == 'X' || *fmt == 'p') {
                                        uint64_t val = (uint64_t)sp[num_args - 1 - arg_idx++];
                                        char hex_buf[20];
                                        axiom_print("0x");
                                        int hi = 0;
                                        char htmp[16];
                                        if (val == 0) axiom_print("0");
                                        else {
                                            const char* hex_digits = "0123456789abcdef";
                                            while (val > 0) { htmp[hi++] = hex_digits[val & 0xF]; val >>= 4; }
                                            for (int i = hi - 1; i >= 0; i--) hex_buf[hi - 1 - i] = htmp[i];
                                            hex_buf[hi] = '\0';
                                            axiom_print(hex_buf);
                                        }
                                    } else if (*fmt == '%') {
                                        axiom_print("%");
                                    }
                                } else {
                                    char ch_str[2] = { *fmt, '\0' };
                                    axiom_print(ch_str);
                                }
                                fmt++;
                            }
                        }
                        ax = 0;
                        break;
                    }
                    case NAT_MALLOC: {
                        size_t sz = (size_t)sp[0];
                        ax = (intptr_t)malloc(sz);
                        break;
                    }
                    case NAT_FREE: {
                        void* p = (void*)sp[0];
                        free(p);
                        ax = 0;
                        break;
                    }
                    case NAT_EXIT: {
                        int code = (int)sp[0];
                        return code;
                    }
                    case NAT_PRINT: {
                        const char* s = (const char*)sp[0];
                        if (s) axiom_print(s);
                        ax = 0;
                        break;
                    }
                    case NAT_PRINTLN: {
                        const char* s = (const char*)sp[0];
                        if (s) axiom_println(s); else axiom_println("");
                        ax = 0;
                        break;
                    }
                    case NAT_STRCPY: {
                        char* dst = (char*)sp[1];
                        const char* src = (const char*)sp[0];
                        if (dst && src) strcpy(dst, src);
                        ax = (intptr_t)dst;
                        break;
                    }
                    default:
                        axiom_println("crun: unknown native syscall");
                        return 1;
                }
                break;
            }

            case EXIT:
                return (int)ax;

            default:
                axiom_println("crun: fatal VM instruction trap");
                return 1;
        }
    }
}

// ----------------------------------------------------------------------------
// Main Entry Point
// ----------------------------------------------------------------------------
int main(int argc, char** argv, CapKeyring* keyring) {
    if (argc > 1 && strcmp(argv[1], "-dh") == 0) {
        axiom_println("  crun                 - on-device C compiler and virtual machine");
        return 0;
    }

    if (argc > 1 && (strcmp(argv[1], "-h") == 0 || strcmp(argv[1], "--help") == 0)) {
        axiom_set_color(COLOR_GOLD, COLOR_BG);
        axiom_println("Usage: crun <file.c | -e \"code\"> [options]");
        axiom_set_color(COLOR_SILVER, COLOR_BG);
        axiom_println("Compile and execute C source programs on-device inside Ring 3.");
        axiom_println("Features:");
        axiom_println("  - Full C syntax (functions, loops, pointers, recursion, expressions)");
        axiom_println("  - Built-in runtime: printf, malloc, free, exit, print, println, strcpy");
        axiom_println("Options:");
        axiom_println("  -e <code>    Execute inline C source string");
        axiom_println("  -h, --help   Show this command reference manual");
        return 0;
    }

    const char* code_source = NULL;
    char inline_buf[512];

    if (argc > 2 && strcmp(argv[1], "-e") == 0) {
        strncpy(inline_buf, argv[2], 511);
        inline_buf[511] = '\0';
        code_source = inline_buf;
    } else if (argc > 1) {
        CapBlobContext* blob = (CapBlobContext*)keyring->storage_cap;
        if (blob && blob->length > 0) {
            // Ensure null termination
            if (blob->length < 1024) {
                blob->data[blob->length] = '\0';
            }
            code_source = blob->data;
        } else {
            axiom_set_color(COLOR_CRIMSON, COLOR_BG);
            axiom_print("crun: unable to read source file: ");
            axiom_println(argv[1]);
            axiom_set_color(COLOR_SILVER, COLOR_BG);
            return 1;
        }
    } else {
        axiom_set_color(COLOR_GOLD, COLOR_BG);
        axiom_println("crun: no input file or expression specified. Try 'crun -h'.");
        axiom_set_color(COLOR_SILVER, COLOR_BG);
        return 1;
    }

    src_ptr = code_source;
    line_num = 1;
    text_idx = 0;
    data_idx = 0;
    sym_count = 0;

    parse_program();

    Symbol* main_sym = lookup_symbol("main");
    if (!main_sym || main_sym->class != 2) {
        axiom_set_color(COLOR_CRIMSON, COLOR_BG);
        axiom_println("crun: error: undefined reference to 'main'");
        axiom_set_color(COLOR_SILVER, COLOR_BG);
        return 1;
    }

    int res = run_vm(main_sym->val);
    return res;
}
