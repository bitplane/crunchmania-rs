use crate::constants::{identify_magic, HEADER_SIZE};
use crate::error::CrmError;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CrmHeader {
    pub magic: [u8; 4],
    pub is_lzh: bool,
    pub is_sampled: bool,
    pub unpacked_size: u32,
    pub packed_size: u32,
}

pub fn parse_header(data: &[u8]) -> Result<CrmHeader, CrmError> {
    if data.len() < HEADER_SIZE {
        return Err(CrmError::HeaderTooShort(data.len(), HEADER_SIZE));
    }

    let raw_magic: [u8; 4] = data[0..4].try_into().unwrap();
    let (magic, is_lzh, is_sampled) =
        identify_magic(raw_magic).ok_or(CrmError::InvalidMagic(raw_magic))?;

    // struct layout: 4s magic, H reserved, I unpacked, I packed (big-endian)
    let unpacked_size = u32::from_be_bytes(data[6..10].try_into().unwrap());
    let packed_size = u32::from_be_bytes(data[10..14].try_into().unwrap());

    if unpacked_size == 0 {
        return Err(CrmError::ZeroUnpackedSize);
    }
    if packed_size == 0 {
        return Err(CrmError::ZeroPackedSize);
    }

    let needed = HEADER_SIZE + packed_size as usize;
    if data.len() < needed {
        return Err(CrmError::DataTooShort(data.len(), needed));
    }

    Ok(CrmHeader {
        magic,
        is_lzh,
        is_sampled,
        unpacked_size,
        packed_size,
    })
}
