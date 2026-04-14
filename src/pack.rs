use std::collections::HashMap;

use crate::constants::{DISTANCE_BITS, DISTANCE_OFFSETS, LENGTH_BITS, LENGTH_OFFSETS};

const MAX_DISTANCE: u32 = DISTANCE_OFFSETS[2] + (1u32 << DISTANCE_BITS[2]) - 1;
pub const MAX_MATCH: usize = 278;
pub const MIN_MATCH: usize = 2;
pub const CHAIN_LIMIT: usize = 4096;

/// Bit stream builder mirroring pack.py's `_Stream`.
///
/// Bits are appended in the decoder's read order. `finalize` emits bytes
/// 16..N as body (reversed) and the first 16 bits as the trailer's
/// `buf_content << 16` (shift = 0).
struct BitStream {
    bits: Vec<u8>,
}

impl BitStream {
    fn new() -> Self {
        Self { bits: Vec::new() }
    }

    #[inline]
    fn write_bit(&mut self, value: u32) {
        self.bits.push((value & 1) as u8);
    }

    #[inline]
    fn write_bits(&mut self, value: u32, count: u32) {
        for i in 0..count {
            self.bits.push(((value >> i) & 1) as u8);
        }
    }

    fn finalize(mut self) -> Vec<u8> {
        while self.bits.len() < 16 {
            self.bits.push(0);
        }
        while self.bits.len() % 8 != 0 {
            self.bits.push(0);
        }

        let mut acc16: u32 = 0;
        for i in 0..16 {
            acc16 |= (self.bits[i] as u32) << i;
        }
        let buf_content: u32 = acc16 << 16;

        let rest = &self.bits[16..];
        let nbytes = rest.len() / 8;
        let mut body = vec![0u8; nbytes];
        for k in 0..nbytes {
            let mut v: u8 = 0;
            for i in 0..8 {
                v |= rest[8 * k + i] << i;
            }
            body[nbytes - 1 - k] = v;
        }

        let mut out = body;
        out.extend_from_slice(&buf_content.to_be_bytes());
        out.extend_from_slice(&0u16.to_be_bytes());
        out
    }
}

fn vlc_index(value: u32, offset_table: &[u32]) -> usize {
    for i in (0..offset_table.len()).rev() {
        if value >= offset_table[i] {
            return i;
        }
    }
    0
}

fn encode_length_index(stream: &mut BitStream, index: usize) {
    match index {
        0 => stream.write_bit(0),
        1 => {
            stream.write_bit(1);
            stream.write_bit(0);
        }
        2 => {
            stream.write_bit(1);
            stream.write_bit(1);
            stream.write_bit(0);
        }
        _ => {
            stream.write_bit(1);
            stream.write_bit(1);
            stream.write_bit(1);
        }
    }
}

fn encode_distance_index(stream: &mut BitStream, index: usize) {
    match index {
        1 => stream.write_bit(0),
        0 => {
            stream.write_bit(1);
            stream.write_bit(0);
        }
        _ => {
            stream.write_bit(1);
            stream.write_bit(1);
        }
    }
}

fn encode_literal(stream: &mut BitStream, byte: u8) {
    stream.write_bit(1);
    stream.write_bits(byte as u32, 8);
}

fn encode_match(stream: &mut BitStream, length: u32, distance: u32) {
    stream.write_bit(0);

    // Length: count = vlc + 2; if count > 23 the decoder subtracts 1.
    // vlc=21 (count=23) is reserved for literal-escape.
    //   L in [2,22]: vlc = L - 2
    //   L in [23,278]: vlc = L - 1
    let length_vlc = if length <= 22 { length - 2 } else { length - 1 };
    let length_index = vlc_index(length_vlc, &LENGTH_OFFSETS);
    encode_length_index(stream, length_index);
    let extra = length_vlc - LENGTH_OFFSETS[length_index];
    stream.write_bits(extra, LENGTH_BITS[length_index]);

    let distance_index = vlc_index(distance, &DISTANCE_OFFSETS);
    encode_distance_index(stream, distance_index);
    let extra = distance - DISTANCE_OFFSETS[distance_index];
    stream.write_bits(extra, DISTANCE_BITS[distance_index]);
}

fn apply_inverse_delta(data: &[u8]) -> Vec<u8> {
    let mut out = vec![0u8; data.len()];
    let mut prev: u8 = 0;
    for i in 0..data.len() {
        out[i] = data[i].wrapping_sub(prev);
        prev = data[i];
    }
    out
}

/// Longest-match search at `pos`. Mirrors pack.py:_find_match exactly,
/// including CHAIN_LIMIT and reverse iteration semantics.
fn find_match(
    data: &[u8],
    pos: usize,
    hash_chains: &HashMap<u32, Vec<u32>>,
    size: usize,
) -> (usize, usize) {
    if pos + MIN_MATCH > size {
        return (0, 0);
    }
    if pos + 2 >= size {
        return (0, 0);
    }

    let h = ((data[pos] as u32) << 16) | ((data[pos + 1] as u32) << 8) | (data[pos + 2] as u32);
    let chain = match hash_chains.get(&h) {
        Some(c) if !c.is_empty() => c,
        _ => return (0, 0),
    };

    let mut best_len: usize = MIN_MATCH - 1;
    let mut best_dist: usize = 0;
    let max_len = core::cmp::min(size - pos, MAX_MATCH);

    for &cand in chain.iter().rev().take(CHAIN_LIMIT) {
        let candidate = cand as usize;
        // Python uses signed math here; candidate could be >= pos if duplicates exist.
        if candidate >= pos {
            continue;
        }
        let dist = pos - candidate;
        if dist > MAX_DISTANCE as usize {
            break;
        }

        // Cheap reject on the byte past current best.
        if data[candidate + best_len] != data[pos + best_len] {
            continue;
        }

        let mut length = 0usize;
        while length < max_len && data[pos + length] == data[candidate + length] {
            length += 1;
        }

        if length > best_len {
            best_len = length;
            best_dist = dist;
            if length >= max_len {
                break;
            }
        }
    }

    if best_len >= MIN_MATCH {
        (best_len, best_dist)
    } else {
        (0, 0)
    }
}

pub fn pack(data: &[u8], sampled: bool) -> Vec<u8> {
    let raw_size = data.len();

    let forward: Vec<u8> = if sampled {
        apply_inverse_delta(data)
    } else {
        data.to_vec()
    };

    // Decoder fills output top-down, so run LZ77 on the reversed buffer.
    let mut work = forward;
    work.reverse();

    let mut stream = BitStream::new();
    let mut hash_chains: HashMap<u32, Vec<u32>> = HashMap::new();
    let mut pos = 0usize;

    while pos < raw_size {
        let (length, distance) = find_match(&work, pos, &hash_chains, raw_size);
        let step = if length >= MIN_MATCH {
            encode_match(&mut stream, length as u32, distance as u32);
            length
        } else {
            encode_literal(&mut stream, work[pos]);
            1
        };

        let end_insert = core::cmp::min(pos + step, raw_size.saturating_sub(2));
        for i in pos..end_insert {
            let h = ((work[i] as u32) << 16) | ((work[i + 1] as u32) << 8) | (work[i + 2] as u32);
            hash_chains.entry(h).or_default().push(i as u32);
        }

        pos += step;
    }

    let packed_data = stream.finalize();
    let packed_size = packed_data.len() as u32;

    let magic: &[u8; 4] = if sampled { b"Crm!" } else { b"CrM!" };

    let mut out = Vec::with_capacity(14 + packed_data.len());
    out.extend_from_slice(magic);
    out.extend_from_slice(&0u16.to_be_bytes());
    out.extend_from_slice(&(raw_size as u32).to_be_bytes());
    out.extend_from_slice(&packed_size.to_be_bytes());
    out.extend_from_slice(&packed_data);
    out
}
