//! Command-line interface for the CoreMark benchmark.
//!
//! Arguments are positionally compatible with the reference `coremark` binary:
//!
//! ```text
//! coremark [seed1] [seed2] [seed3] [iterations] [execs] [unused] [size]
//! ```

use clap::Parser;

use coremark::util::parseval;
use coremark::{Config, run};

/// EEMBC CoreMark 1.0 benchmark.
#[derive(Parser, Debug)]
#[command(
    name = "coremark",
    version,
    about = "EEMBC CoreMark 1.0 benchmark",
    long_about = "Runs the EEMBC CoreMark 1.0 benchmark.\n\n\
        Positional arguments (all optional, in order):\n  \
        seed1 seed2 seed3  Seeds; leaving all three zero selects the 2K \
        performance run.\n  \
        iterations         Iteration count; 0 auto-calibrates for about 10 \
        seconds.\n  \
        execs              Bitmask of algorithms (1 = list, 2 = matrix, 4 = \
        state); 0 runs all.\n  \
        unused             Accepted for compatibility with the C version.\n  \
        size               Total data size in bytes; 0 uses the default of \
        2000.\n\n\
        Values accept the reference syntax: an optional leading '-', an \
        optional '0x' prefix for hexadecimal, and the 'K'/'M' multipliers.",
    allow_negative_numbers = true
)]
struct Args {
    /// Seeds, iteration count, algorithm mask and data size override.
    #[arg(value_parser = parse_seed)]
    values: Vec<i32>,
}

/// Parse an argument with the same rules as the reference `parseval`.
fn parse_seed(value: &str) -> Result<i32, String> {
    Ok(parseval(value))
}

impl Args {
    /// Map the positional arguments onto a benchmark configuration.
    fn config(&self) -> Config {
        let arg = |index: usize| self.values.get(index).copied().unwrap_or(0);
        Config {
            seed1: arg(0) as i16,
            seed2: arg(1) as i16,
            seed3: arg(2) as i16,
            iterations: arg(3).max(0) as u32,
            execs: arg(4).max(0) as u32,
            // Argument 6 (index 5) is unused, as in the reference.
            size: arg(6).max(0) as usize,
        }
    }
}

fn main() {
    let config = Args::parse().config();
    print!("{}", run(&config));
}
