# AxiomOS Sovereign Capability Filesystem ("VaultFS" / "CapFS")

## 1. Executive Summary & Philosophy

In traditional operating systems (Unix, Linux, Windows), filesystems are built around **ambient authority**:
- There is a single, global root directory (`/` or `C:\`) shared by all processes.
- Every process attempts to open paths like `/etc/shadow` or `/home/user/private.txt`.
- The kernel inspects string paths and checks permission bits (`rwxr-xr-x`) against integer user IDs (`UID 0`, `UID 1000`).
- This 1970s architecture suffers from fundamental security vulnerabilities: **confused deputy attacks**, **path traversal (`../../`)**, **symlink races**, and **UID 0 root privilege escalation**.

**AxiomOS completely replaces ambient paths with Object Capabilities:**
1. **Zero Global Root**: There is no universal `/` path.
2. **Directories as Capabilities**: A directory is an unforgeable Endpoint capability residing in a process's private Capability Space (CSpace).
3. **Files as Capabilities**: A file is an Endpoint capability derived from a directory capability.
4. **Per-Process Sovereign Namespaces**: An application only sees what has been explicitly delegated to it. If an app is launched with a capability pointing to `/home/user/notes`, the rest of the storage volume **literally does not exist** in its universe. Path traversal is mathematically impossible.
5. **User Accounts as Authority Keyrings**: Users are not integer UIDs in `/etc/passwd`. An account is an **Authority Keyring Capsule** containing scoped capabilities.

---

## 2. Capability Storage Architecture

```
+-----------------------------------------------------------------------------+
|                             PROCESS CSPACE (SLOTS)                          |
|                                                                             |
|  [Slot 1: CNode]  [Slot 2: VSpace]  [Slot 3: TCB]  [Slot 5: Console]        |
|                                                                             |
|  [Slot 7: Root Directory Capability (Rights::ALL)]                          |
+--------------------------------------|--------------------------------------+
                                       |
                   OP_FS_LOOKUP("etc") | returns derived CapHandle
                                       v
                     +-----------------------------------+
                     |   Directory Capability (Slot 8)   |
                     |   - Target: "etc" sub-tree        |
                     |   - Rights: READ | EXEC           |
                     +-----------------|-----------------+
                                       |
                OP_FS_LOOKUP("os.cfg") | returns derived CapHandle
                                       v
                     +-----------------------------------+
                     |      File Capability (Slot 9)     |
                     |      - Target: "os.cfg" content   |
                     |      - Rights: READ               |
                     +-----------------------------------+
```

---

## 3. Storage Operations & Capability ABI

All filesystem operations are performed via synchronous hardware fast-path system calls through the user's CSpace handles:

| Operation | Code | Arguments | Return | Description |
| :--- | :---: | :--- | :--- | :--- |
| `OP_FS_LOOKUP` | `10` | `name_ptr: u64`, `name_len: u64` | `new_handle: u64` | Resolves child entry in directory, returns new `CapHandle` |
| `OP_FS_CREATE` | `11` | `name_ptr: u64`, `is_dir: u64` | `new_handle: u64` | Carves a new file or sub-directory in this directory |
| `OP_FS_READ`   | `12` | `offset: u64`, `dest_buf: u64` | `bytes_read: u64` | Reads data from a file capability at specified offset |
| `OP_FS_WRITE`  | `13` | `offset: u64`, `src_buf: u64` | `bytes_written: u64` | Writes data to a file capability (requires `Rights::WRITE`) |
| `OP_FS_LIST`   | `14` | `entry_idx: u64`, `out_buf: u64` | `has_more: u64` | Lists entries in a directory capability |
| `OP_FS_STAT`   | `15` | `out_stat_ptr: u64`, `0` | `status: u64` | Returns object size, type, and content hash |
| `OP_FS_DELETE` | `16` | `name_ptr: u64`, `name_len: u64` | `status: u64` | Deletes a named entry from directory (requires `Rights::WRITE`) |

---

## 4. Rights Monotonicity & Attenuation

AxiomOS enforces mathematical rights monotonicity ($R_{\text{child}} \subseteq R_{\text{parent}}$) across all storage objects:

- **Read-Only Volume Mounts**:
  If Genesis delegates a directory capability with `Rights::READ`, any file or subdirectory looked up from that handle is automatically attenuated to `Rights::READ`. The user process **cannot** obtain write authority.
- **Write-Only Dropboxes**:
  A logging service can be given a directory capability with `Rights::WRITE` but not `Rights::READ`. It can create and append logs, but cannot read existing files or inspect other processes' data.
- **Instant Revocation (`OP_REVOKE`)**:
  Revoking a directory capability instantly invalidates all open file handles derived from that subtree across all child processes via generational counter advancement (`ERR_STALE_HANDLE`).

---

## 5. User Accounts as Authority Keyrings

Instead of a flat integer UID model with a dangerous god-mode `root` UID, AxiomOS models identities as **Authority Keyring Capsules**:

```
+-----------------------------------------------------------------------------+
|                         AUTHORITY KEYRING CAPSULE                           |
|                                                                             |
|  1. Identity Keypair:        Cryptographic public/private key anchor         |
|  2. Root Namespace:          Directory Capability to personal home vault     |
|  3. Hardware Capabilities:   Attenuated Console, Keyboard, and Network slots |
|  4. Resource Quotas:         Untyped physical memory budget (e.g. 64 MB)     |
+-----------------------------------------------------------------------------+
```

### Session Types:
1. **The Sovereign Root Session (`root@axiom`)**:
   - Held by Genesis (PID 1) upon boot.
   - Holds the unrestricted Root Storage capability, full physical memory authority, and master hardware endpoints.
2. **Standard User Sessions (`user@axiom`)**:
   - Spawns in an isolated VSpace and CSpace.
   - Holds a root directory capability pointing only to its `/home/<user>` vault.
   - Has no capability to touch other users' vaults or system configurations.
3. **Restricted Guest / Sandbox Sessions**:
   - Ephemeral in-memory RamFS namespace.
   - Zero persistent storage capabilities.
   - Wiped completely upon session termination.

---

## 6. Storage Implementation Roadmap

### Stage 1: In-Memory RamFS (`libaxiom/fs`) — Phase 7B
- Implemented as a lightweight, lock-free memory table.
- Stores system text files, initial configs, and shell utilities in RAM.
- Powers the interactive Genesis shell:
  - `help` — displays available commands and active capabilities.
  - `ls` — lists entries in current directory capability.
  - `cat <file>` — reads and displays file content.
  - `echo "text" > <file>` — writes data to file capability.
  - `status` — inspects memory, capability count, and scheduler ticks.

### Stage 2: Block-Backed Persistent Storage Driver — Phase 7C
- Driver runs in user mode as a capability server.
- Interacts with UEFI/AHCI/NVMe disk controllers via memory-mapped I/O (MMIO) capabilities.
- Mounts FAT32 or Axiom Native Block Vaults.

---

## 7. Developer Quick-Lookup Cheatsheet

### Looking Up and Reading a File in User Space
```rust
use libaxiom::capability::*;
use libaxiom::fs::{DirectoryCap, FileCap};

// 1. Resolve Root Directory Capability from Genesis CSpace (Slot 7)
let root_dir = DirectoryCap::from_slot(7);

// 2. Lookup child file "welcome.txt"
if let Ok(file) = root_dir.lookup("welcome.txt") {
    let mut buf = [0u8; 128];
    // 3. Read up to 128 bytes
    if let Ok(bytes_read) = file.read(0, &mut buf) {
        let text = core::str::from_utf8(&buf[..bytes_read]).unwrap();
        println!("File Content: {}", text);
    }
}
```

### Creating and Writing a File
```rust
// Create "config.sys" in writable directory
if let Ok(file) = root_dir.create("config.sys", false) {
    let payload = b"HOSTNAME=axiom-sovereign\nTHEME=obsidian_cyan\n";
    file.write(0, payload).expect("Failed to write config");
}
```
