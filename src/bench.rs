//! Benchmark driver: memory setup, iteration loop, validation and reporting.
//!
//! Mirrors `core_main.c` together with the timing parts of `core_portme.c`.
//! Running the benchmark ([`run`]) is separated from rendering its report
//! ([`Results`]'s [`Display`] implementation) so both are reusable.

use std::fmt;
use std::time::Instant;

use crate::COMPILER_FLAGS;
use crate::COMPILER_VERSION;
use crate::list::core_list_init;
use crate::matrix::core_init_matrix;
use crate::state::core_init_state;
use crate::types::{
    ALL_ALGORITHMS_MASK, CoreResults, ID_LIST, ID_MATRIX, ID_STATE, LIST_KNOWN_CRC,
    MATRIX_KNOWN_CRC, NUM_ALGORITHMS, STATE_KNOWN_CRC, TOTAL_DATA_SIZE,
};
use crate::util::{crc16, crcu16};

/// Ticks per second produced by [`get_time`] (1 ms resolution, as in the
/// reference POSIX port).
const EE_TICKS_PER_SEC: u64 = 1_000;
/// Nanoseconds per second, matching the reference `NSECS_PER_SEC`.
const NSECS_PER_SEC: u64 = 1_000_000_000;
/// A run shorter than this many seconds is not a valid result.
const MIN_SECS_FOR_VALID: f64 = 10.0;
/// Report index of the 2K performance run; only that one prints the summary.
const KNOWN_ID_2K_PERF: i32 = 3;
/// Value of the reference `MEM_LOCATION` macro, which the POSIX port leaves
/// unspecified (`MEM_LOCATION_UNSPEC`).
const MEM_LOCATION: &str =
    "Please put data memory location here\n\t\t\t(e.g. code in flash, data on heap etc)";
/// Name of the memory method used for the allocation, from the C `mem_name`
/// table; the Rust port always uses the heap.
const MEM_METHOD_NAME: &str = "Heap";

/// Inputs of one benchmark run.
///
/// The defaults reproduce the reference behaviour: leaving every seed at zero
/// selects the 2K performance run, leaving `iterations` at zero makes the
/// driver pick a duration of roughly ten seconds, and leaving `execs` at zero
/// selects every algorithm.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Config {
    pub seed1: i16,
    pub seed2: i16,
    pub seed3: i16,
    /// Number of iterations; `0` selects the automatic (≈10 s) run.
    pub iterations: u32,
    /// Bitmask of algorithms to execute; `0` selects all of them.
    pub execs: u32,
    /// Total data size in bytes; `0` selects [`TOTAL_DATA_SIZE`].
    pub size: usize,
}

/// Outcome of one benchmark run.
///
/// Implements [`Display`] to render a report identical to the reference C
/// output.
#[derive(Clone, Copy, Debug)]
pub struct Results {
    pub seed1: i16,
    pub seed2: i16,
    pub seed3: i16,
    pub size: usize,
    pub iterations: u32,
    pub execs: u32,
    /// Elapsed time in milliseconds.
    pub total_ticks: u64,
    pub seedcrc: u16,
    pub crclist: u16,
    pub crcmatrix: u16,
    pub crcstate: u16,
    pub crcfinal: u16,
    /// Index of the recognised run, or `-1` when the seeds are unknown.
    pub known_id: i32,
    /// `0` when the run is valid, positive on validation errors and negative
    /// when the seeds cannot be validated.
    pub errors: i32,
}

impl Results {
    /// Elapsed time in seconds.
    #[inline]
    pub fn total_secs(&self) -> f64 {
        self.total_ticks as f64 / EE_TICKS_PER_SEC as f64
    }

    /// Iterations executed per second.
    #[inline]
    pub fn iterations_per_sec(&self) -> f64 {
        self.iterations as f64 / self.total_secs()
    }

    /// Title of the recognised run, if any.
    fn run_title(&self) -> Option<&'static str> {
        Some(match self.known_id {
            0 => "6k performance run parameters for coremark.",
            1 => "6k validation run parameters for coremark.",
            2 => "Profile generation run parameters for coremark.",
            3 => "2K performance run parameters for coremark.",
            4 => "2K validation run parameters for coremark.",
            _ => return None,
        })
    }
}

impl fmt::Display for Results {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let secs = self.total_secs();

        if let Some(title) = self.run_title() {
            writeln!(f, "{title}")?;

            let kid = self.known_id as usize;
            let expected = [
                (ID_LIST, "list", self.crclist, LIST_KNOWN_CRC[kid]),
                (ID_MATRIX, "matrix", self.crcmatrix, MATRIX_KNOWN_CRC[kid]),
                (ID_STATE, "state", self.crcstate, STATE_KNOWN_CRC[kid]),
            ];
            for (id, name, got, want) in expected {
                if self.execs & id != 0 && got != want {
                    writeln!(
                        f,
                        "[0]ERROR! {name} crc 0x{got:04x} - should be 0x{want:04x}"
                    )?;
                }
            }
        }

        writeln!(f, "CoreMark Size    : {}", self.size)?;
        writeln!(f, "Total ticks      : {}", self.total_ticks)?;
        writeln!(f, "Total time (secs): {secs:.6}")?;
        if secs > 0.0 {
            writeln!(f, "Iterations/Sec   : {:.6}", self.iterations_per_sec())?;
        }
        if secs < MIN_SECS_FOR_VALID {
            writeln!(
                f,
                "ERROR! Must execute for at least 10 secs for a valid result!"
            )?;
        }

        writeln!(f, "Iterations       : {}", self.iterations)?;
        writeln!(f, "Compiler version : {COMPILER_VERSION}")?;
        writeln!(f, "Compiler flags   : {COMPILER_FLAGS}")?;
        writeln!(f, "Memory location  : {MEM_LOCATION}")?;
        writeln!(f, "seedcrc          : 0x{:04x}", self.seedcrc)?;
        if self.execs & ID_LIST != 0 {
            writeln!(f, "[0]crclist       : 0x{:04x}", self.crclist)?;
        }
        if self.execs & ID_MATRIX != 0 {
            writeln!(f, "[0]crcmatrix     : 0x{:04x}", self.crcmatrix)?;
        }
        if self.execs & ID_STATE != 0 {
            writeln!(f, "[0]crcstate      : 0x{:04x}", self.crcstate)?;
        }
        writeln!(f, "[0]crcfinal      : 0x{:04x}", self.crcfinal)?;

        if self.errors == 0 {
            writeln!(
                f,
                "Correct operation validated. See README.md for run and reporting rules."
            )?;
            if self.known_id == KNOWN_ID_2K_PERF {
                writeln!(
                    f,
                    "CoreMark 1.0 : {:.6} / {COMPILER_VERSION} {COMPILER_FLAGS} / {MEM_METHOD_NAME}",
                    self.iterations_per_sec()
                )?;
            }
        } else if self.errors > 0 {
            writeln!(f, "Errors detected")?;
        } else {
            writeln!(
                f,
                "Cannot validate operation for these seed values, please compare with results on a known platform."
            )?;
        }
        Ok(())
    }
}

/// Run the benchmark described by `config` and return its results.
pub fn run(config: &Config) -> Results {
    let Config {
        mut seed1,
        mut seed2,
        mut seed3,
        iterations,
        mut execs,
        size,
    } = *config;

    if execs == 0 {
        execs = ALL_ALGORITHMS_MASK;
    }

    // Default values based on a single seed, for easy testing.
    match (seed1, seed2, seed3) {
        // Performance run.
        (0, 0, 0) => seed3 = 0x66,
        // Validation run.
        (1, 0, 0) => {
            seed1 = 0x3415;
            seed2 = 0x3415;
            seed3 = 0x66;
        }
        _ => {}
    }

    // Split the total data size between the algorithms that will run.
    let size = if size == 0 { TOTAL_DATA_SIZE } else { size };
    let num_algorithms = (0..NUM_ALGORITHMS)
        .filter(|i| 1u32 << i & execs != 0)
        .count();

    let mut res = CoreResults {
        seed1,
        seed2,
        seed3,
        size: size / num_algorithms,
        execs,
        ..Default::default()
    };

    // The list benchmark invokes the matrix and state algorithms through
    // `calc_func`, so all three must be set up regardless of the `execs` mask,
    // which only gates reporting and validation.
    res.list = Some(core_list_init(res.size, res.seed1));
    let combined = (res.seed1 as i32) | ((res.seed2 as i32) << 16);
    core_init_matrix(res.size, combined, &mut res.mat);
    res.state_mem = core_init_state(res.size, res.seed1);

    // Automatically determine the number of iterations when not specified.
    res.iterations = iterations;
    if res.iterations == 0 {
        res.iterations = 1;
        let mut secs_passed = 0.0;
        while secs_passed < 1.0 {
            res.iterations = res.iterations.saturating_mul(10);
            let start = Instant::now();
            iterate(&mut res);
            secs_passed = ticks_to_secs(get_time(start));
        }
        let divisor = (secs_passed as u32).max(1);
        res.iterations = res.iterations.wrapping_mul(1 + 10 / divisor);
    }

    // Perform the actual benchmark.
    let start = Instant::now();
    iterate(&mut res);
    res.total_ticks = get_time(start);

    // Derive a function of the input to report.
    let mut seedcrc: u16 = 0;
    seedcrc = crc16(res.seed1, seedcrc);
    seedcrc = crc16(res.seed2, seedcrc);
    seedcrc = crc16(res.seed3, seedcrc);
    seedcrc = crc16(res.size as i16, seedcrc);

    let known_id: i32 = match seedcrc {
        0x8a02 => 0,
        0x7b05 => 1,
        0x4eaf => 2,
        0xe9f5 => 3,
        0x18f2 => 4,
        _ => -1,
    };

    // Compare the CRCs against the values known for the recognised run.
    let mut errors = if known_id < 0 { -1 } else { 0 };
    if known_id >= 0 {
        let kid = known_id as usize;
        let expected = [
            (ID_LIST, res.crclist, LIST_KNOWN_CRC[kid]),
            (ID_MATRIX, res.crcmatrix, MATRIX_KNOWN_CRC[kid]),
            (ID_STATE, res.crcstate, STATE_KNOWN_CRC[kid]),
        ];
        errors += expected
            .iter()
            .filter(|(id, got, want)| execs & id != 0 && got != want)
            .count() as i32;
    }
    if ticks_to_secs(res.total_ticks) < MIN_SECS_FOR_VALID {
        errors += 1;
    }

    Results {
        seed1: res.seed1,
        seed2: res.seed2,
        seed3: res.seed3,
        size: res.size,
        iterations: res.iterations,
        execs,
        total_ticks: res.total_ticks,
        seedcrc,
        crclist: res.crclist,
        crcmatrix: res.crcmatrix,
        crcstate: res.crcstate,
        crcfinal: res.crc,
        known_id,
        errors,
    }
}

/// Run `res.iterations` iterations of the list benchmark, mirroring `iterate`.
fn iterate(res: &mut CoreResults) {
    res.crc = 0;
    res.crclist = 0;
    res.crcmatrix = 0;
    res.crcstate = 0;

    for i in 0..res.iterations {
        let mut list = res.list.take().expect("list initialized");
        let crc = list.bench(res, 1);
        res.crc = crcu16(crc, res.crc);
        let crc = list.bench(res, -1);
        res.crc = crcu16(crc, res.crc);
        res.list = Some(list);
        if i == 0 {
            res.crclist = res.crc;
        }
    }
}

/// Return the elapsed milliseconds since `start`, matching `CORE_TICKS`.
#[inline]
fn get_time(start: Instant) -> u64 {
    start.elapsed().as_nanos() as u64 * EE_TICKS_PER_SEC / NSECS_PER_SEC
}

/// Convert a tick count (milliseconds) to seconds.
#[inline]
fn ticks_to_secs(ticks: u64) -> f64 {
    ticks as f64 / EE_TICKS_PER_SEC as f64
}
