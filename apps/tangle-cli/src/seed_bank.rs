//! A versioned common-random-number (CRN) seed bank.
//!
//! A seed bank is the seed list two variants of one scenario share so their
//! per-seed runs are paired: position *i* of variant A and position *i* of
//! variant B run at the same seed, so a paired A/B comparison cancels the
//! between-seed variance both variants carry. The bank is one JSON artifact:
//!
//! ```json
//! {
//!   "seed_bank_version": 1,
//!   "seeds": [0, 1, 2]
//! }
//! ```
//!
//! ## Ordering
//!
//! The bank's seeds are **strictly ascending** and unique, and that file order
//! is the order a consumer uses — it never re-sorts or deduplicates. Ascending
//! is the documented order because it is canonical: equal seed sets produce
//! byte-identical banks, so the bank's content hash is a usable identity for
//! "both sides of the comparison ran one bank". Two banks that hash the same
//! hold the same seeds in the same order.
//!
//! ## Derivation and the CRN limit
//!
//! The kernel derives every named random stream from one root seed:
//! `tangle_sim` mixes the root seed, the stream name (`demand`,
//! `pedestrian_demand`, `profile`, `compliance`), and a stable identifier
//! (demand-source index or agent id) into each stream's seed, so a bank needs no
//! per-stream sub-seeds — the single run seed already determines every stream.
//! Common random numbers then follow from running two variants at the same seed:
//! both draw the same value at the same draw position. The limit is that
//! alignment is positional: a variant that changes the number or order of draws
//! on a stream desynchronizes that stream's correspondence even at the same
//! seed, so a bank equalizes the seeds, not the number of draws.
//!
//! ## Byte reproducibility
//!
//! Nothing in this module reads a clock or a random source: a seed bank is a
//! pure function of the requested seeds, and the writer emits the crate's
//! canonical pretty JSON with a trailing newline.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::trace::sha256_hex;

/// Version of the seed bank artifact format.
pub const SEED_BANK_VERSION: u32 = 1;

/// Failure to build, validate, or read a seed bank.
#[derive(Debug, thiserror::Error)]
pub enum SeedBankError {
    /// The seed bank file could not be read.
    #[error("cannot {action} seed bank '{path}': {source}")]
    Io {
        /// The path the operation targeted.
        path: PathBuf,
        /// The operation that failed.
        action: &'static str,
        /// The underlying I/O failure.
        #[source]
        source: io::Error,
    },
    /// The file was not a seed bank document.
    #[error("seed bank '{path}' is not valid seed-bank JSON: {source}")]
    Parse {
        /// The file that was parsed.
        path: PathBuf,
        /// The underlying JSON failure.
        #[source]
        source: serde_json::Error,
    },
    /// The file declares a format version this build does not read.
    #[error(
        "seed bank '{path}' declares seed_bank_version {version}; this build reads version {supported}"
    )]
    UnsupportedVersion {
        /// The file that declares the version.
        path: PathBuf,
        /// The version the file declares.
        version: u32,
        /// The version this build reads.
        supported: u32,
    },
    /// The seed list is empty, so it pairs no run with anything.
    #[error("{origin} holds no seeds; a seed bank needs at least one")]
    Empty {
        /// What was validated, e.g. `seed bank 'bank.json'`.
        origin: String,
    },
    /// The seed list repeats a seed, so a position would pair a run with itself.
    #[error("{origin} holds seed {seed} twice; every seed must be unique")]
    Duplicate {
        /// What was validated, e.g. `seed bank 'bank.json'`.
        origin: String,
        /// The repeated seed.
        seed: u64,
    },
    /// The seed list is not in ascending order.
    #[error("{origin} is not in ascending order: seed {seed} follows {previous}")]
    NotAscending {
        /// What was validated, e.g. `seed bank 'bank.json'`.
        origin: String,
        /// The seed that precedes.
        previous: u64,
        /// The seed that is out of order.
        seed: u64,
    },
    /// An arithmetic bank was asked for no seeds.
    #[error("a seed bank needs a positive count, got 0")]
    ZeroCount,
    /// An arithmetic bank was asked for a zero step, which would repeat one seed.
    #[error("a seed bank needs a positive stride, got 0")]
    ZeroStride,
    /// An arithmetic bank's last seed does not fit a 64-bit seed.
    #[error(
        "a seed bank from start {start} of {count} seeds at stride {stride} overflows a 64-bit seed"
    )]
    Overflow {
        /// The first seed.
        start: u64,
        /// The number of seeds.
        count: u64,
        /// The step between seeds.
        stride: u64,
    },
}

/// The ordered seeds of one common-random-number bank.
///
/// `seeds` is strictly ascending and unique; [`SeedBank::from_seeds`] and
/// [`read_seed_bank`] are the ways to obtain one, so the invariant holds for
/// every value that reaches a consumer.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SeedBank {
    /// Artifact format version; always [`SEED_BANK_VERSION`] for a bank this
    /// build writes.
    pub seed_bank_version: u32,
    /// The bank's seeds, strictly ascending.
    pub seeds: Vec<u64>,
}

impl SeedBank {
    /// Build a bank from an explicit seed list.
    ///
    /// The list is written in ascending order regardless of the order it is
    /// given, and a repeated seed is refused rather than collapsed, because a
    /// repeated seed would give two positions the same run.
    pub fn from_seeds(seeds: &[u64]) -> Result<Self, SeedBankError> {
        let mut seeds = seeds.to_vec();
        seeds.sort_unstable();
        validate_ascending(&seeds, "the requested seed list")?;
        Ok(Self {
            seed_bank_version: SEED_BANK_VERSION,
            seeds,
        })
    }

    /// Build an arithmetic bank: `start`, `start + stride`, ... `count` seeds.
    ///
    /// The sequence is ascending by construction and refuses a zero count, a
    /// zero stride, and a last seed that would overflow.
    pub fn from_range(start: u64, count: u64, stride: u64) -> Result<Self, SeedBankError> {
        if count == 0 {
            return Err(SeedBankError::ZeroCount);
        }
        if stride == 0 {
            return Err(SeedBankError::ZeroStride);
        }
        let mut seeds = Vec::new();
        for index in 0..count {
            let seed = stride
                .checked_mul(index)
                .and_then(|offset| start.checked_add(offset))
                .ok_or(SeedBankError::Overflow {
                    start,
                    count,
                    stride,
                })?;
            seeds.push(seed);
        }
        Ok(Self {
            seed_bank_version: SEED_BANK_VERSION,
            seeds,
        })
    }
}

/// A seed bank read from disk with the hash of the bytes it was read from.
///
/// The hash covers the file's exact bytes, including whitespace, so a batch
/// manifest can record which bank artifact a run consumed rather than which
/// seeds it happened to hold.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoadedSeedBank {
    /// The validated bank.
    pub bank: SeedBank,
    /// Lowercase hexadecimal SHA-256 of the bank file's exact bytes.
    pub content_sha256: String,
}

/// The identity of the seed bank one batch ran from, as a batch manifest
/// records it.
///
/// The path is the one the command was given, not a canonicalized path, so a
/// consumer re-reads and re-hashes the artifact the batch actually named; the
/// content hash is what proves two batches consumed one bank. Two spellings of
/// the same file compare equal by hash, not by path.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SeedBankReference {
    /// The bank path exactly as the command was given it.
    pub path: String,
    /// Lowercase hexadecimal SHA-256 of the bank file's exact bytes.
    pub content_sha256: String,
}

/// Read, parse, and validate a seed bank file.
///
/// A file that is not seed-bank JSON, declares another format version, holds no
/// seeds, repeats a seed, or is not ascending is refused here, so a batch never
/// runs from an ambiguous bank.
pub fn read_seed_bank(path: &Path) -> Result<LoadedSeedBank, SeedBankError> {
    let bytes = fs::read(path).map_err(|source| SeedBankError::Io {
        path: path.to_path_buf(),
        action: "read",
        source,
    })?;
    let bank: SeedBank = serde_json::from_slice(&bytes).map_err(|source| SeedBankError::Parse {
        path: path.to_path_buf(),
        source,
    })?;
    if bank.seed_bank_version != SEED_BANK_VERSION {
        return Err(SeedBankError::UnsupportedVersion {
            path: path.to_path_buf(),
            version: bank.seed_bank_version,
            supported: SEED_BANK_VERSION,
        });
    }
    let origin = format!("seed bank '{}'", path.display());
    validate_ascending(&bank.seeds, &origin)?;
    Ok(LoadedSeedBank {
        bank,
        content_sha256: sha256_hex(&bytes),
    })
}

/// Check that a seed list is a usable pairing list: non-empty, unique, and
/// ascending.
///
/// `origin` names what is being checked in the error, so a caller that reads a
/// file and a caller that builds an explicit list report the same failure
/// shape.
fn validate_ascending(seeds: &[u64], origin: &str) -> Result<(), SeedBankError> {
    let Some((&first, rest)) = seeds.split_first() else {
        return Err(SeedBankError::Empty {
            origin: origin.to_owned(),
        });
    };
    let mut previous = first;
    for &seed in rest {
        if seed == previous {
            return Err(SeedBankError::Duplicate {
                origin: origin.to_owned(),
                seed,
            });
        }
        if seed < previous {
            return Err(SeedBankError::NotAscending {
                origin: origin.to_owned(),
                previous,
                seed,
            });
        }
        previous = seed;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_explicit_list_is_written_ascending() {
        let bank = SeedBank::from_seeds(&[2, 0, 1]).expect("seeds build");
        assert_eq!(bank.seed_bank_version, SEED_BANK_VERSION);
        assert_eq!(bank.seeds, vec![0, 1, 2]);
    }

    #[test]
    fn a_repeated_or_empty_explicit_list_is_refused() {
        assert!(matches!(
            SeedBank::from_seeds(&[1, 1]),
            Err(SeedBankError::Duplicate { seed: 1, .. })
        ));
        assert!(matches!(
            SeedBank::from_seeds(&[]),
            Err(SeedBankError::Empty { .. })
        ));
    }

    #[test]
    fn an_arithmetic_bank_steps_from_its_start() {
        let bank = SeedBank::from_range(10, 4, 5).expect("range builds");
        assert_eq!(bank.seeds, vec![10, 15, 20, 25]);
    }

    #[test]
    fn an_arithmetic_bank_refuses_zero_and_overflow() {
        assert!(matches!(
            SeedBank::from_range(0, 0, 1),
            Err(SeedBankError::ZeroCount)
        ));
        assert!(matches!(
            SeedBank::from_range(0, 2, 0),
            Err(SeedBankError::ZeroStride)
        ));
        assert!(matches!(
            SeedBank::from_range(u64::MAX - 1, 3, 1),
            Err(SeedBankError::Overflow { .. })
        ));
    }

    /// The validation reports the order the file fixes, so an unsorted bank is
    /// never silently reordered into a different pairing.
    #[test]
    fn descending_seeds_are_refused_not_reordered() {
        assert!(matches!(
            validate_ascending(&[2, 1], "a test list"),
            Err(SeedBankError::NotAscending {
                previous: 2,
                seed: 1,
                ..
            })
        ));
    }
}
