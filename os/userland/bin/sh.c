#include "../include/axiom.h"

// ----------------------------------------------------------------------------
// Sovereign Shell Script Runner & Command Interpreter (sh)
// ----------------------------------------------------------------------------

#define MAX_VARS 32
typedef struct {
    char name[32];
    char value[128];
} EnvVar;

static EnvVar env_vars[MAX_VARS];
static int env_count = 0;

static const char* get_env(const char* name) {
    for (int i = 0; i < env_count; i++) {
        if (strcmp(env_vars[i].name, name) == 0) {
            return env_vars[i].value;
        }
    }
    return "";
}

static void set_env(const char* name, const char* val) {
    for (int i = 0; i < env_count; i++) {
        if (strcmp(env_vars[i].name, name) == 0) {
            strncpy(env_vars[i].value, val, 127);
            env_vars[i].value[127] = '\0';
            return;
        }
    }
    if (env_count < MAX_VARS) {
        strncpy(env_vars[env_count].name, name, 31);
        env_vars[env_count].name[31] = '\0';
        strncpy(env_vars[env_count].value, val, 127);
        env_vars[env_count].value[127] = '\0';
        env_count++;
    }
}

static void expand_variables(const char* in, char* out, size_t max_out) {
    size_t oi = 0;
    while (*in && oi + 1 < max_out) {
        if (*in == '$') {
            in++;
            char var_name[32];
            int vi = 0;
            if (*in == '{') {
                in++;
                while (*in && *in != '}' && vi < 31) {
                    var_name[vi++] = *in++;
                }
                if (*in == '}') in++;
            } else {
                while ((*in >= 'a' && *in <= 'z') || (*in >= 'A' && *in <= 'Z') ||
                       (*in >= '0' && *in <= '9') || *in == '_') {
                    if (vi < 31) var_name[vi++] = *in;
                    in++;
                }
            }
            var_name[vi] = '\0';
            const char* val = get_env(var_name);
            while (*val && oi + 1 < max_out) {
                out[oi++] = *val++;
            }
        } else {
            out[oi++] = *in++;
        }
    }
    out[oi] = '\0';
}

static int execute_line(char* line) {
    // Strip leading whitespace
    while (*line == ' ' || *line == '\t') line++;
    if (*line == '\0' || *line == '#') return 0; // Empty line or comment

    // Strip trailing newline / whitespace
    int len = strlen(line);
    while (len > 0 && (line[len - 1] == '\r' || line[len - 1] == '\n' || line[len - 1] == ' ' || line[len - 1] == '\t')) {
        line[--len] = '\0';
    }
    if (len == 0) return 0;

    // Expand variables in line
    char exp_line[256];
    expand_variables(line, exp_line, sizeof(exp_line));

    // Variable assignment: VAR=VAL
    char* eq = (char*)strchr(exp_line, '=');
    if (eq && (exp_line[0] != ' ' && exp_line[0] != '\t')) {
        int is_ident = 1;
        for (char* p = exp_line; p < eq; p++) {
            if (!((*p >= 'a' && *p <= 'z') || (*p >= 'A' && *p <= 'Z') || (*p >= '0' && *p <= '9') || *p == '_')) {
                is_ident = 0;
                break;
            }
        }
        if (is_ident) {
            *eq = '\0';
            const char* val = eq + 1;
            // Strip surrounding quotes
            if ((val[0] == '"' || val[0] == '\'') && val[strlen(val) - 1] == val[0]) {
                char clean_val[128];
                int clen = strlen(val) - 2;
                strncpy(clean_val, val + 1, clen);
                clean_val[clen] = '\0';
                set_env(exp_line, clean_val);
            } else {
                set_env(exp_line, val);
            }
            return 0;
        }
    }

    // Split command and arguments
    char* cmd = exp_line;
    char* args = (char*)strchr(exp_line, ' ');
    if (args) {
        *args = '\0';
        args++;
        while (*args == ' ') args++;
    } else {
        args = "";
    }

    // Built-in commands
    if (strcmp(cmd, "echo") == 0) {
        // Strip outer quotes if any
        int alen = strlen(args);
        if (alen >= 2 && ((args[0] == '"' && args[alen - 1] == '"') || (args[0] == '\'' && args[alen - 1] == '\''))) {
            args[alen - 1] = '\0';
            args++;
        }
        axiom_println(args);
        return 0;
    }

    if (strcmp(cmd, "export") == 0 || strcmp(cmd, "set") == 0) {
        char* eq2 = (char*)strchr(args, '=');
        if (eq2) {
            *eq2 = '\0';
            set_env(args, eq2 + 1);
        }
        return 0;
    }

    if (strcmp(cmd, "clear") == 0) {
        axiom_clear();
        return 0;
    }

    if (strcmp(cmd, "exit") == 0) {
        axiom_exit(0);
        return 0;
    }

    if (strcmp(cmd, "help") == 0) {
        axiom_set_color(COLOR_GOLD, COLOR_BG);
        axiom_println("Sovereign Shell (sh) Built-in Commands:");
        axiom_set_color(COLOR_SILVER, COLOR_BG);
        axiom_println("  echo <text>          - print text with $VAR expansion");
        axiom_println("  VAR=value            - set shell environment variable");
        axiom_println("  export VAR=value     - export environment variable");
        axiom_println("  clear                - clear terminal canvas");
        axiom_println("  exit                 - terminate shell session");
        return 0;
    }

    // Print notice for executed system command
    axiom_set_color(COLOR_MUTED, COLOR_BG);
    axiom_print("[sh] executing: ");
    axiom_print(cmd);
    if (*args) {
        axiom_print(" ");
        axiom_print(args);
    }
    axiom_println("");
    axiom_set_color(COLOR_SILVER, COLOR_BG);
    return 0;
}

static void run_script(const char* script) {
    char line_buf[256];
    int li = 0;
    while (*script) {
        char ch = *script++;
        if (ch == '\n' || ch == ';') {
            line_buf[li] = '\0';
            execute_line(line_buf);
            li = 0;
        } else {
            if (li < 255) line_buf[li++] = ch;
        }
    }
    if (li > 0) {
        line_buf[li] = '\0';
        execute_line(line_buf);
    }
}

int main(int argc, char** argv, CapKeyring* keyring) {
    if (argc > 1 && strcmp(argv[1], "-dh") == 0) {
        axiom_println("  sh                   - sovereign shell script runner");
        return 0;
    }

    if (argc > 1 && (strcmp(argv[1], "-h") == 0 || strcmp(argv[1], "--help") == 0)) {
        axiom_set_color(COLOR_GOLD, COLOR_BG);
        axiom_println("Usage: sh <script.sh | -c \"commands\"> [options]");
        axiom_set_color(COLOR_SILVER, COLOR_BG);
        axiom_println("Sovereign shell script execution engine.");
        axiom_println("Options:");
        axiom_println("  -c <cmds>    Execute inline command string");
        axiom_println("  -h, --help   Show this command reference manual");
        return 0;
    }

    // Set initial environment variables
    set_env("SHELL", "/apps/sys/sh.bin");
    set_env("USER", "root");
    set_env("OS", "AxiomOS");

    if (argc > 2 && strcmp(argv[1], "-c") == 0) {
        run_script(argv[2]);
        return 0;
    }

    if (argc > 1) {
        // Read script from storage cap
        CapBlobContext* blob = (CapBlobContext*)keyring->storage_cap;
        if (blob && blob->length > 0) {
            run_script(blob->data);
            return 0;
        } else {
            axiom_set_color(COLOR_CRIMSON, COLOR_BG);
            axiom_print("sh: unable to read script file: ");
            axiom_println(argv[1]);
            axiom_set_color(COLOR_SILVER, COLOR_BG);
            return 1;
        }
    }

    // Interactive REPL Mode
    axiom_set_color(COLOR_GOLD, COLOR_BG);
    axiom_println("AxiomOS Sovereign Shell (sh) v0.3.0");
    axiom_set_color(COLOR_SILVER, COLOR_BG);
    axiom_println("Type 'help' for built-ins or 'exit' to return.");

    char line[128];
    while (1) {
        axiom_set_color(COLOR_CYAN, COLOR_BG);
        axiom_print("axsh$ ");
        axiom_set_color(COLOR_SILVER, COLOR_BG);
        int len = axiom_getline(line, sizeof(line));
        if (len < 0) break;
        if (len > 0) {
            execute_line(line);
        }
    }

    return 0;
}
