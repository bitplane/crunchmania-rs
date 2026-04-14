use crate::bitreader::BackwardBitReader;
use crate::constants::{DISTANCE_BITS, DISTANCE_OFFSETS, HEADER_SIZE, LENGTH_BITS, LENGTH_OFFSETS};
use crate::error::CrmError;
use crate::header::parse_header;

#[inline]
fn vlc_decode(
    reader: &mut BackwardBitReader,
    bits_table: &[u32],
    offset_table: &[u32],
    index: usize,
) -> u32 {
    reader.read_bits(bits_table[index]) + offset_table[index]
}

#[inline]
fn decode_length_index(reader: &mut BackwardBitReader) -> usize {
    if reader.read_bit() == 0 {
        return 0;
    }
    if reader.read_bit() == 0 {
        return 1;
    }
    if reader.read_bit() == 0 {
        return 2;
    }
    3
}

#[inline]
fn decode_distance_index(reader: &mut BackwardBitReader) -> usize {
    if reader.read_bit() == 0 {
        return 1;
    }
    if reader.read_bit() == 0 {
        return 0;
    }
    2
}

fn unpack_standard(reader: &mut BackwardBitReader, raw_size: usize) -> Vec<u8> {
    let mut output = vec![0u8; raw_size];
    let mut pos = raw_size;

    while pos > 0 {
        if reader.read_bit() == 1 {
            pos -= 1;
            output[pos] = reader.read_bits(8) as u8;
        } else {
            let length_index = decode_length_index(reader);
            let mut count = vlc_decode(reader, &LENGTH_BITS, &LENGTH_OFFSETS, length_index) + 2;

            if count == 23 {
                // literal escape
                let n = if reader.read_bit() == 1 {
                    reader.read_bits(5) + 15
                } else {
                    reader.read_bits(14) + 15
                };
                for _ in 0..n {
                    pos -= 1;
                    output[pos] = reader.read_bits(8) as u8;
                }
            } else {
                if count > 23 {
                    count -= 1;
                }
                let distance_index = decode_distance_index(reader);
                let distance =
                    vlc_decode(reader, &DISTANCE_BITS, &DISTANCE_OFFSETS, distance_index) as usize;

                for _ in 0..count {
                    pos -= 1;
                    output[pos] = output[pos + distance];
                }
            }
        }
    }

    output
}

fn read_huffman_table(
    reader: &mut BackwardBitReader,
    code_length: u32,
) -> Result<(Vec<i32>, u32), CrmError> {
    let max_depth = reader.read_bits(4);
    if max_depth == 0 {
        return Err(CrmError::ZeroHuffmanDepth);
    }

    let mut level_counts = Vec::with_capacity(max_depth as usize);
    for i in 0..max_depth {
        let bits = core::cmp::min(i + 1, code_length);
        level_counts.push(reader.read_bits(bits));
    }

    let table_size = 1usize << max_depth;
    let mut lookup = vec![-1i32; table_size];

    let mut code: u32 = 0;
    for depth_idx in 0..max_depth {
        let depth = depth_idx + 1;
        let pad_bits = max_depth - depth;
        for _ in 0..level_counts[depth_idx as usize] {
            let value = reader.read_bits(code_length);
            let canonical = code >> pad_bits;
            let reversed_code = bit_reverse(canonical, depth);
            for suffix in 0..(1u32 << pad_bits) {
                let idx = (reversed_code | (suffix << depth)) as usize;
                lookup[idx] = ((value << 4) | depth) as i32;
            }
            code += 1 << pad_bits;
        }
    }

    Ok((lookup, max_depth))
}

fn bit_reverse(mut value: u32, bits: u32) -> u32 {
    let mut result = 0u32;
    for _ in 0..bits {
        result = (result << 1) | (value & 1);
        value >>= 1;
    }
    result
}

#[inline]
fn decode_huffman(reader: &mut BackwardBitReader, lookup: &[i32], max_depth: u32) -> u32 {
    let bits = reader.read_bits(max_depth);
    let entry = lookup[bits as usize];
    let depth = (entry as u32) & 0xF;
    let unused = max_depth - depth;
    if unused != 0 {
        // Put back the unused high bits, matching unpack.py:135
        reader.accumulator = (reader.accumulator << unused) | (bits as u64 >> depth);
        reader.bits_left += unused;
    }
    (entry as u32) >> 4
}

fn unpack_lzh(reader: &mut BackwardBitReader, raw_size: usize) -> Result<Vec<u8>, CrmError> {
    let mut output = vec![0u8; raw_size];
    let mut pos = raw_size;

    loop {
        let (length_lookup, length_depth) = read_huffman_table(reader, 9)?;
        let (distance_lookup, distance_depth) = read_huffman_table(reader, 4)?;

        let items = reader.read_bits(16) + 1;
        for _ in 0..items {
            let value = decode_huffman(reader, &length_lookup, length_depth);

            if value & 0x100 != 0 {
                pos -= 1;
                output[pos] = (value & 0xFF) as u8;
            } else {
                let count = value + 3;

                let distance_bits = decode_huffman(reader, &distance_lookup, distance_depth);
                let distance = if distance_bits == 0 {
                    reader.read_bits(1) + 1
                } else {
                    (reader.read_bits(distance_bits) | (1 << distance_bits)) + 1
                } as usize;

                for _ in 0..count {
                    pos -= 1;
                    output[pos] = output[pos + distance];
                }
            }
        }

        if reader.read_bit() == 0 {
            break;
        }
    }

    Ok(output)
}

fn apply_delta(data: &mut [u8]) {
    let mut acc: u8 = 0;
    for b in data.iter_mut() {
        acc = acc.wrapping_add(*b);
        *b = acc;
    }
}

pub fn unpack(data: &[u8]) -> Result<Vec<u8>, CrmError> {
    let header = parse_header(data)?;

    let start = HEADER_SIZE;
    let end = HEADER_SIZE + header.packed_size as usize - 6;
    if end + 6 > data.len() || end < start {
        return Err(CrmError::Corrupt);
    }

    let mut reader = BackwardBitReader::new(data, start, end);

    let mut output = if header.is_lzh {
        unpack_lzh(&mut reader, header.unpacked_size as usize)?
    } else {
        unpack_standard(&mut reader, header.unpacked_size as usize)
    };

    if header.is_sampled {
        apply_delta(&mut output);
    }

    Ok(output)
}
