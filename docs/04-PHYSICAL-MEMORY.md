# Axiom 1: Physical Memory

## 1. Purpose

Axiom's kernel cannot safely allocate physical memory by assuming that every address reported as RAM is available to the operating system.

Firmware describes the physical memory topology of the machine, while Axiom additionally tracks which regions are already owned by the boot process and kernel.

The physical memory allocator therefore operates on the intersection of:

```text
Firmware-reported usable memory ∩ Axiom-owned memory excluded = Allocatable physical frames
```

This document describes the first physical memory management subsystem implemented in Axiom 1.

---

## 2. Firmware Memory Discovery

During the 16-bit real-mode portion of Stage 2, the bootloader queries the BIOS using the `INT 15h, EAX=E820` interface.

The BIOS returns a list of physical memory regions.

Each entry contains:

* Base physical address
* Region length
* Region type
* ACPI extended attributes

Axiom preserves these entries and passes them to the Rust kernel.

The kernel does not assume a fixed amount of RAM.

---

## 3. BootInfo ABI

The bootloader constructs a `BootInfo` structure and places it at physical address `0x6000`.

The structure contains:

* Axiom boot magic
* ABI version
* Pointer to the E820 memory map
* Number of memory-map entries
* Memory-map entry size

The memory map itself begins at physical address `0x6020`.

The bootloader passes the physical address of `BootInfo` to the Rust kernel through `RDI`.

The kernel receives it through:

```rust
_start(boot_info: *const BootInfo)
```

Both `BootInfo` and `MemoryMapEntry` use:

```rust
#[repr(C)]
```

so their binary layout is explicitly defined across the assembly/Rust boundary.

---

## 4. Observed QEMU Memory Map

The current QEMU BIOS reports:

```text
7 E820 entries
```

The kernel successfully reads the entry count from:

```text
BootInfo.memory_map_len
```

The exact entries are firmware-provided and should not be hardcoded into the allocator.

The allocator only treats entries with:

```text
region_type == 1
```

as usable RAM.

---

## 5. Physical Frame Abstraction

Axiom manages physical memory in 4 KiB pages.

The fundamental physical-memory object is:

```rust
pub struct PhysFrame {
    pub start_address: u64,
}
```

The frame size is defined as:

```rust
pub const PAGE_SIZE: u64 = 4096;
```

A valid frame therefore begins at an address divisible by `4096`.

Examples:

```text
0x00200000
0x00201000
0x00202000
0x00203000
```

Each address represents the beginning of a distinct 4 KiB physical frame.

---

## 6. Memory Ownership

E820 tells Axiom which regions the firmware considers usable.

It does **not** know which regions Axiom is currently using.

Therefore Axiom maintains a second layer of memory ownership.

A frame is allocatable only when:

```text
E820 says usable
AND
Axiom does not reserve it
```

This prevents the allocator from returning memory containing:

* Bootloader state
* Firmware structures
* Page tables
* Bootstrap stack
* Boot information
* Kernel code/data
* Other explicitly reserved hardware regions

---

## 7. Reserved Memory

For the initial Axiom 1 implementation, memory below the first megabyte is conservatively quarantined.

```text
[0x00000000, 0x00100000)
```

This covers the legacy low-memory region containing structures such as:

* Interrupt Vector Table
* BIOS Data Area
* BIOS/firmware-related structures
* Stage 1
* Stage 2
* Bootstrap page tables
* Bootstrap stack
* VGA memory

The kernel bootstrap region is additionally reserved:

```text
[0x00100000, 0x00200000)
```

This protects the initial kernel image and provides a conservative safety margin around the kernel.

Memory ranges use the invariant:

```text
[start, end)
```

where `end` is exclusive.

Therefore:

```text
[start, end)
```

means:

```text
start <= address < end
```

---

## 8. Frame Allocation Algorithm

`FrameAllocator` maintains:

```rust
pub struct FrameAllocator {
    boot_info: *const BootInfo,
    current_entry_index: usize,
    next_free_address: u64,
}
```

The allocator walks the E820 entries sequentially.

For every entry:

1. Ignore non-usable regions.
2. Establish the first candidate address.
3. Align the candidate upward to a 4 KiB boundary.
4. Calculate the end of the E820 region.
5. Generate candidate frames.
6. Reject candidates inside Axiom's reserved regions.
7. Return the first valid frame.
8. Continue from the next frame on subsequent allocations.
9. Move to the next E820 entry when the current region is exhausted.

Conceptually:

```text
for each E820 entry
    if not usable
        skip

    for each 4 KiB frame
        if Axiom-reserved
            skip
        else
            allocate
```

---

## 9. First Allocation Proof

The initial allocator test requested three physical frames.

The kernel returned:

```text
0x00200000
0x00201000
0x00202000
```

These addresses demonstrate:

* 4 KiB alignment
* Sequential frame allocation
* Lower 1 MiB quarantine enforcement
* Kernel-region quarantine enforcement
* E820 usable-memory filtering

The allocator correctly skipped the physical ranges below `0x00200000` that were excluded by Axiom's ownership rules.

---

## 10. Important Bootstrap Distinction

The bootloader already creates page tables that identity-map the first 2 MiB of physical memory.

This does **not** constitute a physical memory manager.

The two mechanisms serve different purposes.

### Bootloader paging

Its purpose is:

```text
Make enough memory addressable
        ↓
Enter 64-bit mode
        ↓
Execute the kernel
```

### Physical frame allocator

Its purpose is:

```text
Understand physical RAM
        ↓
Track Axiom-owned regions
        ↓
Provide unused 4 KiB frames
```

The existing 2 MiB identity mapping is therefore a bootstrap mechanism, not the final memory-management architecture.

---

## 11. Current Limitations

The first implementation deliberately uses conservative hardcoded reservations.

The kernel reservation currently covers:

```text
0x00100000 → 0x00200000
```

regardless of the actual linked kernel size.

Future versions should derive the kernel's physical boundaries from linker-provided symbols such as:

```text
_kernel_start
_kernel_end
```

The allocator currently does not implement frame reclamation.

There is also no general-purpose virtual memory allocator yet.

The current page tables are still the bootloader's bootstrap identity mapping.

These limitations are intentional for Axiom 1.

---

## 12. Verification Status

### Physical Memory Ownership

* [x] BIOS E820 memory discovery
* [x] BootInfo ABI
* [x] E820 map passed to Rust
* [x] `#[repr(C)]` ABI structures
* [x] 4 KiB `PhysFrame` abstraction
* [x] E820 usable-region filtering
* [x] Axiom reserved-region filtering
* [x] Sequential frame allocation
* [x] Verified first allocated frame
* [x] Verified consecutive frame allocation
* [x] Verified kernel region is skipped
* [x] Verified lower 1 MiB is skipped
* [x] 256-entry IDT
* [x] IDTR loading
* [x] IDT gate construction
* [x] CPU → assembly exception entry
* [x] Assembly → Rust transition
* [x] Rust → assembly return
* [x] IRETQ
* [x] Breakpoint exception (#3)
* [x] Assembly entry/return
* [x] Ring-0 exception frame
* [x] Panic renderer
* [x] #8 Double Fault vector
* [x] Kernel-owned GDT
* [x] Kernel code/data segment descriptors
* [x] Task State Segment (TSS)
* [x] Emergency Double Fault stack
* [x] LGDT
* [x] Kernel code-segment reload
* [x] LTR
* [x] Double Fault IST assignment
* [x] IDT
* [x] CPU exception entry stubs
* [x] CPU exception handlers
* [ ] Frame deallocation
* [ ] Dynamic kernel boundaries
* [ ] Physical frame accounting
* [ ] Full physical-memory ownership model
* [ ] Virtual memory manager
* [ ] Kernel heap




---

## 13. Next Step

The physical frame allocator and exception subsystem now provide the basic safety mechanisms required for Axiom's next memory-management layer.

The next major subsystem is:

**Virtual Memory**

The intended progression is:

```text
E820
  ↓
BootInfo
  ↓
Physical Frame Allocator
  ↓
IDT + Exceptions
  ↓
Kernel GDT + TSS + IST
  ↓
Virtual Memory
  ↓
Kernel Heap
  ↓
Processes / Threads
  ↓
IPC
  ↓
Capability Kernel
  ↓
VGOS
  ↓
AxSH

Virtual memory will replace the bootloader's temporary 2 MiB identity mapping with an Axiom-owned paging architecture.

This will allow Axiom to:
* Establish its own page-table hierarchy.
* Map physical frames into controlled virtual addresses.
* Protect kernel and allocator metadata from accidental access.
* Establish a dedicated kernel stack with a guard page.
* Provide the foundation required for a real kernel heap.
* Eventually support isolated address spaces for processes and capability-owned objects.

The current bootloader page tables remain a temporary bootstrap mechanism and should not be treated as part of Axiom's final virtual-memory architecture.