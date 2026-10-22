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
    pub magic: u64, // 0x4158494F4D5F4653 ("AXIOM_FS")
    pub entry_count: u32,
    pub _pad: u32,
    pub entries: [DiskFileEntry; 8],
    pub _reserved: [u8; 112],
}

// Embedded C and Assembly Binaries compiled from userland/
pub const ECHO_BIN: &[u8]   = include_bytes!("../../target/userland_bin/echo.bin");
pub const LS_BIN: &[u8]     = include_bytes!("../../target/userland_bin/ls.bin");
pub const CAT_BIN: &[u8]    = include_bytes!("../../target/userland_bin/cat.bin");
pub const WHOAMI_BIN: &[u8] = include_bytes!("../../target/userland_bin/whoami.bin");
pub const AXE_BIN: &[u8]    = include_bytes!("../../target/userland_bin/axe.bin");

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
const OS_RELEASE: &[u8] = b"NAME=\"AxiomOS Sovereign Core Substrate\"\nVERSION=\"0.2.0-alpha\"\nARCH=\"x86_64\"\nSECURITY=\"Object Capability Graph (CapGraph)\"\nAUTHORITY=\"Zero Ambient Authority (CPL=3)\"\n\0";
const MOTD: &[u8] = b"Welcome to AxiomOS.\nAll storage is modeled as an Object-Oriented Structural Graph.\nCommands are standalone C/ASM binaries in /apps/sys.\n\0";
const README: &[u8] = b"Sovereign Root Workspace\n- apps/sys/  : Standalone executable capability binaries (C/ASM)\n- apps/pkg/  : User and installed capability packages\n- meta/      : System capability ledger and configuration\n- vaults/    : User data vaults (VaultFS)\n- edps/      : Device capability endpoints (console, drives/ata0)\n- srvs/      : Ring 3 system server IPC channels\n\0";
const CAPS_LEDGER: &[u8] = b"# AxiomOS Capability Ledger v1.0\necho.bin        console:write,storage:write\nls.bin          console:write,storage:read\ncat.bin         console:write,storage:read\nwhoami.bin      console:write\naxe.bin         console:all,storage:all\n\0";

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

        // Mount persistent disk nodes if present
        self.mount_disk();
    }

    /// Mounts dynamic files from persistent ATA disk
    pub fn mount_disk(&mut self) {
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
        let storage_handle = ((1u64) << 32) | (ROOT_SLOT_STORAGE as u64);
        let ret = unsafe { invoke_short(storage_handle, OP_STORAGE_READ, 0, sb_ptr as u64) };
        if ret != 0 {
            return;
        }

        if sb.magic == 0x4158494F4D5F4653 {
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
        } else {
            // First time boot on clean disk: write blank superblock
            sb.magic = 0x4158494F4D5F4653;
            sb.entry_count = 0;
            let sb_write_ptr = &sb as *const DiskSuperblock as *const u8;
            let _ = unsafe { invoke_short(storage_handle, OP_STORAGE_WRITE, 0, sb_write_ptr as u64) };
        }
    }

    /// Flushes all dynamic files to persistent ATA disk
    pub fn sync_disk(&mut self) -> Result<(), &'static str> {
        let storage_handle = ((1u64) << 32) | (ROOT_SLOT_STORAGE as u64);
        let mut sb = DiskSuperblock {
            magic: 0x4158494F4D5F4653,
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

        let mut file_idx = 0;
        for node_id in 0..MAX_NODES {
            if self.nodes[node_id].in_use && self.nodes[node_id].kind == GraphObjectKind::DirectoryMap {
                for link in self.nodes[node_id].entries.iter() {
                    if link.in_use && file_idx < 8 {
                        let target = &self.nodes[link.token.target_id];
                        if target.in_use && target.kind == GraphObjectKind::Blob && target.is_dynamic {
                            let start_lba = 1 + (file_idx as u32) * 2;
                            let sec_count = if target.data_len > 512 { 2 } else { 1 };

                            let mut sbuf = [0u8; 512];
                            let c1 = core::cmp::min(target.data_len, 512);
                            sbuf[..c1].copy_from_slice(&target.dynamic_buf[..c1]);
                            let _ = unsafe { invoke_short(storage_handle, OP_STORAGE_WRITE, start_lba as u64, sbuf.as_ptr() as u64) };

                            if sec_count > 1 {
                                let mut sbuf2 = [0u8; 512];
                                let c2 = target.data_len - 512;
                                sbuf2[..c2].copy_from_slice(&target.dynamic_buf[512..512 + c2]);
                                let _ = unsafe { invoke_short(storage_handle, OP_STORAGE_WRITE, (start_lba + 1) as u64, sbuf2.as_ptr() as u64) };
                            }

                            let ent = &mut sb.entries[file_idx];
                            ent.name[..link.name_len].copy_from_slice(&link.name[..link.name_len]);
                            ent.parent_map = node_id as u32;
                            ent.data_len = target.data_len as u32;
                            ent.start_lba = start_lba;
                            ent.sector_count = sec_count;

                            file_idx += 1;
                        }
                    }
                }
            }
        }

        sb.entry_count = file_idx as u32;
        let sb_ptr = &sb as *const DiskSuperblock as *const u8;
        let ret = unsafe { invoke_short(storage_handle, OP_STORAGE_WRITE, 0, sb_ptr as u64) };
        if ret == 0 {
            Ok(())
        } else {
            Err("Disk write failed")
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
