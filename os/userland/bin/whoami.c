#include "axiom.h"

int main(int argc, char** argv) {
    if (argc >= 2 && argv[1] != NULL) {
        if (strcmp(argv[1], "-dh") == 0) {
            axiom_println("  whoami               - display active user identity and authority keyring");
            return 0;
        }
        if (strcmp(argv[1], "-h") == 0 || strcmp(argv[1], "--help") == 0) {
            axiom_println("Usage: whoami [options]");
            axiom_println("Display active user identity and CSpace authority keyring.");
            axiom_println("Options:");
            axiom_println("  -h, --help    Show this command reference manual");
            return 0;
        }
    }

    const CapKeyring* kr = (const CapKeyring*)0x600000;
    const char* uname = kr->user_name;
    if (uname[0] == '\0') {
        uname = (kr->user_id == 0) ? "root" : "user";
    }

    axiom_print(uname);
    if (kr->user_id == 0) {
        axiom_println(" (Authority Keyring #0: Genesis Sovereign, Rights: 0b1111 [ALL])");
    } else {
        axiom_print(" (Authority Keyring #");
        char uid_str[16];
        uint32_t u = kr->user_id;
        int pos = 0;
        if (u == 0) {
            uid_str[pos++] = '0';
        } else {
            char tmp[16];
            int tpos = 0;
            while (u > 0) {
                tmp[tpos++] = '0' + (u % 10);
                u /= 10;
            }
            while (tpos > 0) {
                uid_str[pos++] = tmp[--tpos];
            }
        }
        uid_str[pos] = '\0';
        axiom_print(uid_str);
        axiom_print(", Rights: 0b");
        
        char rbits[5];
        rbits[0] = (kr->rights & 8) ? '1' : '0';
        rbits[1] = (kr->rights & 4) ? '1' : '0';
        rbits[2] = (kr->rights & 2) ? '1' : '0';
        rbits[3] = (kr->rights & 1) ? '1' : '0';
        rbits[4] = '\0';
        axiom_print(rbits);

        axiom_print(" [");
        int comma = 0;
        if (kr->rights & RIGHTS_READ) { axiom_print("READ"); comma = 1; }
        if (kr->rights & RIGHTS_WRITE) { if (comma) axiom_print("|"); axiom_print("WRITE"); comma = 1; }
        if (kr->rights & RIGHTS_EXEC) { if (comma) axiom_print("|"); axiom_print("EXEC"); comma = 1; }
        if (kr->rights & RIGHTS_GRANT) { if (comma) axiom_print("|"); axiom_print("GRANT"); }
        axiom_println("])");
    }

    return 0;
}
