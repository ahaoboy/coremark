//! CRC service functions and seed parsing, mirroring `core_util.c`.

/// Update a 16b CRC with one byte, mirroring the C `crcu8`.
///
/// The update matches the reference implementation bit-for-bit, including the
/// final `crc &= 0x7fff` on the non-carry path.
#[inline]
pub fn crcu8(data: u8, crc: u16) -> u16 {
    let mut crc = crc;
    let mut data = data;
    for _ in 0..8 {
        let x16 = (data & 1) ^ ((crc & 1) as u8);
        data >>= 1;
        let carry = x16 == 1;
        if carry {
            crc ^= 0x4002;
        }
        crc >>= 1;
        if carry {
            crc |= 0x8000;
        } else {
            crc &= 0x7fff;
        }
    }
    crc
}

/// Update a 16b CRC with a 16b value.
#[inline]
pub fn crcu16(newval: u16, crc: u16) -> u16 {
    let crc = crcu8(newval as u8, crc);
    crcu8((newval >> 8) as u8, crc)
}

/// Update a 16b CRC with a signed 16b value.
#[inline]
pub fn crc16(newval: i16, crc: u16) -> u16 {
    crcu16(newval as u16, crc)
}

/// Update a 16b CRC with a 32b value.
#[inline]
pub fn crcu32(newval: u32, crc: u16) -> u16 {
    let crc = crc16(newval as i16, crc);
    crc16((newval >> 16) as i16, crc)
}

/// Parse a seed string the way the reference `parseval` does.
///
/// Supports an optional leading `-`, an optional `0x` prefix for hexadecimal
/// input and the `K`/`M` suffix multipliers. Returns 0 when no digits are found.
pub fn parseval(valstring: &str) -> i32 {
    let bytes = valstring.as_bytes();
    let mut pos = 0;
    let mut neg = 1i32;

    if pos < bytes.len() && bytes[pos] == b'-' {
        neg = -1;
        pos += 1;
    }

    let mut hexmode = false;
    if pos + 1 < bytes.len() && bytes[pos] == b'0' && bytes[pos + 1] == b'x' {
        hexmode = true;
        pos += 2;
    }

    let mut retval: i32 = 0;
    if hexmode {
        while pos < bytes.len() {
            let c = bytes[pos];
            let digit = if c.is_ascii_digit() {
                c - b'0'
            } else if (b'a'..=b'f').contains(&c) {
                10 + c - b'a'
            } else {
                break;
            };
            retval = retval.wrapping_mul(16).wrapping_add(digit as i32);
            pos += 1;
        }
    } else {
        while pos < bytes.len() && bytes[pos].is_ascii_digit() {
            retval = retval
                .wrapping_mul(10)
                .wrapping_add((bytes[pos] - b'0') as i32);
            pos += 1;
        }
    }

    if pos < bytes.len() && bytes[pos] == b'K' {
        retval = retval.wrapping_mul(1024);
    }
    if pos < bytes.len() && bytes[pos] == b'M' {
        retval = retval.wrapping_mul(1024 * 1024);
    }

    retval.wrapping_mul(neg)
}
