//! Matrix benchmark, mirroring `core_matrix.c`.

use crate::types::MatParams;
use crate::util::crc16;

#[inline]
fn matrix_big(x: i16) -> i16 {
    (0xf000 | x as i32) as i16
}

#[inline]
fn bit_extract(x: i32, from: u32, to: u32) -> i32 {
    (x >> from) & (!(0xffffffffu32 << to) as i32)
}

/// Benchmark function, mirroring `core_bench_matrix`.
pub fn core_bench_matrix(p: &mut MatParams, seed: i16, crc: u16) -> u16 {
    crc16(matrix_test(p, seed), crc)
}

/// Perform the matrix test sequence, mirroring `matrix_test`.
///
/// Operates on the stored matrices in place and restores `A` before returning,
/// exactly like the reference implementation. The returned CRC is narrowed to
/// `i16` because the C signature is `ee_s16` while the accumulator is `ee_u16`.
fn matrix_test(p: &mut MatParams, val: i16) -> i16 {
    let n = p.n;
    let clipval = matrix_big(val);

    matrix_add_const(n, &mut p.a, val); // make sure data changes
    matrix_mul_const(n, &mut p.c, &p.a, val);
    let mut crc = crc16(matrix_sum(n, &p.c, clipval), 0);
    matrix_mul_vect(n, &mut p.c, &p.a, &p.b);
    crc = crc16(matrix_sum(n, &p.c, clipval), crc);
    matrix_mul_matrix(n, &mut p.c, &p.a, &p.b);
    crc = crc16(matrix_sum(n, &p.c, clipval), crc);
    matrix_mul_matrix_bitextract(n, &mut p.c, &p.a, &p.b);
    crc = crc16(matrix_sum(n, &p.c, clipval), crc);

    matrix_add_const(n, &mut p.a, -val); // return matrix to initial value
    crc as i16
}

/// Initialize the memory block for matrix benchmarking, mirroring `core_init_matrix`.
pub fn core_init_matrix(blksize: usize, seed: i32, p: &mut MatParams) -> usize {
    let mut seed = if seed == 0 { 1 } else { seed };
    let mut order: i32 = 1;

    // Determine N: smallest count whose working set exceeds the block size.
    let blksize = blksize as u32;
    let mut i: u32 = 0;
    let mut j: u32 = 0;
    while j < blksize {
        i += 1;
        j = i.wrapping_mul(i).wrapping_mul(2).wrapping_mul(4);
    }
    let n = i.saturating_sub(1) as usize;

    let mut a = vec![0i16; n * n];
    let mut b = vec![0i16; n * n];

    for row in 0..n {
        for col in 0..n {
            seed = order.wrapping_mul(seed) % 65536;
            let mut val = ((seed + order) & 0x0ffff) as i16;
            b[row * n + col] = val;
            val = (((val as i32) + order) & 0x0ff) as i16;
            a[row * n + col] = val;
            order += 1;
        }
    }

    p.n = n;
    p.a = a;
    p.b = b;
    p.c = vec![0i32; n * n];
    n
}

/// Accumulate a value-dependent function over the result matrix, mirroring `matrix_sum`.
fn matrix_sum(n: usize, c: &[i32], clipval: i16) -> i16 {
    let mut tmp: i32 = 0;
    let mut prev: i32 = 0;
    let mut ret: i16 = 0;
    let clip = clipval as i32;
    for row in 0..n {
        for col in 0..n {
            let cur = c[row * n + col];
            tmp += cur;
            if tmp > clip {
                ret = ret.wrapping_add(10);
                tmp = 0;
            } else {
                ret = ret.wrapping_add(if cur > prev { 1 } else { 0 });
            }
            prev = cur;
        }
    }
    ret
}

/// Multiply a matrix by a constant, mirroring `matrix_mul_const`.
fn matrix_mul_const(n: usize, c: &mut [i32], a: &[i16], val: i16) {
    let val = val as i32;
    for (ci, &ai) in c[..n * n].iter_mut().zip(&a[..n * n]) {
        *ci = ai as i32 * val;
    }
}

/// Add a constant value to all elements of a matrix, mirroring `matrix_add_const`.
fn matrix_add_const(n: usize, a: &mut [i16], val: i16) {
    for ai in &mut a[..n * n] {
        *ai = ai.wrapping_add(val);
    }
}

/// Multiply a matrix by a vector, mirroring `matrix_mul_vect`.
fn matrix_mul_vect(n: usize, c: &mut [i32], a: &[i16], b: &[i16]) {
    for i in 0..n {
        let arow = &a[i * n..i * n + n];
        let mut acc: i32 = 0;
        for (j, &av) in arow.iter().enumerate() {
            acc = acc.wrapping_add(av as i32 * b[j] as i32);
        }
        c[i] = acc;
    }
}

/// Multiply a matrix by a matrix, mirroring `matrix_mul_matrix`.
///
/// The loop nesting is reordered to `i-k-j` so the inner loop walks two
/// contiguous rows. For each output element the terms are still accumulated over
/// `k` in the same order as the reference, so the result is bit-identical while
/// being far more cache- and vectorizer-friendly.
fn matrix_mul_matrix(n: usize, c: &mut [i32], a: &[i16], b: &[i16]) {
    for i in 0..n {
        let arow = &a[i * n..i * n + n];
        let crow = &mut c[i * n..i * n + n];
        for v in crow.iter_mut() {
            *v = 0;
        }
        for k in 0..n {
            let aik = arow[k] as i32;
            let brow = &b[k * n..k * n + n];
            for j in 0..n {
                crow[j] = crow[j].wrapping_add(aik * brow[j] as i32);
            }
        }
    }
}

/// Multiply a matrix by a matrix and extract bits from the products.
///
/// Uses the same `i-k-j` reordering as [`matrix_mul_matrix`], preserving the
/// per-element accumulation order.
fn matrix_mul_matrix_bitextract(n: usize, c: &mut [i32], a: &[i16], b: &[i16]) {
    for i in 0..n {
        let arow = &a[i * n..i * n + n];
        let crow = &mut c[i * n..i * n + n];
        for v in crow.iter_mut() {
            *v = 0;
        }
        for k in 0..n {
            let aik = arow[k] as i32;
            let brow = &b[k * n..k * n + n];
            for j in 0..n {
                let tmp = aik * brow[j] as i32;
                crow[j] = crow[j].wrapping_add(bit_extract(tmp, 2, 4) * bit_extract(tmp, 5, 7));
            }
        }
    }
}
