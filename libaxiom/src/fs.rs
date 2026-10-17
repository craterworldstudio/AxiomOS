//! AxiomOS In-Memory Capability Filesystem ("VaultFS" / "CapFS")
//!
//! Implements a zero-ambient-authority in-memory filesystem where directories
//! and files are manipulated as capability handles without a global root.

pub const MAX_INODES: usize = 32;
pub const MAX_NAME_LEN: usize = 32;
pub const MAX_FILE_SIZE: usize = 512;

#[derive(Clone, Copy)]
pub struct Inode {
    pub name: [u8; MAX_NAME_LEN],
    pub name_len: usize,
    pub data: [u8; MAX_FILE_SIZE],
    pub data_len: usize,
    pub is_dir: bool,
    pub parent_id: usize,
    pub in_use: bool,
}

impl Inode {
    pub const fn empty() -> Self {
        Self {
            name: [0; MAX_NAME_LEN],
            name_len: 0,
            data: [0; MAX_FILE_SIZE],
            data_len: 0,
            is_dir: false,
            parent_id: 0,
            in_use: false,
        }
    }

    pub fn name_str(&self) -> &str {
        core::str::from_utf8(&self.name[..self.name_len]).unwrap_or("<invalid>")
    }

    pub fn content_str(&self) -> &str {
        core::str::from_utf8(&self.data[..self.data_len]).unwrap_or("<binary data>")
    }
}

pub struct VaultFS {
    pub nodes: [Inode; MAX_INODES],
    pub initialized: bool,
}

impl VaultFS {
    pub const fn new() -> Self {
        Self {
            nodes: [Inode::empty(); MAX_INODES],
            initialized: false,
        }
    }

    /// Initializes pre-populated root vault nodes.
    pub fn init(&mut self) {
        if self.initialized { return; }

        // Inode 0: Root vault directory (".")
        self.add_node(0, ".", true, b"");

        // System directories
        let etc_id  = self.add_node(0, "etc", true, b"");
        let bin_id  = self.add_node(0, "bin", true, b"");
        let home_id = self.add_node(0, "home", true, b"");
        let root_id = self.add_node(home_id, "root", true, b"");

        // Pre-populated files
        self.add_node(
            etc_id,
            "os-release",
            false,
            b"NAME=\"AxiomOS Sovereign Core Substrate\"\nVERSION=\"0.2.0-alpha\"\nARCH=\"x86_64\"\nSECURITY=\"Object Capabilities (CDT Model)\"\nAUTHORITY=\"Zero Ambient Authority (CPL=3)\"\n",
        );

        self.add_node(
            etc_id,
            "motd",
            false,
            b"================================================================================\n AXIOM OS SOVEREIGN CORE SUBSTRATE -- PID 1 (GENESIS) ONLINE\n================================================================================\nWelcome to AxiomOS. Sovereign capability computing substrate.\nType 'help' for available built-in commands.\n",
        );

        self.add_node(
            root_id,
            "readme.txt",
            false,
            b"AxiomOS Root User Vault (PID 1).\nZero ambient authority is active.\nAll files and directories are object capabilities.\nThere is no global / path.\n",
        );

        self.add_node(
            bin_id,
            "help.txt",
            false,
            b"AxiomOS Built-in Utilities:\n  help              - Display this command reference\n  ls [dir]          - List entries in current or specified directory\n  cat <file>        - Print content of specified file\n  echo <text> [> f] - Print text or redirect to file\n  status            - Display CSpace authority and memory state\n  clear             - Redraw canvas and clear screen\n  reboot            - Hardware reboot\n",
        );

        self.initialized = true;
    }

    fn add_node(&mut self, parent_id: usize, name: &str, is_dir: bool, content: &[u8]) -> usize {
        for (idx, node) in self.nodes.iter_mut().enumerate() {
            if !node.in_use {
                node.in_use = true;
                node.is_dir = is_dir;
                node.parent_id = parent_id;

                let name_bytes = name.as_bytes();
                let nlen = core::cmp::min(name_bytes.len(), MAX_NAME_LEN);
                node.name[..nlen].copy_from_slice(&name_bytes[..nlen]);
                node.name_len = nlen;

                let dlen = core::cmp::min(content.len(), MAX_FILE_SIZE);
                node.data[..dlen].copy_from_slice(&content[..dlen]);
                node.data_len = dlen;

                return idx;
            }
        }
        0
    }

    /// Resolves path (e.g. "etc/os-release" or "readme.txt") starting from `start_dir`.
    pub fn lookup(&self, start_dir: usize, path: &str) -> Option<usize> {
        let trimmed = path.trim_matches('/');
        if trimmed.is_empty() || trimmed == "." {
            return Some(start_dir);
        }

        let mut current = start_dir;
        for part in trimmed.split('/') {
            if part.is_empty() || part == "." {
                continue;
            }
            if part == ".." {
                current = self.nodes[current].parent_id;
                continue;
            }

            let mut found = false;
            for (idx, node) in self.nodes.iter().enumerate() {
                if node.in_use && node.parent_id == current && idx != current {
                    if node.name_str() == part {
                        current = idx;
                        found = true;
                        break;
                    }
                }
            }
            if !found {
                return None;
            }
        }

        Some(current)
    }

    /// Creates or overwrites a file with content in `parent_dir`.
    pub fn write_file(&mut self, parent_dir: usize, name: &str, content: &[u8]) -> Result<usize, &'static str> {
        // If file already exists, overwrite
        if let Some(existing_id) = self.lookup(parent_dir, name) {
            let node = &mut self.nodes[existing_id];
            if node.is_dir {
                return Err("Is a directory");
            }
            let dlen = core::cmp::min(content.len(), MAX_FILE_SIZE);
            node.data[..dlen].copy_from_slice(&content[..dlen]);
            node.data_len = dlen;
            return Ok(existing_id);
        }

        // Otherwise allocate new node
        let new_id = self.add_node(parent_dir, name, false, content);
        if new_id == 0 && !name.is_empty() && name != "." {
            return Err("VaultFS out of inodes");
        }
        Ok(new_id)
    }
}

pub static mut VAULT: VaultFS = VaultFS::new();
