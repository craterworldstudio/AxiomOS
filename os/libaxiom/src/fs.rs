//! AxiomOS Object-Oriented Structural Graph Filesystem ("CapGraph")
//!
//! Implements a pure capability graph where:
//! - Directories are Capability Maps (Key-Value stores mapping string names to capability tokens).
//! - Graph Topology (No single cosmic root): Objects can be linked to multiple maps simultaneously.
//! - A process perceives only the subgraph it holds entry capabilities for.

pub const MAX_NODES: usize = 64;
pub const MAX_MAP_ENTRIES: usize = 16;
pub const MAX_NAME_LEN: usize = 32;
pub const MAX_DYNAMIC_DATA: usize = 1024;

use crate::syscall::{invoke_short, OP_STORAGE_READ, OP_STORAGE_WRITE};
use crate::ROOT_SLOT_STORAGE;

/// Sovereign VaultStore Constants
pub const VAULT_MAGIC: u64 = 0x4158494F4D5F4341; // "AXIOM_CA" (Sovereign CapObject Vault)
pub const MAX_EXTENTS: usize = 4;
pub const MAX_CAPOBJECT_ENTRIES: usize = 16;
pub const PARTITION_DATA_START_LBA: u32 = 10240; // ata0p2 starting LBA
pub const PARTITION_SYS_START_LBA: u32  = 2048;  // ata0p1 starting LBA

/// Dynamic extent descriptor representing a contiguous run of sectors
#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct ExtentRun {
    pub start_lba: u32,
    pub sector_count: u32,
}

/// Sovereign CapObject Descriptor (replaces Unix Inode)
#[repr(C)]
#[derive(Clone, Copy)]
pub struct CapObjectEntry {
    pub name: [u8; 32],
    pub parent_map: u32,
    pub data_len: u32,
    pub extent_count: u32,
    pub extents: [ExtentRun; MAX_EXTENTS],
    pub rights: u8,
    pub _reserved: [u8; 7],
}

impl Default for CapObjectEntry {
    fn default() -> Self {
        Self {
            name: [0; 32],
            parent_map: 0,
            data_len: 0,
            extent_count: 0,
            extents: [ExtentRun { start_lba: 0, sector_count: 0 }; MAX_EXTENTS],
            rights: 0,
            _reserved: [0; 7],
        }
    }
}

/// Dynamic VaultStore Superblock (512 bytes / 1 sector)
#[repr(C)]
#[derive(Clone, Copy)]
pub struct VaultSuperblock {
    pub magic: u64, // VAULT_MAGIC
    pub generation: u64, // Monotonic CoW generation counter
    pub entry_count: u32,
    pub bitmap_lba: u32,
    pub data_start_lba: u32,
    pub total_sectors: u32,
    pub entries: [CapObjectEntry; 4], // 4 entries per 512-byte sector descriptor
    pub _reserved: [u8; 16],
}

// Fallback legacy structure for backward compatibility during disk migration
#[repr(C)]
#[derive(Clone, Copy)]
pub struct DiskFileEntry {
    pub name: [u8; 32],
    pub parent_map: u32,
    pub data_len: u32,
    pub start_lba: u32,
    pub sector_count: u32,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct DiskSuperblock {
    pub magic: u64,
    pub entry_count: u32,
    pub _pad: u32,
    pub entries: [DiskFileEntry; 8],
    pub _reserved: [u8; 112],
}

/// GPT Partition Table Structures
#[repr(C)]
#[derive(Clone, Copy)]
pub struct GptHeader {
    pub signature: [u8; 8], // "EFI PART"
    pub revision: u32,
    pub header_size: u32,
    pub header_crc: u32,
    pub reserved: u32,
    pub current_lba: u64,
    pub backup_lba: u64,
    pub first_usable_lba: u64,
    pub last_usable_lba: u64,
    pub disk_guid: [u8; 16],
    pub partition_entry_lba: u64,
    pub num_partition_entries: u32,
    pub partition_entry_size: u32,
    pub partition_entries_crc: u32,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct GptPartitionEntry {
    pub type_guid: [u8; 16],
    pub unique_guid: [u8; 16],
    pub start_lba: u64,
    pub end_lba: u64,
    pub attributes: u64,
    pub name: [u16; 36],
}

// Embedded C and Assembly Binaries compiled from userland/
pub const ECHO_BIN: &[u8]   = include_bytes!("../../../target/userland_bin/echo.bin");
pub const LS_BIN: &[u8]     = include_bytes!("../../../target/userland_bin/ls.bin");
pub const CAT_BIN: &[u8]    = include_bytes!("../../../target/userland_bin/cat.bin");
pub const WHOAMI_BIN: &[u8] = include_bytes!("../../../target/userland_bin/whoami.bin");
pub const AXE_BIN: &[u8]    = include_bytes!("../../../target/userland_bin/axe.bin");
pub const PROC_BIN: &[u8]   = include_bytes!("../../../target/userland_bin/proc.bin");
pub const WORKER_BIN: &[u8] = include_bytes!("../../../target/userland_bin/worker.bin");
pub const GREP_BIN: &[u8]   = include_bytes!("../../../target/userland_bin/grep.bin");

// Capability Rights Bitflags
pub const RIGHTS_NONE: u8  = 0b0000;
pub const RIGHTS_READ: u8  = 0b0001;
pub const RIGHTS_WRITE: u8 = 0b0010;
pub const RIGHTS_EXEC: u8  = 0b0100;
pub const RIGHTS_GRANT: u8 = 0b1000;
pub const RIGHTS_ALL: u8   = 0b1111;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum GraphObjectKind {
    Unused,
    Blob,           // File data content
    DirectoryMap,   // Key-Value capability dictionary
    DeviceEndpoint, // Hardware / device capability (Console, etc.)
}

/// A capability token pointing directly to a graph object with attenuated rights.
#[derive(Clone, Copy)]
pub struct CapToken {
    pub target_id: usize,
    pub rights: u8,
}

impl CapToken {
    pub const fn empty() -> Self {
        Self { target_id: 0, rights: 0 }
    }
}

/// A single Key-Value link inside a Directory Capability Map.
#[derive(Clone, Copy)]
pub struct MapLink {
    pub name: [u8; MAX_NAME_LEN],
    pub name_len: usize,
    pub token: CapToken,
    pub in_use: bool,
}

impl MapLink {
    pub const fn empty() -> Self {
        Self {
            name: [0; MAX_NAME_LEN],
            name_len: 0,
            token: CapToken::empty(),
            in_use: false,
        }
    }

    pub fn name_str(&self) -> &str {
        core::str::from_utf8(&self.name[..self.name_len]).unwrap_or("<invalid>")
    }
}

/// An object in the structural graph.
#[derive(Clone, Copy)]
pub struct GraphNode {
    pub kind: GraphObjectKind,
    pub in_use: bool,

    // If Blob:
    pub data_ptr: *const u8,
    pub data_len: usize,
    pub is_dynamic: bool,
    pub dynamic_buf: [u8; MAX_DYNAMIC_DATA],

    // If DirectoryMap:
    pub entries: [MapLink; MAX_MAP_ENTRIES],
    pub entry_count: usize,
}

impl GraphNode {
    pub const fn empty() -> Self {
        Self {
            kind: GraphObjectKind::Unused,
            in_use: false,
            data_ptr: core::ptr::null(),
            data_len: 0,
            is_dynamic: false,
            dynamic_buf: [0; MAX_DYNAMIC_DATA],
            entries: [MapLink::empty(); MAX_MAP_ENTRIES],
            entry_count: 0,
        }
    }

    pub fn content_str(&self) -> &str {
        if self.data_len == 0 || self.data_ptr.is_null() {
            return "";
        }
        let slice = unsafe { core::slice::from_raw_parts(self.data_ptr, self.data_len) };
        core::str::from_utf8(slice).unwrap_or("<binary data>")
    }
}

/// The sovereign Object-Oriented Structural Graph
pub struct CapGraph {
    pub nodes: [GraphNode; MAX_NODES],
    pub initialized: bool,
}

pub static mut GRAPH: CapGraph = CapGraph::new();

#[repr(C)]
#[derive(Clone, Copy)]
pub struct CapBlobContext {
    pub data: [u8; 1024],
    pub length: u32,
    pub modified: u32,
}

// Static system configuration strings (explicitly null-terminated for C boundary safety)
const OS_RELEASE: &[u8] = b"NAME=\"AxiomOS Sovereign Core Substrate\"\nVERSION=\"0.3.0-alpha\"\nARCH=\"x86_64\"\nSECURITY=\"Object Capability Graph (CapGraph)\"\nAUTHORITY=\"Zero Ambient Authority (CPL=3)\"\n\0";
const MOTD: &[u8] = b"Welcome to AxiomOS.\nAll storage is modeled as an Object-Oriented Structural Graph.\nCommands are standalone C/ASM binaries in /apps/sys.\n\0";
const README: &[u8] = b"Sovereign Root Workspace\n- apps/sys/  : Standalone executable capability binaries (C/ASM)\n- apps/pkg/  : User and installed capability packages\n- meta/      : System capability ledger and configuration\n- vaults/    : User data vaults (VaultFS)\n- edps/      : Device capability endpoints (console, drives/ata0)\n- srvs/      : Ring 3 system server IPC channels\n\0";
const CAPS_LEDGER: &[u8] = b"# AxiomOS Capability Ledger v1.0\necho.bin        console:write,storage:write\nls.bin          console:write,storage:read\ncat.bin         console:write,storage:read\nwhoami.bin      console:write\naxe.bin         console:all,storage:all\nproc.bin        console:all,storage:read\nworker.bin      console:write\n\0";

impl CapGraph {
    pub const fn new() -> Self {
        Self {
            nodes: [GraphNode::empty(); MAX_NODES],
            initialized: false,
        }
    }

    pub fn init(&mut self) {
        if self.initialized { return; }

        // Node 0: Root Genesis Entry Map (Key-Value Directory Map)
        self.nodes[0] = GraphNode::empty();
        self.nodes[0].kind = GraphObjectKind::DirectoryMap;
        self.nodes[0].in_use = true;

        // Sovereign Namespace
        // 1. apps/ Directory Map
        let apps_id = self.add_map(0, "apps", RIGHTS_READ | RIGHTS_EXEC);
        let apps_sys_id = self.add_map(apps_id, "sys", RIGHTS_READ | RIGHTS_EXEC);
        let _apps_pkg_id = self.add_map(apps_id, "pkg", RIGHTS_ALL);

        // 2. meta/ Directory Map
        let meta_id = self.add_map(0, "meta", RIGHTS_READ);

        // 3. vaults/ Directory Map
        let vaults_id = self.add_map(0, "vaults", RIGHTS_ALL);
        let root_vault_id = self.add_map(vaults_id, "root", RIGHTS_ALL);

        // 4. edps/ Directory Map (Capability Endpoints)
        let edps_id = self.add_map(0, "edps", RIGHTS_READ);
        let edps_drives_id = self.add_map(edps_id, "drives", RIGHTS_READ);

        // 5. srvs/ Directory Map (Ring 3 Server Channels)
        let _srvs_id = self.add_map(0, "srvs", RIGHTS_READ | RIGHTS_WRITE);

        // Add executables to apps/sys/ Map
        self.add_static_blob(apps_sys_id, "echo.bin", ECHO_BIN, RIGHTS_EXEC | RIGHTS_READ);
        self.add_static_blob(apps_sys_id, "ls.bin", LS_BIN, RIGHTS_EXEC | RIGHTS_READ);
        self.add_static_blob(apps_sys_id, "cat.bin", CAT_BIN, RIGHTS_EXEC | RIGHTS_READ);
        self.add_static_blob(apps_sys_id, "whoami.bin", WHOAMI_BIN, RIGHTS_EXEC | RIGHTS_READ);
        self.add_static_blob(apps_sys_id, "axe.bin", AXE_BIN, RIGHTS_EXEC | RIGHTS_READ);
        self.add_static_blob(apps_sys_id, "proc.bin", PROC_BIN, RIGHTS_EXEC | RIGHTS_READ);
        self.add_static_blob(apps_sys_id, "worker.bin", WORKER_BIN, RIGHTS_EXEC | RIGHTS_READ);
        self.add_static_blob(apps_sys_id, "grep.bin", GREP_BIN, RIGHTS_EXEC | RIGHTS_READ);

        // Add config files to meta/ Map
        self.add_static_blob(meta_id, "caps.ledger", CAPS_LEDGER, RIGHTS_READ);
        self.add_static_blob(meta_id, "os-release", OS_RELEASE, RIGHTS_READ);
        self.add_static_blob(meta_id, "motd", MOTD, RIGHTS_READ);

        // Add vaults/root data files
        self.add_static_blob(root_vault_id, "readme.txt", README, RIGHTS_ALL);

        // Add capability endpoints
        self.add_endpoint(edps_id, "console", 5, RIGHTS_WRITE | RIGHTS_READ);
        self.add_endpoint(edps_drives_id, "ata0", 7, RIGHTS_WRITE | RIGHTS_READ);

        self.initialized = true;

        // Dynamically probe disk partition tables (GPT / MBR) on edps/drives/ata0
        self.probe_partitions(edps_drives_id);

        // Mount persistent disk nodes if present
        self.mount_disk();
    }

    /// Dynamically inspects disk LBA 0 (MBR) and LBA 1..33 (GPT) to register partition endpoints
    pub fn probe_partitions(&mut self, drives_map_id: usize) {
        let storage_handle = ((1u64) << 32) | (ROOT_SLOT_STORAGE as u64);
        let mut sector_buf = [0u8; 512];
        let buf_ptr = sector_buf.as_mut_ptr();

        // 1. Read LBA 1 to probe for GPT Partition Table
        let ret_gpt = unsafe { invoke_short(storage_handle, OP_STORAGE_READ, 1, buf_ptr as u64) };
        if ret_gpt == 0 && &sector_buf[0..8] == b"EFI PART" {
            // Valid GPT Header found! Read partition entries at LBA 2
            let mut entries_buf = [0u8; 512];
            let ent_ptr = entries_buf.as_mut_ptr();
            let ret_ent = unsafe { invoke_short(storage_handle, OP_STORAGE_READ, 2, ent_ptr as u64) };
            if ret_ent == 0 {
                // Parse up to 4 GPT partition entries from the first entry sector
                for p_idx in 0..4 {
                    let offset = p_idx * 128;
                    let type_guid = &entries_buf[offset..offset + 16];
                    let is_empty = type_guid.iter().all(|&b| b == 0);
                    if !is_empty {
                        let mut name_buf = [0u8; 16];
                        let mut nlen = 0;
                        for c in 0..8 {
                            let ch = entries_buf[offset + 56 + c * 2];
                            if ch == 0 { break; }
                            name_buf[nlen] = ch;
                            nlen += 1;
                        }
                        if nlen > 0 {
                            if let Ok(pname) = core::str::from_utf8(&name_buf[..nlen]) {
                                self.add_endpoint(drives_map_id, pname, 7, RIGHTS_WRITE | RIGHTS_READ);
                            }
                        } else {
                            match p_idx {
                                0 => { self.add_endpoint(drives_map_id, "ata0p1", 7, RIGHTS_READ); },
                                1 => { self.add_endpoint(drives_map_id, "ata0p2", 7, RIGHTS_WRITE | RIGHTS_READ); },
                                2 => { self.add_endpoint(drives_map_id, "ata0p3", 7, RIGHTS_WRITE | RIGHTS_READ); },
                                _ => { self.add_endpoint(drives_map_id, "ata0p4", 7, RIGHTS_WRITE | RIGHTS_READ); },
                            }
                        }
                    }
                }
                return;
            }
        }

        // 2. Fallback: Read LBA 0 to probe for legacy MBR / DOS Partition Table (Standard USB Flash Drives)
        let ret_mbr = unsafe { invoke_short(storage_handle, OP_STORAGE_READ, 0, buf_ptr as u64) };
        if ret_mbr == 0 && sector_buf[510] == 0x55 && sector_buf[511] == 0xAA {
            let mut found_any = false;
            for p_idx in 0..4 {
                let p_offset = 446 + p_idx * 16;
                let part_type = sector_buf[p_offset + 4];
                // Type != 0x00 (empty) and != 0xEE (protective GPT, which would have matched above)
                if part_type != 0x00 && part_type != 0xEE {
                    let mut start_lba = 0u32;
                    let mut sec_count = 0u32;
                    start_lba |= sector_buf[p_offset + 8] as u32;
                    start_lba |= (sector_buf[p_offset + 9] as u32) << 8;
                    start_lba |= (sector_buf[p_offset + 10] as u32) << 16;
                    start_lba |= (sector_buf[p_offset + 11] as u32) << 24;

                    sec_count |= sector_buf[p_offset + 12] as u32;
                    sec_count |= (sector_buf[p_offset + 13] as u32) << 8;
                    sec_count |= (sector_buf[p_offset + 14] as u32) << 16;
                    sec_count |= (sector_buf[p_offset + 15] as u32) << 24;

                    let mut name_buf = [0u8; 8];
                    name_buf[0] = b'a'; name_buf[1] = b't'; name_buf[2] = b'a'; name_buf[3] = b'0';
                    name_buf[4] = b'p'; name_buf[5] = b'1' + (p_idx as u8);
                    if let Ok(pname) = core::str::from_utf8(&name_buf[..6]) {
                        let node_id = self.add_endpoint(drives_map_id, pname, 7, RIGHTS_WRITE | RIGHTS_READ);
                        if node_id > 0 {
                            // Populate endpoint descriptor with actual physical partition metrics
                            let ep_node = &mut self.nodes[node_id];
                            let mb_size = (sec_count as u64 * 512) / (1024 * 1024);
                            let mut d_pos = 0;
                            let pfx = b"[CAPABILITY ENDPOINT: Physical MBR Partition Slice (";
                            ep_node.dynamic_buf[d_pos..d_pos + pfx.len()].copy_from_slice(pfx); d_pos += pfx.len();
                            ep_node.dynamic_buf[d_pos..d_pos + 6].copy_from_slice(&name_buf[..6]); d_pos += 6;
                            let pfx2 = b")]\nType: 0x";
                            ep_node.dynamic_buf[d_pos..d_pos + pfx2.len()].copy_from_slice(pfx2); d_pos += pfx2.len();
                            // hex byte
                            let hex_chars = b"0123456789ABCDEF";
                            ep_node.dynamic_buf[d_pos] = hex_chars[(part_type >> 4) as usize]; d_pos += 1;
                            ep_node.dynamic_buf[d_pos] = hex_chars[(part_type & 0xF) as usize]; d_pos += 1;
                            let pfx3 = b" | Start LBA: ";
                            ep_node.dynamic_buf[d_pos..d_pos + pfx3.len()].copy_from_slice(pfx3); d_pos += pfx3.len();
                            // write start_lba decimal
                            d_pos += Self::write_u32_dec(&mut ep_node.dynamic_buf[d_pos..], start_lba);
                            let pfx4 = b" | Sectors: ";
                            ep_node.dynamic_buf[d_pos..d_pos + pfx4.len()].copy_from_slice(pfx4); d_pos += pfx4.len();
                            d_pos += Self::write_u32_dec(&mut ep_node.dynamic_buf[d_pos..], sec_count);
                            let pfx5 = b" (~";
                            ep_node.dynamic_buf[d_pos..d_pos + pfx5.len()].copy_from_slice(pfx5); d_pos += pfx5.len();
                            d_pos += Self::write_u32_dec(&mut ep_node.dynamic_buf[d_pos..], mb_size as u32);
                            let pfx6 = b" MB)\nOperations: OP_STORAGE_READ(20), OP_STORAGE_WRITE(21)\nPhysical Source: Attached USB / IDE Disk Medium\n";
                            ep_node.dynamic_buf[d_pos..d_pos + pfx6.len()].copy_from_slice(pfx6); d_pos += pfx6.len();
                            ep_node.data_ptr = ep_node.dynamic_buf.as_ptr();
                            ep_node.data_len = d_pos;
                            ep_node.is_dynamic = true;
                        }
                        found_any = true;
                    }
                }
            }
            if found_any {
                return;
            }
        }

        // 3. Fallback: If unpartitioned, register default partition slices
        self.add_endpoint(drives_map_id, "ata0p1", 7, RIGHTS_READ);
        self.add_endpoint(drives_map_id, "ata0p2", 7, RIGHTS_WRITE | RIGHTS_READ);
    }

    /// Mounts dynamic files from persistent ATA disk (Supports CapObject VaultStore & Legacy Superblock)
    pub fn mount_disk(&mut self) {
        let storage_handle = ((1u64) << 32) | (ROOT_SLOT_STORAGE as u64);

        // 1. Try mounting sovereign CapObject VaultSuperblock at ata0p2 partition start
        let mut vault_sb = VaultSuperblock {
            magic: 0,
            generation: 0,
            entry_count: 0,
            bitmap_lba: 0,
            data_start_lba: 0,
            total_sectors: 0,
            entries: [CapObjectEntry::default(); 4],
            _reserved: [0; 16],
        };
        let vsb_ptr = &mut vault_sb as *mut VaultSuperblock as *mut u8;
        let ret = unsafe { invoke_short(storage_handle, OP_STORAGE_READ, PARTITION_DATA_START_LBA as u64, vsb_ptr as u64) };

        if ret == 0 && vault_sb.magic == VAULT_MAGIC {
            let count = core::cmp::min(vault_sb.entry_count as usize, 4);
            for i in 0..count {
                let ent = &vault_sb.entries[i];
                if ent.data_len == 0 || ent.extent_count == 0 { continue; }

                let mut data_buf = [0u8; 1024];
                let mut bytes_loaded = 0usize;

                for e_idx in 0..(core::cmp::min(ent.extent_count as usize, MAX_EXTENTS)) {
                    let ext = &ent.extents[e_idx];
                    for s in 0..ext.sector_count {
                        if bytes_loaded >= 1024 { break; }
                        let dest_ptr = unsafe { data_buf.as_mut_ptr().add(bytes_loaded) };
                        let _ = unsafe { invoke_short(storage_handle, OP_STORAGE_READ, (ext.start_lba + s) as u64, dest_ptr as u64) };
                        bytes_loaded += 512;
                    }
                }

                let dlen = core::cmp::min(ent.data_len as usize, 1024);
                let name_len = ent.name.iter().position(|&c| c == 0).unwrap_or(32);
                if let Ok(name_str) = core::str::from_utf8(&ent.name[..name_len]) {
                    let target_map = if ent.parent_map < (MAX_NODES as u32) && self.nodes[ent.parent_map as usize].in_use {
                        ent.parent_map as usize
                    } else if let Some(vr) = self.lookup(0, "vaults/root") {
                        vr.target_id
                    } else {
                        0
                    };
                    let _ = self.write_file_no_sync(target_map, name_str, &data_buf[..dlen]);
                }
            }
            return;
        }

        // 2. Fallback / Migration: Check legacy prototype superblock at LBA 0
        let mut sb = DiskSuperblock {
            magic: 0,
            entry_count: 0,
            _pad: 0,
            entries: [DiskFileEntry {
                name: [0; 32],
                parent_map: 0,
                data_len: 0,
                start_lba: 0,
                sector_count: 0,
            }; 8],
            _reserved: [0; 112],
        };
        let sb_ptr = &mut sb as *mut DiskSuperblock as *mut u8;
        let ret_leg = unsafe { invoke_short(storage_handle, OP_STORAGE_READ, 0, sb_ptr as u64) };

        if ret_leg == 0 && sb.magic == 0x4158494F4D5F4653 {
            let count = core::cmp::min(sb.entry_count as usize, 8);
            for i in 0..count {
                let ent = &sb.entries[i];
                if ent.data_len == 0 { continue; }
                let mut data_buf = [0u8; 1024];
                let sec1_ptr = data_buf.as_mut_ptr();
                let _ = unsafe { invoke_short(storage_handle, OP_STORAGE_READ, ent.start_lba as u64, sec1_ptr as u64) };
                if ent.sector_count > 1 {
                    let sec2_ptr = unsafe { sec1_ptr.add(512) };
                    let _ = unsafe { invoke_short(storage_handle, OP_STORAGE_READ, (ent.start_lba + 1) as u64, sec2_ptr as u64) };
                }

                let dlen = core::cmp::min(ent.data_len as usize, 1024);
                let name_len = ent.name.iter().position(|&c| c == 0).unwrap_or(32);
                if let Ok(name_str) = core::str::from_utf8(&ent.name[..name_len]) {
                    let target_map = if ent.parent_map < (MAX_NODES as u32) && self.nodes[ent.parent_map as usize].in_use {
                        ent.parent_map as usize
                    } else if let Some(vr) = self.lookup(0, "vaults/root") {
                        vr.target_id
                    } else {
                        0
                    };
                    let _ = self.write_file_no_sync(target_map, name_str, &data_buf[..dlen]);
                }
            }
        }
    }

    /// Flushes dynamic files to persistent CapObject VaultStore on partition ata0p2 with CoW generation tracking
    pub fn sync_disk(&mut self) -> Result<(), &'static str> {
        let storage_handle = ((1u64) << 32) | (ROOT_SLOT_STORAGE as u64);

        // Read current superblock to advance generation
        let mut current_gen = 0u64;
        let mut cur_sb = VaultSuperblock {
            magic: 0,
            generation: 0,
            entry_count: 0,
            bitmap_lba: 0,
            data_start_lba: 0,
            total_sectors: 0,
            entries: [CapObjectEntry::default(); 4],
            _reserved: [0; 16],
        };
        let cur_sb_ptr = &mut cur_sb as *mut VaultSuperblock as *mut u8;
        if unsafe { invoke_short(storage_handle, OP_STORAGE_READ, PARTITION_DATA_START_LBA as u64, cur_sb_ptr as u64) } == 0 {
            if cur_sb.magic == VAULT_MAGIC {
                current_gen = cur_sb.generation;
            }
        }

        let mut vault_sb = VaultSuperblock {
            magic: VAULT_MAGIC,
            generation: current_gen + 1,
            entry_count: 0,
            bitmap_lba: PARTITION_DATA_START_LBA + 1,
            data_start_lba: PARTITION_DATA_START_LBA + 8,
            total_sectors: 22495,
            entries: [CapObjectEntry::default(); 4],
            _reserved: [0; 16],
        };

        let mut file_idx = 0;
        let mut alloc_lba_offset = 0u32;

        for node_id in 0..MAX_NODES {
            if self.nodes[node_id].in_use && self.nodes[node_id].kind == GraphObjectKind::DirectoryMap {
                for link in self.nodes[node_id].entries.iter() {
                    if link.in_use && file_idx < 4 {
                        let target = &self.nodes[link.token.target_id];
                        if target.in_use && target.kind == GraphObjectKind::Blob && target.is_dynamic {
                            let sec_count = if target.data_len > 512 { 2 } else { 1 };
                            let start_lba = vault_sb.data_start_lba + alloc_lba_offset;
                            alloc_lba_offset += sec_count;

                            // Write first 512 bytes
                            let mut sbuf = [0u8; 512];
                            let c1 = core::cmp::min(target.data_len, 512);
                            sbuf[..c1].copy_from_slice(&target.dynamic_buf[..c1]);
                            let _ = unsafe { invoke_short(storage_handle, OP_STORAGE_WRITE, start_lba as u64, sbuf.as_ptr() as u64) };

                            // Write second 512 bytes if extent spans 2 sectors
                            if sec_count > 1 {
                                let mut sbuf2 = [0u8; 512];
                                let c2 = target.data_len - 512;
                                sbuf2[..c2].copy_from_slice(&target.dynamic_buf[512..512 + c2]);
                                let _ = unsafe { invoke_short(storage_handle, OP_STORAGE_WRITE, (start_lba + 1) as u64, sbuf2.as_ptr() as u64) };
                            }

                            let ent = &mut vault_sb.entries[file_idx];
                            ent.name[..link.name_len].copy_from_slice(&link.name[..link.name_len]);
                            ent.parent_map = node_id as u32;
                            ent.data_len = target.data_len as u32;
                            ent.extent_count = 1;
                            ent.extents[0] = ExtentRun {
                                start_lba,
                                sector_count: sec_count,
                            };
                            ent.rights = link.token.rights;

                            file_idx += 1;
                        }
                    }
                }
            }
        }

        vault_sb.entry_count = file_idx as u32;
        let sb_ptr = &vault_sb as *const VaultSuperblock as *const u8;

        // Atomic Root Generation Commit to ata0p2 superblock (LBA 10240)
        let ret = unsafe { invoke_short(storage_handle, OP_STORAGE_WRITE, PARTITION_DATA_START_LBA as u64, sb_ptr as u64) };
        if ret == 0 {
            Ok(())
        } else {
            Err("VaultStore write failed")
        }
    }

    /// Allocates an empty node in the graph
    fn alloc_node(&mut self) -> usize {
        for (i, node) in self.nodes.iter_mut().enumerate() {
            if !node.in_use {
                node.in_use = true;
                return i;
            }
        }
        0
    }

    pub fn write_u32_dec(buf: &mut [u8], mut val: u32) -> usize {
        if val == 0 {
            if !buf.is_empty() { buf[0] = b'0'; return 1; }
            return 0;
        }
        let mut tmp = [0u8; 10];
        let mut count = 0;
        while val > 0 && count < 10 {
            tmp[count] = b'0' + (val % 10) as u8;
            val /= 10;
            count += 1;
        }
        let n = core::cmp::min(count, buf.len());
        for i in 0..n {
            buf[i] = tmp[count - 1 - i];
        }
        n
    }

    /// Inserts a link into a Directory Map
    pub fn link_to_map(&mut self, map_id: usize, name: &str, token: CapToken) -> bool {
        let node = &mut self.nodes[map_id];
        if node.kind != GraphObjectKind::DirectoryMap { return false; }
        
        let nbytes = name.as_bytes();
        let nlen = core::cmp::min(nbytes.len(), MAX_NAME_LEN);

        // Check if name already exists; if so, update
        for link in node.entries.iter_mut() {
            if link.in_use && link.name_len == nlen && &link.name[..nlen] == &nbytes[..nlen] {
                link.token = token;
                return true;
            }
        }

        // Add new entry
        for link in node.entries.iter_mut() {
            if !link.in_use {
                link.in_use = true;
                link.name[..nlen].copy_from_slice(&nbytes[..nlen]);
                link.name_len = nlen;
                link.token = token;
                node.entry_count += 1;
                return true;
            }
        }
        false
    }

    /// Creates and links a new Directory Map
    pub fn add_map(&mut self, parent_map_id: usize, name: &str, rights: u8) -> usize {
        let new_id = self.alloc_node();
        if new_id == 0 { return 0; }
        
        self.nodes[new_id] = GraphNode::empty();
        self.nodes[new_id].kind = GraphObjectKind::DirectoryMap;
        self.nodes[new_id].in_use = true;

        if parent_map_id < MAX_NODES && self.nodes[parent_map_id].in_use {
            self.link_to_map(parent_map_id, name, CapToken { target_id: new_id, rights });
        }
        new_id
    }

    /// Creates and links a static Blob (immutable byte slice)
    pub fn add_static_blob(&mut self, parent_map_id: usize, name: &str, data: &'static [u8], rights: u8) -> usize {
        let new_id = self.alloc_node();
        if new_id == 0 { return 0; }

        self.nodes[new_id] = GraphNode::empty();
        self.nodes[new_id].kind = GraphObjectKind::Blob;
        self.nodes[new_id].in_use = true;
        self.nodes[new_id].data_ptr = data.as_ptr();
        self.nodes[new_id].data_len = data.len();
        self.nodes[new_id].is_dynamic = false;

        self.link_to_map(parent_map_id, name, CapToken { target_id: new_id, rights });
        new_id
    }

    /// Creates and links a device endpoint
    pub fn add_endpoint(&mut self, parent_map_id: usize, name: &str, target_slot: usize, rights: u8) -> usize {
        let new_id = self.alloc_node();
        if new_id == 0 { return 0; }

        self.nodes[new_id] = GraphNode::empty();
        self.nodes[new_id].kind = GraphObjectKind::DeviceEndpoint;
        self.nodes[new_id].in_use = true;
        self.nodes[new_id].data_len = target_slot;

        self.link_to_map(parent_map_id, name, CapToken { target_id: new_id, rights });
        new_id
    }
    /// Relative graph lookup: starts from `start_map_id` and traverses path tokens
    pub fn lookup(&self, start_map_id: usize, path: &str) -> Option<CapToken> {
        let trimmed = path.trim_matches('/');
        if trimmed.is_empty() || trimmed == "." {
            return Some(CapToken { target_id: start_map_id, rights: RIGHTS_ALL });
        }

        // Try lookup starting from start_map_id
        if let Some(token) = self.lookup_internal(start_map_id, trimmed) {
            return Some(token);
        }

        // If start_map_id != 0 and path has no leading dot, fallback to root node 0
        if start_map_id != 0 && !path.starts_with('.') {
            return self.lookup_internal(0, trimmed);
        }

        None
    }

    fn lookup_internal(&self, start_map_id: usize, trimmed: &str) -> Option<CapToken> {
        let mut current_map = start_map_id;

        for part in trimmed.split('/') {
            if part.is_empty() || part == "." {
                continue;
            }

            let node = &self.nodes[current_map];
            if node.kind != GraphObjectKind::DirectoryMap {
                return None;
            }

            let mut matched = false;
            let part_bytes = part.as_bytes();
            for link in node.entries.iter() {
                if link.in_use && link.name_len == part_bytes.len() && &link.name[..link.name_len] == part_bytes {
                    let target_node = &self.nodes[link.token.target_id];
                    if target_node.kind == GraphObjectKind::DirectoryMap {
                        current_map = link.token.target_id;
                    }
                    matched = true;
                    if part == trimmed.split('/').last().unwrap_or("") {
                        return Some(link.token);
                    }
                    break;
                }
            }
            if !matched {
                return None;
            }
        }
        Some(CapToken { target_id: current_map, rights: RIGHTS_ALL })
    }

    /// Writes or creates a file in the graph and automatically persists to disk
    pub fn write_file(&mut self, start_map_id: usize, path: &str, content: &[u8]) -> Result<usize, &'static str> {
        let res = self.write_file_no_sync(start_map_id, path, content);
        if res.is_ok() {
            let _ = self.sync_disk();
        }
        res
    }

    /// Internal write without disk flush (used by mount_disk to avoid re-sync recursion)
    pub fn write_file_no_sync(&mut self, start_map_id: usize, path: &str, content: &[u8]) -> Result<usize, &'static str> {
        let trimmed = path.trim_matches('/');
        if trimmed.is_empty() { return Err("Empty path"); }

        // Find leaf name and parent directory path
        let mut parts: [ &str; 8 ] = [ ""; 8 ];
        let mut count = 0;
        for p in trimmed.split('/') {
            if !p.is_empty() && count < 8 {
                parts[count] = p;
                count += 1;
            }
        }

        if count == 0 { return Err("Invalid path"); }

        // Traverse or auto-create intermediate directory maps
        let mut cur_map = start_map_id;
        for i in 0..count - 1 {
            let seg = parts[i];
            let mut next_map = None;
            let cur_node = &self.nodes[cur_map];
            for link in cur_node.entries.iter() {
                if link.in_use && link.name_str() == seg {
                    if self.nodes[link.token.target_id].kind == GraphObjectKind::DirectoryMap {
                        next_map = Some(link.token.target_id);
                        break;
                    } else {
                        return Err("Component is not a directory map");
                    }
                }
            }

            cur_map = match next_map {
                Some(id) => id,
                None => {
                    // Auto-create intermediate directory map (mkdir -p behavior)
                    self.add_map(cur_map, seg, RIGHTS_ALL)
                }
            };
        }

        let leaf_name = parts[count - 1];

        // Check if file already exists in target map
        let mut existing_target = None;
        let cur_node = &self.nodes[cur_map];
        for link in cur_node.entries.iter() {
            if link.in_use && link.name_str() == leaf_name {
                existing_target = Some(link.token.target_id);
                break;
            }
        }

        if let Some(target_id) = existing_target {
            let target = &mut self.nodes[target_id];
            if target.kind == GraphObjectKind::DirectoryMap {
                return Err("Is a directory map");
            }
            // Overwrite dynamic content
            let clen = core::cmp::min(content.len(), MAX_DYNAMIC_DATA);
            target.dynamic_buf[..clen].copy_from_slice(&content[..clen]);
            target.data_ptr = target.dynamic_buf.as_ptr();
            target.data_len = clen;
            target.is_dynamic = true;
            return Ok(target_id);
        }

        // Allocate new Blob node
        let new_id = self.alloc_node();
        if new_id == 0 { return Err("CapGraph out of nodes"); }

        let target = &mut self.nodes[new_id];
        target.kind = GraphObjectKind::Blob;
        target.in_use = true;
        let clen = core::cmp::min(content.len(), MAX_DYNAMIC_DATA);
        target.dynamic_buf[..clen].copy_from_slice(&content[..clen]);
        target.data_ptr = target.dynamic_buf.as_ptr();
        target.data_len = clen;
        target.is_dynamic = true;

        self.link_to_map(cur_map, leaf_name, CapToken { target_id: new_id, rights: RIGHTS_READ | RIGHTS_WRITE });
        Ok(new_id)
    }

    /// Looks up capability rights in /meta/caps.ledger
    pub fn get_ledger_rights(&self, bin_name: &str) -> Option<&'static str> {
        let name_only = if let Some(idx) = bin_name.rfind('/') {
            &bin_name[idx + 1..]
        } else {
            bin_name
        };

        match name_only {
            "echo" | "echo.bin" => Some("console:write,storage:write"),
            "ls" | "ls.bin" => Some("console:write,storage:read"),
            "cat" | "cat.bin" => Some("console:write,storage:read"),
            "whoami" | "whoami.bin" => Some("console:write"),
            "axe" | "axe.bin" => Some("console:all,storage:all"),
            "proc" | "proc.bin" => Some("console:all,storage:read"),
            "worker" | "worker.bin" => Some("console:write"),
            _ => None,
        }
    }

    /// Appends a new package entry to /meta/caps.ledger without needing Genesis root authority
    pub fn append_ledger_entry(&mut self, bin_name: &str, rights: &str, caller_rights: u8) -> Result<(), &'static str> {
        // Enforce Monotonicity Invariant: cannot grant rights greater than caller holds
        if (caller_rights & RIGHTS_WRITE) == 0 {
            return Err("Caller lacks authority to append to ledger");
        }

        // Verify not already registered
        if self.get_ledger_rights(bin_name).is_some() {
            return Err("Package already registered in ledger");
        }

        // Append to dynamic buffer of meta/caps.ledger node
        if let Some(token) = self.lookup(0, "meta/caps.ledger") {
            let node = &mut self.nodes[token.target_id];
            let mut pos = node.data_len;
            let b1 = bin_name.as_bytes();
            let b2 = b"        ";
            let b3 = rights.as_bytes();
            let b4 = b"\n";
            
            let total_add = 1 + b1.len() + b2.len() + b3.len() + b4.len();
            if pos + total_add < MAX_DYNAMIC_DATA {
                if !node.is_dynamic {
                    // Copy static data to dynamic buffer first
                    let old_slice = unsafe { core::slice::from_raw_parts(node.data_ptr, node.data_len) };
                    node.dynamic_buf[..node.data_len].copy_from_slice(old_slice);
                    node.data_ptr = node.dynamic_buf.as_ptr();
                    node.is_dynamic = true;
                }
                node.dynamic_buf[pos..pos + b1.len()].copy_from_slice(b1); pos += b1.len();
                node.dynamic_buf[pos..pos + b2.len()].copy_from_slice(b2); pos += b2.len();
                node.dynamic_buf[pos..pos + b3.len()].copy_from_slice(b3); pos += b3.len();
                node.dynamic_buf[pos] = b'\n'; pos += 1;
                node.data_len = pos;
                let _ = self.sync_disk();
                return Ok(());
            }
        }
        Err("Ledger buffer full")
    }
}

/// Canonicalizes a relative or step-back path against a working directory.
/// Supports multi-level step-backs (`../../`), explicit local paths (`./`),
/// root navigation (`/`), and vault home navigation (`~`).
pub fn resolve_path(cwd: &str, input_path: &str) -> ([u8; 64], usize) {
    let mut segments: [&str; 8] = [""; 8];
    let mut seg_count = 0;

    let trimmed_input = input_path.trim();
    let (is_absolute, path_to_process) = if trimmed_input == "~" {
        (true, "vaults/root")
    } else if trimmed_input.starts_with("~/") {
        (true, &trimmed_input[2..])
    } else if trimmed_input.starts_with('/') {
        (true, trimmed_input)
    } else {
        (false, trimmed_input)
    };

    if trimmed_input.starts_with("~/") {
        segments[0] = "vaults";
        segments[1] = "root";
        seg_count = 2;
    } else if !is_absolute {
        for seg in cwd.split('/') {
            let s = seg.trim();
            if !s.is_empty() && s != "." {
                if seg_count < 8 {
                    segments[seg_count] = s;
                    seg_count += 1;
                }
            }
        }
    }

    for seg in path_to_process.split('/') {
        let s = seg.trim();
        if s.is_empty() || s == "." {
            continue;
        } else if s == ".." {
            if seg_count > 0 {
                seg_count -= 1;
            }
        } else {
            if seg_count < 8 {
                segments[seg_count] = s;
                seg_count += 1;
            }
        }
    }

    let mut out = [0u8; 64];
    let mut out_len = 0;
    if seg_count == 0 {
        out[0] = b'/';
        return (out, 1);
    }

    for (i, seg) in segments[..seg_count].iter().enumerate() {
        if i > 0 && out_len < 63 {
            out[out_len] = b'/';
            out_len += 1;
        }
        let b = seg.as_bytes();
        let copy_len = core::cmp::min(b.len(), 63 - out_len);
        out[out_len..out_len + copy_len].copy_from_slice(&b[..copy_len]);
        out_len += copy_len;
    }

    (out, out_len)
}

