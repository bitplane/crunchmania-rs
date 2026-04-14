use thiserror::Error;

#[derive(Debug, Error)]
pub enum CrmError {
    #[error("data too short for CrM header: {0} < {1}")]
    HeaderTooShort(usize, usize),
    #[error("invalid CrM magic: {0:?}")]
    InvalidMagic([u8; 4]),
    #[error("unpacked size is zero")]
    ZeroUnpackedSize,
    #[error("packed size is zero")]
    ZeroPackedSize,
    #[error("data too short: {0} < {1}")]
    DataTooShort(usize, usize),
    #[error("LZH huffman table has zero max depth")]
    ZeroHuffmanDepth,
    #[error("corrupt stream")]
    Corrupt,
}
