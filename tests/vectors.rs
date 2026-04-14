use std::fs;
use std::path::PathBuf;

use crunchmania::{pack, parse_header, unpack, CrmError};

fn data_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/data")
}

fn read(name: &str) -> Vec<u8> {
    fs::read(data_dir().join(name)).expect("test data missing")
}

// Mirrors Python conftest.py ALL_FILES.
const STANDARD_FILES: &[&str] = &["test_C1.crm"];
const LZH_FILES: &[&str] = &[
    "13.bmp.crm2",
    "14.bmp",
    "15.bmp",
    "5.bmp",
    "6.bmp",
    "DECK2prefs",
    "GAME-OVER.DAWN",
    "pubfinale1",
    "test_C1_lz.crm",
    "vdp15",
];
const STANDARD_DELTA_FILES: &[&str] = &["test_C1_delta.crm"];
const LZH_DELTA_FILES: &[&str] = &["shnock", "test_C1_lz_delta.crm"];

fn all_files() -> Vec<&'static str> {
    STANDARD_FILES
        .iter()
        .chain(LZH_FILES)
        .chain(STANDARD_DELTA_FILES)
        .chain(LZH_DELTA_FILES)
        .copied()
        .collect()
}

#[test]
fn unpack_output_size_matches_header() {
    for name in all_files() {
        let data = read(name);
        let header = parse_header(&data).unwrap_or_else(|e| panic!("{name}: {e}"));
        let out = unpack(&data).unwrap_or_else(|e| panic!("{name}: {e}"));
        assert_eq!(
            out.len(),
            header.unpacked_size as usize,
            "{name}: unpack output size"
        );
    }
}

#[test]
fn all_c1_variants_identical() {
    let names = [
        "test_C1.crm",
        "test_C1_lz.crm",
        "test_C1_delta.crm",
        "test_C1_lz_delta.crm",
    ];
    let outputs: Vec<Vec<u8>> = names.iter().map(|n| unpack(&read(n)).unwrap()).collect();
    for i in 1..outputs.len() {
        assert_eq!(
            outputs[i], outputs[0],
            "{} differs from {}",
            names[i], names[0]
        );
    }
}

#[test]
fn header_rejects_too_short() {
    assert!(matches!(
        parse_header(b""),
        Err(CrmError::HeaderTooShort(_, _))
    ));
    assert!(matches!(
        parse_header(&[0u8; 5]),
        Err(CrmError::HeaderTooShort(_, _))
    ));
}

#[test]
fn header_rejects_bad_magic() {
    let mut buf = vec![0u8; 32];
    buf[0..4].copy_from_slice(b"WXYZ");
    assert!(matches!(parse_header(&buf), Err(CrmError::InvalidMagic(_))));
}

fn round_trip(raw: &[u8], sampled: bool) {
    let packed = pack(raw, sampled);
    let unpacked = unpack(&packed).expect("round-trip unpack");
    assert_eq!(unpacked, raw, "round-trip mismatch (sampled={sampled})");
}

#[test]
fn round_trip_synthetic_zeros() {
    round_trip(&vec![0u8; 1024], false);
    round_trip(&vec![0u8; 1024], true);
}

#[test]
fn round_trip_synthetic_repeating() {
    let data: Vec<u8> = (0..4096).map(|i| (i % 37) as u8).collect();
    round_trip(&data, false);
    round_trip(&data, true);
}

#[test]
fn round_trip_synthetic_pseudo_random() {
    // Tiny xorshift so the test is deterministic and dep-free.
    let mut s: u32 = 0xdeadbeef;
    let data: Vec<u8> = (0..8192)
        .map(|_| {
            s ^= s << 13;
            s ^= s >> 17;
            s ^= s << 5;
            s as u8
        })
        .collect();
    round_trip(&data, false);
    round_trip(&data, true);
}

#[test]
fn round_trip_single_byte() {
    round_trip(&[0x42], false);
    round_trip(&[0x42], true);
}

#[test]
fn round_trip_min_and_max_match_boundaries() {
    // MAX_MATCH = 278, MIN_MATCH = 2. Build a buffer with long runs.
    let mut data = vec![0xABu8; 300];
    data.extend_from_slice(&[0xAB, 0xCD]);
    data.extend(std::iter::repeat(0xEF).take(278));
    round_trip(&data, false);
}

#[test]
fn round_trip_real_test_files() {
    // For each real decompressible file, unpack then pack+unpack and assert.
    for name in all_files() {
        let data = read(name);
        let raw = unpack(&data).unwrap_or_else(|e| panic!("{name}: unpack: {e}"));
        // Pack as standard (no delta) and round-trip.
        let packed = pack(&raw, false);
        let again = unpack(&packed).unwrap_or_else(|e| panic!("{name}: re-unpack: {e}"));
        assert_eq!(again, raw, "{name}: round-trip");
    }
}
