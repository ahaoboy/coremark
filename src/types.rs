//! Shared constants, types and working state for the CoreMark benchmark.
//!
//! This mirrors `coremark.h` from the reference C implementation.

/// Default total size for the data the algorithms operate on.
pub const TOTAL_DATA_SIZE: usize = 2 * 1000;

// Algorithm identifiers (bitmask), matching `ID_LIST`/`ID_MATRIX`/`ID_STATE`.
pub const ID_LIST: u32 = 1 << 0;
pub const ID_MATRIX: u32 = 1 << 1;
pub const ID_STATE: u32 = 1 << 2;
pub const ALL_ALGORITHMS_MASK: u32 = ID_LIST | ID_MATRIX | ID_STATE;
pub const NUM_ALGORITHMS: usize = 3;

/// A single linked-list data cell.
#[derive(Clone, Copy, Default)]
pub struct ListData {
    pub data16: i16,
    pub idx: i16,
}

/// Matrix parameters, mirroring `mat_params`.
#[derive(Default)]
pub struct MatParams {
    pub n: usize,
    pub a: Vec<i16>,
    pub b: Vec<i16>,
    pub c: Vec<i32>,
}

/// All inputs, working memory and outputs of one benchmark context, mirroring
/// the reference `core_results`.
#[derive(Default)]
pub struct CoreResults {
    // Inputs.
    pub seed1: i16,
    pub seed2: i16,
    pub seed3: i16,
    pub size: usize,
    pub iterations: u32,
    pub execs: u32,
    pub list: Option<crate::list::List>,
    pub state_mem: Vec<u8>,
    pub mat: MatParams,
    // Outputs.
    pub crc: u16,
    pub crclist: u16,
    pub crcmatrix: u16,
    pub crcstate: u16,
    pub total_ticks: u64,
}

/// Known CRC values for the list benchmark, indexed by the detected run.
pub const LIST_KNOWN_CRC: [u16; 5] = [0xd4b0, 0x3340, 0x6a79, 0xe714, 0xe3c1];
/// Known CRC values for the matrix benchmark, indexed by the detected run.
pub const MATRIX_KNOWN_CRC: [u16; 5] = [0xbe52, 0x1199, 0x5608, 0x1fd7, 0x0747];
/// Known CRC values for the state benchmark, indexed by the detected run.
pub const STATE_KNOWN_CRC: [u16; 5] = [0x5e47, 0x39bf, 0xe5a4, 0x8e3a, 0x8d84];

/// Compile-time replacement for the reference `check_data_types()`.
///
/// The C version reports a runtime error when a typedef is not the expected
/// width; Rust's fixed-width integer types guarantee this statically, so the
/// equivalents are asserted here and the function always contributes 0 errors.
const _: () = {
    assert!(size_of::<u8>() == 1, "u8 is not an 8b datatype");
    assert!(size_of::<u16>() == 2, "u16 is not a 16b datatype");
    assert!(size_of::<i16>() == 2, "i16 is not a 16b datatype");
    assert!(size_of::<i32>() == 4, "i32 is not a 32b datatype");
    assert!(size_of::<u32>() == 4, "u32 is not a 32b datatype");
    assert!(
        size_of::<usize>() == size_of::<*const u8>(),
        "usize is not a datatype that holds a pointer"
    );
};
