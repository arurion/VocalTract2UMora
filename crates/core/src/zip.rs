// SPDX-License-Identifier: GPL-3.0-or-later
//! A minimal ZIP writer (stored, no compression), enough to bundle WAV files and oto.ini.

fn crc_table() -> [u32; 256] {
    let mut t = [0u32; 256];
    for n in 0..256u32 {
        let mut c = n;
        for _ in 0..8 { c = if c & 1 != 0 { 0xEDB8_8320 ^ (c >> 1) } else { c >> 1 }; }
        t[n as usize] = c;
    }
    t
}

pub fn crc32(data: &[u8]) -> u32 {
    let t = crc_table();
    let mut c = 0xFFFF_FFFFu32;
    for &b in data { c = t[((c ^ b as u32) & 0xFF) as usize] ^ (c >> 8); }
    c ^ 0xFFFF_FFFF
}

pub struct ZipWriter {
    out: Vec<u8>,
    central: Vec<u8>,
    count: u16,
}

impl Default for ZipWriter {
    fn default() -> Self { ZipWriter { out: vec![], central: vec![], count: 0 } }
}

// 2026-01-01 00:00:00 in DOS date / time format.
const DOS_DATE: u16 = ((2026 - 1980) << 9) | (1 << 5) | 1;
const DOS_TIME: u16 = 0;

impl ZipWriter {
    pub fn new() -> Self { Self::default() }

    /// Adds a file. File names are written as UTF-8.
    pub fn add(&mut self, name: &str, data: &[u8]) {
        let crc = crc32(data);
        let offset = self.out.len() as u32;
        let (n, size) = (name.as_bytes(), data.len() as u32);
        let flags: u16 = 0x0800; // UTF-8 names
        self.out.extend(0x0403_4b50u32.to_le_bytes());
        self.out.extend(20u16.to_le_bytes());
        self.out.extend(flags.to_le_bytes());
        self.out.extend(0u16.to_le_bytes()); // stored
        self.out.extend(DOS_TIME.to_le_bytes());
        self.out.extend(DOS_DATE.to_le_bytes());
        self.out.extend(crc.to_le_bytes());
        self.out.extend(size.to_le_bytes());
        self.out.extend(size.to_le_bytes());
        self.out.extend((n.len() as u16).to_le_bytes());
        self.out.extend(0u16.to_le_bytes());
        self.out.extend_from_slice(n);
        self.out.extend_from_slice(data);

        self.central.extend(0x0201_4b50u32.to_le_bytes());
        self.central.extend(20u16.to_le_bytes()); // made by
        self.central.extend(20u16.to_le_bytes()); // needed
        self.central.extend(flags.to_le_bytes());
        self.central.extend(0u16.to_le_bytes());
        self.central.extend(DOS_TIME.to_le_bytes());
        self.central.extend(DOS_DATE.to_le_bytes());
        self.central.extend(crc.to_le_bytes());
        self.central.extend(size.to_le_bytes());
        self.central.extend(size.to_le_bytes());
        self.central.extend((n.len() as u16).to_le_bytes());
        self.central.extend(0u16.to_le_bytes()); // extra
        self.central.extend(0u16.to_le_bytes()); // comment
        self.central.extend(0u16.to_le_bytes()); // disk
        self.central.extend(0u16.to_le_bytes()); // internal attrs
        self.central.extend(0u32.to_le_bytes()); // external attrs
        self.central.extend(offset.to_le_bytes());
        self.central.extend_from_slice(n);
        self.count += 1;
    }

    pub fn finish(mut self) -> Vec<u8> {
        let cd_offset = self.out.len() as u32;
        let cd_size = self.central.len() as u32;
        self.out.extend_from_slice(&self.central);
        self.out.extend(0x0605_4b50u32.to_le_bytes());
        self.out.extend(0u16.to_le_bytes());
        self.out.extend(0u16.to_le_bytes());
        self.out.extend(self.count.to_le_bytes());
        self.out.extend(self.count.to_le_bytes());
        self.out.extend(cd_size.to_le_bytes());
        self.out.extend(cd_offset.to_le_bytes());
        self.out.extend(0u16.to_le_bytes());
        self.out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn crc32_matches_the_standard_check_value() {
        assert_eq!(crc32(b"123456789"), 0xCBF4_3926);
        assert_eq!(crc32(b""), 0);
    }

    #[test]
    fn writes_a_readable_archive() {
        let mut z = ZipWriter::new();
        z.add("a.txt", b"hello");
        z.add("dir/b.bin", &[1, 2, 3, 4]);
        let bytes = z.finish();
        assert_eq!(&bytes[0..4], b"PK\x03\x04");
        // End of central directory: two entries.
        let e = bytes.len() - 22;
        assert_eq!(&bytes[e..e + 4], b"PK\x05\x06");
        assert_eq!(u16::from_le_bytes([bytes[e + 10], bytes[e + 11]]), 2);
        // The first local header points at the data we stored.
        assert_eq!(&bytes[30 + 5..30 + 5 + 5], b"hello");
        assert_eq!(u32::from_le_bytes([bytes[14], bytes[15], bytes[16], bytes[17]]), crc32(b"hello"));
    }
}
