# Axiom Microkernel: Boot Modularity & Custom OS Personality Guide

## 1. Architectural Philosophy: The Sovereign Substrate

Axiom Microkernel is a pure, capability-based microkernel written in freestanding Rust (`#![no_std]`). Following the classic L4 and seL4 microkernel philosophy, the microkernel is **strictly agnostic to the operating system personality running on top of it**.

The repository is divided into two sovereign domains:
- **`kernel/` (Axiom Microkernel Substrate)**: Ring 0 trusted computing base (TCB) providing memory virtualization (PML4 paging), thread scheduling (preemptive round-robin), capability security tables (CNodes & CSpaces), hardware interrupt dispatch, and primitive IPC endpoints.
- **`os/` (AxiomOS Userspace Distribution)**: Ring 3 userspace operating system consisting of Genesis (system manager & capability shell), `libaxiom` (capability runtime & structural graph filesystem), and userland executables.

```text
+-----------------------------------------------------------------+
|                        AxiomOS Userspace                        |
|   (Genesis PID 1, System Servers, Shell, CapGraph, C Binaries)  |
|            OR ANY CUSTOM THIRD-PARTY OS PERSONALITY             |
+-----------------------------------------------------------------+
                                |
             Hardware Syscall ABI (syscall / sysret)
                                |
+-----------------------------------------------------------------+
|                    Axiom Microkernel (Ring 0)                   |
|     Capabilities | VSpaces | CNodes | TCBs | Scheduler | IPC    |
+-----------------------------------------------------------------+
                                |
                 Sovereign BootInfo Hand-Off
                                |
+-----------------------------------------------------------------+
|                   Axiom UEFI Bootloader (GOP)                   |
|     Loads: /axiom_kernel.bin  AND  /genesis.bin (Initrd Module)  |
+-----------------------------------------------------------------+
```

---

## 2. Boot Modularity & The `BootInfo` Specification

The microkernel contains **zero compile-time embedded userspace code**. Instead, the UEFI bootloader (`kernel/boot/uefi`) loads the microkernel and the initial OS binary from the ESP FAT32 volume and hands off execution via the `BootInfo` structure.

### BootInfo ABI (Physical Address `0x6000`)

```rust
#[repr(C)]
pub struct BootInfo {
    pub magic: u32,                     // 0xC0DEB007 (Verification Magic)
    pub version: u32,                   // 0x03 (BootInfo ABI version)
    pub memory_map_ptr: u64,            // Physical address of E820 map (0x6100)
    pub memory_map_len: u64,            // Number of memory descriptors
    pub memory_map_entry_size: u64,     // Size of MemoryMapEntry (24 bytes)
    pub framebuffer_base: u64,          // Physical base of GOP linear framebuffer
    pub framebuffer_size: u64,          // Framebuffer size in bytes
    pub horizontal_resolution: u32,     // Display width (e.g., 1280)
    pub vertical_resolution: u32,       // Display height (e.g., 720)
    pub pixels_per_scan_line: u32,      // Framebuffer stride (pitch)
    pub initrd_base: u64,               // Physical base address of Initial OS Module
    pub initrd_size: u64,               // Size in bytes of Initial OS Module
}
```

### Bootloader Hand-Off Sequence:
1. `axiom_boot` queries UEFI Simple File System Protocol on the ESP boot drive.
2. It loads `axiom_kernel.bin` into physical `0x100000` (1 MiB).
3. It searches for `genesis.bin` on the root of the ESP drive and reads it into physical `0x2000000` (32 MiB).
4. It sets `BootInfo.initrd_base = 0x2000000` and `BootInfo.initrd_size = <file_size>`.
5. The microkernel boots, automatically reserves `0x2000000..0x2000000 + initrd_size` in its physical bitmap allocator, and creates the first Ring 3 process (PID 1) by copying the module into virtual address `0x400000`.

---

## 3. The Genesis Execution Environment (PID 1)

When the microkernel spawns the initial OS personality binary (PID 1), it provides a standard Ring 3 environment:

### Virtual Address Space Layout
- **`0x400000 .. 0x480000`**: Executable code and data (Initial OS binary, up to 512 KiB).
- **`0x500000 .. 0x510000`**: Scratch / user program execution space (64 KiB).
- **`0x600000 .. 0x601000`**: Initial process data frame (keyring, IPC contexts, argv).
- **`0x7F0000 .. 0x800000`**: Initial process user stack (64 KiB, stack top at `0x800000`).

### Capability Space (CNode Slots)
The initial process starts with root capabilities in its private CNode:
| Slot | Capability | Description |
| :--- | :--- | :--- |
| **Slot 1** | `CNode` | Process's own Capability Space (allows minting & granting caps) |
| **Slot 2** | `VSpace` | Process's Virtual Address Space (allows mapping/unmapping memory) |
| **Slot 3** | `TCB` | Process's Thread Control Block (task lifecycle, exit, wait, kill) |
| **Slot 4** | `Untyped` | Reserved physical memory for dynamic retyping |
| **Slot 5** | `Console` | Graphics video output and PS/2 keyboard input endpoint |
| **Slot 7** | `Storage` | Hardware ATA PIO primary storage controller endpoint |

---

## 4. Hardware System Call ABI

Processes communicate with the microkernel using the x86_64 `syscall` instruction with zero runtime libraries required:

### Register Convention
- **`%rdi`**: Capability Slot Handle (e.g. `0x100000005` for Console)
- **`%rsi`**: Syscall Operation Code
- **`%rdx`**: Argument 1 (e.g. ASCII characters packed in 64-bit chunk, or memory address)
- **`%r10`**: Argument 2 (e.g. chunk length, or operation flags)
- **`%rax`**: Return status code (`0` = Success)

### Core Syscall Operations
| Opcode | Name | Capability Target | Parameters |
| :--- | :--- | :--- | :--- |
| **`6`** | `OP_CONSOLE_WRITE` | Console (`Slot 5`) | `%rdx`: 8 packed ASCII bytes, `%r10`: length (1..8) |
| **`7`** | `OP_CONSOLE_CLEAR` | Console (`Slot 5`) | `%rdx`: background color |
| **`10`** | `OP_CONSOLE_GETCHAR`| Console (`Slot 5`) | Returns ASCII code in `%rax` (-1 if no key) |
| **`11`** | `OP_CONSOLE_SET_CURSOR`| Console (`Slot 5`) | `%rdx`: row, `%r10`: column |
| **`12`** | `OP_TASK_SPAWN` | TCB (`Slot 3`) | `%rdx`: pointer to `TaskSpawnParams` |
| **`13`** | `OP_TASK_EXIT` | TCB (`Slot 3`) | `%rdx`: exit status code |
| **`14`** | `OP_TASK_WAIT` | TCB (`Slot 3`) | `%rdx`: target PID |
| **`15`** | `OP_TASK_LIST` | TCB (`Slot 3`) | `%rdx`: pointer to `ProcessInfo` buffer, `%r10`: capacity |
| **`16`** | `OP_TASK_KILL` | TCB (`Slot 3`) | `%rdx`: target PID to terminate |
| **`20`** | `OP_STORAGE_READ` | Storage (`Slot 7`) | `%rdx`: LBA sector, `%r10`: 512-byte buffer address |
| **`21`** | `OP_STORAGE_WRITE` | Storage (`Slot 7`) | `%rdx`: LBA sector, `%r10`: 512-byte buffer address |

---

## 5. Building a Custom OS on Axiom Microkernel

To build your own operating system or bare-metal environment on top of Axiom:

### Step 1: Write Your Initial OS Binary (e.g. `my_os.c`)

```c
// Entry point linked at 0x400000
void print_str(const char* s) {
    while (*s) {
        unsigned long chunk = 0;
        int len = 0;
        while (*s && len < 8) {
            chunk |= ((unsigned long)(unsigned char)*s++) << (len * 8);
            len++;
        }
        // Invoke OP_CONSOLE_WRITE (Opcode 6) on Console Capability (Slot 5)
        register unsigned long rdi asm("rdi") = 0x100000005;
        register unsigned long rsi asm("rsi") = 6;
        register unsigned long rdx asm("rdx") = chunk;
        register unsigned long r10 asm("r10") = len;
        asm volatile("syscall" : : "r"(rdi), "r"(rsi), "r"(rdx), "r"(r10) : "rax", "rcx", "r11", "memory");
    }
}

void _start() {
    print_str("Hello from My Custom OS running on Axiom Microkernel!\n");

    // Main event loop
    while (1) {
        asm volatile("pause");
    }
}
```

### Step 2: Compile & Link at `0x400000`

```bash
gcc -m64 -ffreestanding -fno-pie -fno-stack-protector -nostdlib -mno-red-zone -O2 -c my_os.c -o my_os.o
ld -m elf_x86_64 -Ttext=0x400000 -o my_os.elf my_os.o
objcopy -O binary my_os.elf genesis.bin
```

### Step 3: Package & Boot

Place `genesis.bin` onto the FAT32 ESP disk image:

```bash
mcopy -o -i target/axiom_mkern.img genesis.bin ::genesis.bin
```

Launch with QEMU:
```bash
./run.sh
```

Axiom Microkernel will boot, initialize capabilities and paging, load your `genesis.bin`, and jump to its `_start` in Ring 3!
