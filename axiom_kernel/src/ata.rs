//! AxiomOS Freestanding ATA PIO Storage Driver
//! Direct hardware port I/O on Primary IDE Controller (0x1F0..0x1F7)

const ATA_PORT_DATA: u16    = 0x1F0;
const ATA_PORT_ERROR: u16   = 0x1F1;
const ATA_PORT_COUNT: u16   = 0x1F2;
const ATA_PORT_LBA_LO: u16  = 0x1F3;
const ATA_PORT_LBA_MID: u16 = 0x1F4;
const ATA_PORT_LBA_HI: u16  = 0x1F5;
const ATA_PORT_DRIVE: u16   = 0x1F6;
const ATA_PORT_STATUS: u16  = 0x1F7;
const ATA_PORT_CMD: u16     = 0x1F7;

const ATA_CMD_READ: u8      = 0x20;
const ATA_CMD_WRITE: u8     = 0x30;
const ATA_CMD_FLUSH: u8     = 0xE7;

const STATUS_BSY: u8        = 0x80;
const STATUS_DRDY: u8       = 0x40;
const STATUS_DRQ: u8        = 0x08;
const STATUS_ERR: u8        = 0x01;

/// Waits for Busy (BSY) bit to clear
unsafe fn wait_not_busy() -> Result<u8, ()> {
    for _ in 0..100_000 {
        let status: u8;
        core::arch::asm!("in al, dx", in("dx") ATA_PORT_STATUS, out("al") status, options(nomem, nostack));
        if (status & STATUS_BSY) == 0 {
            return Ok(status);
        }
    }
    Err(())
}

/// Waits for Data Request (DRQ) ready or returns error if ERR set
unsafe fn wait_drq() -> Result<u8, ()> {
    for _ in 0..100_000 {
        let status: u8;
        core::arch::asm!("in al, dx", in("dx") ATA_PORT_STATUS, out("al") status, options(nomem, nostack));
        if (status & STATUS_ERR) != 0 {
            return Err(());
        }
        if (status & STATUS_DRQ) != 0 {
            return Ok(status);
        }
    }
    Err(())
}

/// Reads a 512-byte sector from ATA disk using 28-bit LBA addressing
pub unsafe fn read_sector(drive: u8, lba: u32, buf: *mut u8) -> Result<(), ()> {
    if buf.is_null() {
        return Err(());
    }

    wait_not_busy()?;

    // 0xE0 = Master (drive 0), 0xF0 = Slave (drive 1)
    let drive_select = 0xE0u8 | ((drive & 1) << 4) | (((lba >> 24) & 0x0F) as u8);
    core::arch::asm!("out dx, al", in("dx") ATA_PORT_DRIVE, in("al") drive_select, options(nomem, nostack));

    // Wait 400ns by reading status 4 times (standard ATA delay)
    for _ in 0..4 {
        let _: u8;
        core::arch::asm!("in al, dx", in("dx") ATA_PORT_STATUS, out("al") _, options(nomem, nostack));
    }

    core::arch::asm!("out dx, al", in("dx") ATA_PORT_COUNT, in("al") 1u8, options(nomem, nostack));
    core::arch::asm!("out dx, al", in("dx") ATA_PORT_LBA_LO, in("al") (lba & 0xFF) as u8, options(nomem, nostack));
    core::arch::asm!("out dx, al", in("dx") ATA_PORT_LBA_MID, in("al") ((lba >> 8) & 0xFF) as u8, options(nomem, nostack));
    core::arch::asm!("out dx, al", in("dx") ATA_PORT_LBA_HI, in("al") ((lba >> 16) & 0xFF) as u8, options(nomem, nostack));
    core::arch::asm!("out dx, al", in("dx") ATA_PORT_CMD, in("al") ATA_CMD_READ, options(nomem, nostack));

    wait_drq()?;

    // Read 256 16-bit words (512 bytes)
    let word_buf = buf as *mut u16;
    for i in 0..256 {
        let word: u16;
        core::arch::asm!("in ax, dx", in("dx") ATA_PORT_DATA, out("ax") word, options(nomem, nostack));
        core::ptr::write_unaligned(word_buf.add(i), word);
    }

    Ok(())
}

/// Writes a 512-byte sector to ATA disk using 28-bit LBA addressing
pub unsafe fn write_sector(drive: u8, lba: u32, buf: *const u8) -> Result<(), ()> {
    if buf.is_null() {
        return Err(());
    }

    wait_not_busy()?;

    let drive_select = 0xE0u8 | ((drive & 1) << 4) | (((lba >> 24) & 0x0F) as u8);
    core::arch::asm!("out dx, al", in("dx") ATA_PORT_DRIVE, in("al") drive_select, options(nomem, nostack));

    for _ in 0..4 {
        let _: u8;
        core::arch::asm!("in al, dx", in("dx") ATA_PORT_STATUS, out("al") _, options(nomem, nostack));
    }

    core::arch::asm!("out dx, al", in("dx") ATA_PORT_COUNT, in("al") 1u8, options(nomem, nostack));
    core::arch::asm!("out dx, al", in("dx") ATA_PORT_LBA_LO, in("al") (lba & 0xFF) as u8, options(nomem, nostack));
    core::arch::asm!("out dx, al", in("dx") ATA_PORT_LBA_MID, in("al") ((lba >> 8) & 0xFF) as u8, options(nomem, nostack));
    core::arch::asm!("out dx, al", in("dx") ATA_PORT_LBA_HI, in("al") ((lba >> 16) & 0xFF) as u8, options(nomem, nostack));
    core::arch::asm!("out dx, al", in("dx") ATA_PORT_CMD, in("al") ATA_CMD_WRITE, options(nomem, nostack));

    wait_drq()?;

    // Write 256 16-bit words (512 bytes)
    let word_buf = buf as *const u16;
    for i in 0..256 {
        let word = core::ptr::read_unaligned(word_buf.add(i));
        core::arch::asm!("out dx, ax", in("dx") ATA_PORT_DATA, in("ax") word, options(nomem, nostack));
    }

    // Flush cache
    core::arch::asm!("out dx, al", in("dx") ATA_PORT_CMD, in("al") ATA_CMD_FLUSH, options(nomem, nostack));
    wait_not_busy()?;

    Ok(())
}
