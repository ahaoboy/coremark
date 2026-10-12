//! State machine benchmark, mirroring `core_state.c`.

use crate::util::crcu32;

/// All possible states of the input classifier FSM, mirroring `core_state_e`.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum CoreState {
    Start = 0,
    Invalid,
    S1,
    S2,
    Int,
    Float,
    Exponent,
    Scientific,
}

impl CoreState {
    pub const COUNT: usize = 8;
}

// Default initialization patterns.
const INT_PAT: [&[u8]; 4] = [b"5012", b"1234", b"-874", b"+122"];
const FLOAT_PAT: [&[u8]; 4] = [b"35.54400", b".1234500", b"-110.700", b"+0.64400"];
const SCI_PAT: [&[u8]; 4] = [b"5.500e+3", b"-.123e-2", b"-87e+832", b"+0.6e-12"];
const ERR_PAT: [&[u8]; 4] = [b"T0.3e-1F", b"-T.T++Tq", b"1T3.4e4z", b"34.0e-T^"];

#[inline]
fn ee_isdigit(c: u8) -> bool {
    c.is_ascii_digit()
}

/// Advance the FSM from `str` until a token boundary or invalid input.
///
/// Returns the end state and the number of bytes consumed, mirroring the way
/// the C version updates the input pointer through `*instr`.
fn core_state_transition(
    instr: &[u8],
    transition_count: &mut [u32; CoreState::COUNT],
) -> (CoreState, usize) {
    let mut state = CoreState::Start;
    let mut i = 0usize;

    while i < instr.len() && instr[i] != 0 && state != CoreState::Invalid {
        let next_symbol = instr[i];
        if next_symbol == b',' {
            // End of this input.
            i += 1;
            break;
        }
        match state {
            CoreState::Start => {
                if ee_isdigit(next_symbol) {
                    state = CoreState::Int;
                } else if next_symbol == b'+' || next_symbol == b'-' {
                    state = CoreState::S1;
                } else if next_symbol == b'.' {
                    state = CoreState::Float;
                } else {
                    state = CoreState::Invalid;
                    transition_count[CoreState::Invalid as usize] += 1;
                }
                transition_count[CoreState::Start as usize] += 1;
            }
            CoreState::S1 => {
                if ee_isdigit(next_symbol) {
                    state = CoreState::Int;
                    transition_count[CoreState::S1 as usize] += 1;
                } else if next_symbol == b'.' {
                    state = CoreState::Float;
                    transition_count[CoreState::S1 as usize] += 1;
                } else {
                    state = CoreState::Invalid;
                    transition_count[CoreState::S1 as usize] += 1;
                }
            }
            CoreState::Int => {
                if next_symbol == b'.' {
                    state = CoreState::Float;
                    transition_count[CoreState::Int as usize] += 1;
                } else if !ee_isdigit(next_symbol) {
                    state = CoreState::Invalid;
                    transition_count[CoreState::Int as usize] += 1;
                }
            }
            CoreState::Float => {
                if next_symbol == b'E' || next_symbol == b'e' {
                    state = CoreState::S2;
                    transition_count[CoreState::Float as usize] += 1;
                } else if !ee_isdigit(next_symbol) {
                    state = CoreState::Invalid;
                    transition_count[CoreState::Float as usize] += 1;
                }
            }
            CoreState::S2 => {
                if next_symbol == b'+' || next_symbol == b'-' {
                    state = CoreState::Exponent;
                    transition_count[CoreState::S2 as usize] += 1;
                } else {
                    state = CoreState::Invalid;
                    transition_count[CoreState::S2 as usize] += 1;
                }
            }
            CoreState::Exponent => {
                if ee_isdigit(next_symbol) {
                    state = CoreState::Scientific;
                    transition_count[CoreState::Exponent as usize] += 1;
                } else {
                    state = CoreState::Invalid;
                    transition_count[CoreState::Exponent as usize] += 1;
                }
            }
            CoreState::Scientific => {
                if !ee_isdigit(next_symbol) {
                    state = CoreState::Invalid;
                    transition_count[CoreState::Invalid as usize] += 1;
                }
            }
            // The loop exits as soon as the state becomes invalid, so this arm
            // is never reached with an `Invalid` state.
            CoreState::Invalid => unreachable!("invalid state ends the scan"),
        }
        i += 1;
    }

    (state, i)
}

/// Initialize the input data for the state machine benchmark.
///
/// Returns a zero-terminated buffer of `size` bytes. At least one byte is
/// produced so the scanning benchmark always finds a terminator, which keeps
/// degenerate (zero-sized) configurations well defined.
pub fn core_init_state(size: usize, seed: i16) -> Vec<u8> {
    let mut p = vec![0u8; size.max(1)];
    let pattern_end = p.len() - 1;

    let mut total = 0usize;
    let mut next = 0usize;
    let mut buf: &[u8] = b"";
    let mut seed = seed;

    while total + next + 1 < pattern_end {
        if next > 0 {
            p[total..total + next].copy_from_slice(&buf[..next]);
            p[total + next] = b',';
            total += next + 1;
        }
        seed = seed.wrapping_add(1);
        let pattern_idx = ((seed >> 3) & 0x3) as usize;
        match seed & 0x7 {
            0..=2 => {
                buf = INT_PAT[pattern_idx];
                next = 4;
            }
            3..=4 => {
                buf = FLOAT_PAT[pattern_idx];
                next = 8;
            }
            5..=6 => {
                buf = SCI_PAT[pattern_idx];
                next = 8;
            }
            _ => {
                buf = ERR_PAT[pattern_idx];
                next = 8;
            }
        }
    }

    // The remaining bytes stay zero, matching the reference fill loop.
    p
}

/// Run the state machine benchmark, mirroring `core_bench_state`.
pub fn core_bench_state(
    blksize: usize,
    memblock: &mut [u8],
    seed1: i16,
    seed2: i16,
    step: i16,
    crc: u16,
) -> u16 {
    let mut final_counts = [0u32; CoreState::COUNT];
    let mut track_counts = [0u32; CoreState::COUNT];

    // Run the state machine over the input.
    {
        let mut pos = 0usize;
        while memblock[pos] != 0 {
            let (fstate, consumed) = core_state_transition(&memblock[pos..], &mut track_counts);
            final_counts[fstate as usize] += 1;
            pos += consumed;
        }
    }

    {
        // Insert some corruption. When `step` is not positive the C version's
        // pointer arithmetic never advances, so nothing is modified.
        let step = step as isize;
        if step > 0 {
            let mut p = 0usize;
            while p < blksize {
                if memblock[p] != b',' {
                    memblock[p] ^= seed1 as u8;
                }
                p = p.wrapping_add(step as usize);
            }
        }
    }

    // Run the state machine over the input again.
    {
        let mut pos = 0usize;
        while memblock[pos] != 0 {
            let (fstate, consumed) = core_state_transition(&memblock[pos..], &mut track_counts);
            final_counts[fstate as usize] += 1;
            pos += consumed;
        }
    }

    {
        // Undo the corruption. Again, only positive steps advance.
        let step = step as isize;
        if step > 0 {
            let mut p = 0usize;
            while p < blksize {
                if memblock[p] != b',' {
                    memblock[p] ^= seed2 as u8;
                }
                p = p.wrapping_add(step as usize);
            }
        }
    }

    let mut crc = crc;
    for i in 0..CoreState::COUNT {
        crc = crcu32(final_counts[i], crc);
        crc = crcu32(track_counts[i], crc);
    }
    crc
}
