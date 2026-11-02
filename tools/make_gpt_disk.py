#!/usr/bin/env python3
"""
AxiomOS Sovereign Disk Image Generator
Formats raw disk images with UEFI Protective MBR and standard GPT partition table.
Partitions:
  - ata0p1: Axiom Substrate (System / Read-Only binaries)
  - ata0p2: Axiom Vaults (Dynamic CapObject Persistent Storage)
"""

import sys, struct, zlib, uuid

def make_gpt_disk(output_path, total_sectors=32768): # 16 MB = 32768 sectors of 512 bytes
    disk = bytearray(total_sectors * 512)
    
    # 1. Protective MBR (LBA 0)
    disk[446:446+16] = struct.pack('<BBBBBBBBII', 
        0x00,              # Bootable (0x00 = non-bootable)
        0x00, 0x02, 0x00,  # Start CHS
        0xEE,              # OSType: GPT Protective MBR
        0xFF, 0xFF, 0xFF,  # End CHS
        1,                 # Starting LBA
        total_sectors - 1  # Size in sectors
    )
    disk[510:512] = b'\x55\xAA'

    # Partition Boundaries:
    # Partition 1 (ata0p1 - System Substrate): LBA 2048 .. 10239 (8192 sectors = 4 MB)
    # Partition 2 (ata0p2 - User Vaults Data): LBA 10240 .. 32734 (22495 sectors = ~11 MB)
    p1_start = 2048
    p1_end   = 10239
    p2_start = 10240
    p2_end   = total_sectors - 34

    # 2. Partition Entries (LBA 2..33, 128 entries of 128 bytes each = 16384 bytes)
    part_entries = bytearray(128 * 128)

    type_guid = uuid.UUID('a8104d5f-1234-5678-9abc-def012345678').bytes_le
    u1_guid = uuid.UUID('11111111-2222-3333-4444-555555555555').bytes_le
    u2_guid = uuid.UUID('66666666-7777-8888-9999-aaaaaaaaaaaa').bytes_le

    # Entry 0: ata0p1
    p1_name = 'ata0p1'.encode('utf-16le').ljust(72, b'\x00')
    part_entries[0:128] = struct.pack('<16s16sQQQ72s', type_guid, u1_guid, p1_start, p1_end, 0, p1_name)

    # Entry 1: ata0p2
    p2_name = 'ata0p2'.encode('utf-16le').ljust(72, b'\x00')
    part_entries[128:256] = struct.pack('<16s16sQQQ72s', type_guid, u2_guid, p2_start, p2_end, 0, p2_name)

    entries_crc = zlib.crc32(part_entries) & 0xFFFFFFFF

    # 3. Primary GPT Header (LBA 1, 92 bytes)
    disk_guid = uuid.UUID('a8a8a8a8-b8b8-c8c8-d8d8-e8e8e8e8e8e8').bytes_le
    header = bytearray(struct.pack('<8sIIIIQQQQ16sQIII',
        b'EFI PART',       # 8s: Signature
        0x00010000,         # I: Revision 1.0
        92,                 # I: Header size
        0,                  # I: Header CRC32 (zero during calculation)
        0,                  # I: Reserved
        1,                  # Q: Current LBA
        total_sectors - 1,  # Q: Backup LBA
        34,                 # Q: First usable LBA
        total_sectors - 34, # Q: Last usable LBA
        disk_guid,          # 16s: Disk GUID
        2,                  # Q: Partition entries starting LBA
        128,                # I: Number of partition entries
        128,                # I: Size of partition entry
        entries_crc         # I: CRC32 of partition entries
    ))
    header_crc = zlib.crc32(header[:92]) & 0xFFFFFFFF
    struct.pack_into('<I', header, 16, header_crc)

    disk[512:512+92] = header[:92]
    disk[1024:1024+len(part_entries)] = part_entries

    # 4. Backup GPT Header and Entries at end of disk
    backup_header = bytearray(header)
    struct.pack_into('<Q', backup_header, 24, total_sectors - 1)
    struct.pack_into('<Q', backup_header, 32, 1)
    struct.pack_into('<Q', backup_header, 72, total_sectors - 33)
    struct.pack_into('<I', backup_header, 16, 0)
    backup_crc = zlib.crc32(backup_header[:92]) & 0xFFFFFFFF
    struct.pack_into('<I', backup_header, 16, backup_crc)

    disk[(total_sectors - 33)*512 : (total_sectors - 1)*512] = part_entries
    disk[(total_sectors - 1)*512 : (total_sectors - 1)*512 + 92] = backup_header[:92]

    with open(output_path, 'wb') as f:
        f.write(disk)
    print(f"[GPT] Successfully forged sovereign partitioned disk: {output_path} ({total_sectors * 512 // 1024 // 1024} MB)")

if __name__ == '__main__':
    out_file = sys.argv[1] if len(sys.argv) > 1 else 'target/axiom_data.img'
    make_gpt_disk(out_file)
