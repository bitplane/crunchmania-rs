pub const HEADER_SIZE: usize = 14;

pub const LENGTH_BITS: [u32; 4] = [1, 2, 4, 8];
pub const LENGTH_OFFSETS: [u32; 4] = [0, 2, 6, 22];

pub const DISTANCE_BITS: [u32; 3] = [5, 9, 14];
pub const DISTANCE_OFFSETS: [u32; 3] = [0, 32, 544];

/// Returns (canonical_magic, is_lzh, is_sampled) for a 4-byte header prefix,
/// or None if the bytes are neither a canonical nor a known clone magic.
pub fn identify_magic(raw: [u8; 4]) -> Option<([u8; 4], bool, bool)> {
    // Canonical magics
    match &raw {
        b"CrM!" => return Some((*b"CrM!", false, false)),
        b"CrM2" => return Some((*b"CrM2", true, false)),
        b"Crm!" => return Some((*b"Crm!", false, true)),
        b"Crm2" => return Some((*b"Crm2", true, true)),
        _ => {}
    }
    // Clone magics → canonical
    let canonical: &[u8; 4] = match &raw {
        // 0x18051973 big-endian == [0x18, 0x05, 0x19, 0x73] — Fears
        [0x18, 0x05, 0x19, 0x73] => b"CrM2",
        b"CD\xb3\xb9" => b"CrM2", // BiFi 2
        b"Iron" => b"CrM2",       // Sun / TRSI
        b"MSS!" => b"CrM2",       // Infection / Mystic
        b"mss!" => b"Crm2",
        b"DCS!" => b"CrM!", // Sonic Attack / DualCrew-Shining
        _ => return None,
    };
    match canonical {
        b"CrM!" => Some((*canonical, false, false)),
        b"CrM2" => Some((*canonical, true, false)),
        b"Crm!" => Some((*canonical, false, true)),
        b"Crm2" => Some((*canonical, true, true)),
        _ => unreachable!(),
    }
}

/// All 4-byte magics the scanner should look at.
pub const ALL_MAGICS: &[[u8; 4]] = &[
    *b"CrM!",
    *b"CrM2",
    *b"Crm!",
    *b"Crm2",
    [0x18, 0x05, 0x19, 0x73],
    *b"CD\xb3\xb9",
    *b"Iron",
    *b"MSS!",
    *b"mss!",
    *b"DCS!",
];
