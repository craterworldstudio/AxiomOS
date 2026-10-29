# Architecture Specification: Beyond Inodes — The Sovereign CapObject & Continual VaultStore

## 1. Why We Reject Unix Inodes

In traditional operating systems (Unix, Linux, ext4, UFS), storage metadata is built around the **inode (index node)**, invented in 1970:
- **Global Integer Identifiers**: Inodes are indexed by global flat integers (`stat.st_ino`). Any program with read access can enumerate, brute-force, or guess file identities across the filesystem.
- **Ambient UID/GID Permissions**: Inodes store integer user/group IDs and permission bitmasks (`rwxr-xr-x`). This creates privilege escalation vectors (confused deputy, setuid root vulnerabilities, race conditions via `symlink` / `chown`).
- **In-Place Mutation & Fragility**: Inodes are overwritten in place on disk. When power fails during metadata updates, inodes are left half-written (torn writes), requiring complex repair tools (`fsck`) or causing silent filesystem corruption.
- **Inode Exhaustion Bug**: A disk can have 500 GB of free space, but if all available inodes are used up by small files, the OS throws `ENOSPC: No space left on device`.

### Inode Comparison Matrix

| Property | Unix Inode (ext4/UFS) | ZFS Dnode / Btrfs B-Tree | Axiom Sovereign CapObject ("VaultStore") |
| :--- | :--- | :--- | :--- |
| **Identity Model** | Global Integer (`ino_t`) | Object Set ID | **Cryptographic Capability Token / OID** |
| **Access Control** | Ambient UID/GID + ACLs | Posix ACLs | **Mathematically Attenuated Capabilities** |
| **Integrity** | None (silent bit rot) | Fletcher4 / SHA-256 | **BLAKE3 Cryptographic Merkle Root per Object** |
| **Mutation Model** | In-place update | Copy-on-Write (CoW) | **Strict Copy-on-Write Merkle DAG (Immutable History)** |
| **Allocation Limit**| Fixed inode table ceiling| Dynamic tree nodes | **Dynamic Extents (No Inode Count Limit)** |
| **Confused Deputy** | Highly vulnerable | Vulnerable | **Impossible (Zero Ambient Root / No Path Traversal)** |

---

## 2. The Solution: Sovereign CapObjects & Continual Merkle-Extents

AxiomOS replaces the 1970s inode with **Self-Authenticating CapObjects**:

```
+-----------------------------------------------------------------------------------------+
|                                SOVEREIGN CAPOBJECT DESCRIPTOR                           |
+-----------------------------------------------------------------------------------------+
| [Magic: 0x4341504F424A]  [Object ID: 128-bit UUID/Hash]  [ObjectKind: Blob | DirMap]    |
| [Merkle Root: 32-byte BLAKE3]  [Byte Length: 64-bit uint]  [Rights Mask: 32-bit uint]   |
| [Creation Epoch: 64-bit]       [Revision Generation: 64-bit]                            |
+-----------------------------------------------------------------------------------------+
| EXTENT RUN DESCRIPTORS (Dynamic Contiguous Disk Runs):                                   |
|   Run 0:  Start LBA: 0x00001000 | Sector Count: 128 (64 KB)  | Checksum: [b3_hash...]    |
|   Run 1:  Start LBA: 0x00004500 | Sector Count: 512 (256 KB) | Checksum: [b3_hash...]    |
|   ... (Overflow Extent Tree for Gigabyte+ Files)                                        |
+-----------------------------------------------------------------------------------------+
```

### Key Architectural Invariants
1. **Zero Global Inode Numbers**: Objects are addressed purely through unforgeable Capability Handles derived along private process CSpaces.
2. **Continual Extent-Based Allocation**: Files are not restricted to 1024 bytes or fixed block counts. Storage is allocated in contiguous extent runs `(start_lba, sector_count)`, maximizing sequential I/O throughput.
3. **Copy-on-Write (CoW) Tree Topology**:
   - In accordance with Axiom Manifesto Principle #4 (*"We reject Mutable History"*), disk blocks are **never overwritten in place**.
   - Writing new data allocates fresh unassigned sectors.
   - The root generation pointer is updated via a single, atomic 512-byte root sector commit.
   - If power is cut at any millisecond during writing, the previous consistent state remains completely untouched.

---

## 3. Threat Model & Hacker Exploit Mitigation Matrix

| # | Attack Vector | Hacker Exploit Scenario | AxiomOS Substrate Mitigation |
| :-: | :--- | :--- | :--- |
| **1** | **LBA Boundary Traversal** | Compromised user process holding partition capability `ata0p2` submits raw LBA 0 to overwrite system bootloader or partition headers. | **Hardware Capability Clamping**: Microkernel storage driver mathematically bounds $LBA_{\text{req}} < \text{PartitionLength}$ and offsets $LBA_{\text{phys}} = LBA_{\text{base}} + LBA_{\text{req}}$. Violations trigger immediate `ERR_CAP_OUT_OF_BOUNDS`. |
| **2** | **Disk Metadata Injection** | Malicious disk image or crafted sector payloads contain cyclic pointers, negative lengths, or overlapping extents to trigger heap overflow or panic. | **Cryptographic Merkle Validation & Zero-Alloc Deserialization**: Descriptors use fixed-size validated layouts with BLAKE3 checksums. Strict invariant bounds-checking on every field before decoding; cyclic references mathematically rejected. |
| **3** | **Storage Resource Starvation** | Rogue user script writes continuous garbage data until the disk is 100% full, preventing system logs or crash diagnostics from writing. | **Capability Storage Quotas**: Each namespace holds an Untyped Disk Budget token. Block allocations require spending capability quota units; unauthorized allocations are rejected before touching the block allocator. |
| **4** | **Data Remanence / Use-After-Free** | User A deletes sensitive cryptographic keys. User B allocates a new file and reads uninitialized disk sectors to steal Alice's deleted secrets. | **Zero-On-Allocate & Cryptographic Erasure**: The block allocator clears newly allocated sectors before granting access. Files are encrypted with ephemeral per-object keys; deletion securely destroys the key, rendering raw sectors permanent random noise. |
| **5** | **Torn Writes & Power Interruption** | Attacker cuts power mid-write to desynchronize file pointers and cause double-allocation corruption. | **Atomic CoW Root Pointer Commit**: New extents and metadata are written to unallocated sectors first. The master root generation is swapped in a single 512-byte atomic sector write. Uncommitted writes are discarded on reboot. |
| **6** | **Direct Hardware Port I/O Bypass** | Ring 3 malware attempts to execute raw assembly `out dx, al` to port `0x1F0` to bypass filesystem capability checks. | **Privilege Isolation & TSS I/O Bitmap**: Ring 3 threads execute with `CPL = 3` and `IOPL = 0`. The microkernel TSS I/O Permission Bitmap strictly denies userland access to ports `0x1F0..0x1F7`. |
| **7** | **Capability Handle Guessing** | Malicious binary attempts to brute-force 64-bit capability handles to gain access to unauthorized storage endpoints. | **Generational 64-Bit Monotonic Handles**: Handles combine a private slot index and a 32-bit cryptographically updated generation counter. Handles cannot be forged or shared across address spaces. |

---

## 4. Disk Partitioning Architecture: Sovereign Vault Slices

```
+-------------------------------------------------------------------------------------------------+
|                       PHYSICAL STORAGE MEDIUM: edps/drives/ata0                                 |
+-------------------------------------------------------------------------------------------------+
| LBA 0: Protective MBR / Boot Record                                                             |
| LBA 1: Primary GPT Header                                                                       |
| LBA 2..33: GPT Partition Entries (Axiom Sovereign Vault GUID: 0xA8104D5F-...)                   |
+-------------------------------------------------------------------------------------------------+
| PARTITION 1: System Substrate (/edps/drives/ata0p1)                                             |
| - Offset: LBA 2048 (1 MB boundary) | Size: 16 MB                                                |
| - Contents: Read-only signed core binaries, kernel fallback, and root identity                  |
+-------------------------------------------------------------------------------------------------+
| PARTITION 2: Continual User Data Vault (/edps/drives/ata0p2)                                    |
| - Offset: LBA 34816 | Size: Remainder of drive                                                  |
| - Contents: Dynamic Free Block Bitmap, Continual CapObject Extents, User Vaults                |
+-------------------------------------------------------------------------------------------------+
| LBA N-33..N-1: Secondary Backup GPT Partition Entries                                           |
| LBA N: Backup GPT Header                                                                        |
+-------------------------------------------------------------------------------------------------+
```
