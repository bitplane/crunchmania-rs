/// LSB-first bit reader that reads bytes backward through packed data.
///
/// Direct port of `crunchmania/bitreader.py`. The decoder initialises the
/// accumulator from the 6-byte trailer (4-byte `buf_content` + 2-byte `shift`)
/// then pulls bytes from `end-1` downward, stopping at `start`.
pub struct BackwardBitReader<'a> {
    data: &'a [u8],
    start: usize,
    pos: isize,
    pub accumulator: u64,
    pub bits_left: u32,
}

impl<'a> BackwardBitReader<'a> {
    pub fn new(data: &'a [u8], start: usize, end: usize) -> Self {
        let buf_content = u32::from_be_bytes(data[end..end + 4].try_into().unwrap()) as u64;
        let shift = u16::from_be_bytes(data[end + 4..end + 6].try_into().unwrap()) as u32;

        let bits_left = shift + 16;
        let accumulator = buf_content >> 16u32.saturating_sub(shift);

        Self {
            data,
            start,
            pos: end as isize - 1,
            accumulator,
            bits_left,
        }
    }

    #[inline]
    fn read_byte(&mut self) -> u64 {
        if self.pos < self.start as isize {
            return 0;
        }
        let b = self.data[self.pos as usize] as u64;
        self.pos -= 1;
        b
    }

    #[inline]
    pub fn read_bits(&mut self, count: u32) -> u32 {
        while self.bits_left < count {
            self.accumulator |= self.read_byte() << self.bits_left;
            self.bits_left += 8;
        }
        let result = (self.accumulator & ((1u64 << count) - 1)) as u32;
        self.accumulator >>= count;
        self.bits_left -= count;
        result
    }

    #[inline]
    pub fn read_bit(&mut self) -> u32 {
        self.read_bits(1)
    }
}
