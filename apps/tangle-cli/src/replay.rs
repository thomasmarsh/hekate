//! Reproduce a completed run from its immutable run directory.
//!
//! A run directory is self-sufficient to reproduce its run: its `manifest.json`
//! names the authored scenario source and the SHA-256 of its bytes, the root
//! seed, the fixed step, and the tick count. `replay` re-loads that source,
//! checks the recorded content hash against the bytes on disk, and re-runs the
//! kernel with the recorded parameters, emitting the reproduced canonical event
//! stream — the same bytes `run` writes to `--output` and into the run
//! directory's `events.jsonl.gz`.
//!
//! Reproduction is checked, not assumed. With verification, replay compares the
//! reproduced stream with the recorded `events.jsonl.gz`, decompressed to
//! canonical record order, byte for byte, and the reproduced trace hash with
//! the manifest's recorded stream hash, so a tampered artifact, a scenario
//! source edited after the run, or an altered seed, step, or tick count fails
//! with a diagnostic naming the mismatch.
//!
//! Replay is read-only. It opens the run directory and the scenario source and
//! writes neither, so it cannot mutate a completed run's artifacts.

use std::fs;
use std::io::{self, Read};
use std::path::{Path, PathBuf};

use flate2::read::GzDecoder;
use tangle_sim::{InitError, RunConfig, Seconds};

use crate::load_scenario_hashed;
use crate::run_dir::{EVENT_STREAM_COMPRESSION, EVENT_STREAM_FILE, MANIFEST_FILE, RunManifest};
use crate::trace::{Trace, canonical_run};

/// Failure to reproduce a recorded run.
#[derive(Debug, thiserror::Error)]
pub enum ReplayError {
    /// The run directory does not exist.
    #[error("run directory '{path}' does not exist")]
    MissingRunDirectory {
        /// The directory replay was pointed at.
        path: PathBuf,
    },
    /// The directory holds no completion marker, so it is not a completed run.
    #[error(
        "run directory '{path}' is incomplete: it holds no manifest.json, the completion marker of a run; replay needs a completed run directory"
    )]
    IncompleteRunDirectory {
        /// The incomplete directory it was pointed at.
        path: PathBuf,
    },
    /// The manifest could not be read.
    #[error("cannot read run manifest '{path}': {source}")]
    ManifestRead {
        /// The manifest file that failed.
        path: PathBuf,
        /// The underlying I/O failure.
        #[source]
        source: io::Error,
    },
    /// The manifest is not valid run-manifest JSON.
    #[error("run manifest '{path}' is not valid run-manifest JSON: {source}")]
    ManifestParse {
        /// The manifest file that failed.
        path: PathBuf,
        /// The underlying JSON failure.
        #[source]
        source: serde_json::Error,
    },
    /// The recorded scenario source could not be loaded.
    #[error("cannot load the recorded scenario source '{path}': {source}")]
    Scenario {
        /// The path the manifest recorded.
        path: PathBuf,
        /// The loader's failure.
        #[source]
        source: crate::LoadError,
    },
    /// The scenario source no longer hashes to the manifest's recorded hash.
    #[error(
        "scenario source '{path}' has content hash {actual}, but the run manifest recorded {recorded}; the source changed since the run"
    )]
    ScenarioChanged {
        /// The path the manifest recorded.
        path: PathBuf,
        /// The content hash the manifest recorded.
        recorded: String,
        /// The content hash of the bytes on disk now.
        actual: String,
    },
    /// The kernel rejected the recorded run parameters.
    #[error("cannot reproduce the recorded run: {source}")]
    Init {
        /// The kernel's initialization failure.
        #[source]
        source: InitError,
    },
    /// The manifest names a stream this run-directory layout does not hold.
    #[error(
        "run directory '{path}' describes event stream '{described}', but replay verifies 'events.jsonl.gz' (gzip)"
    )]
    UnexpectedStream {
        /// The run directory whose manifest disagrees.
        path: PathBuf,
        /// The stream the manifest describes, as `path (compression)`.
        described: String,
    },
    /// The recorded event stream could not be read.
    #[error("cannot read the recorded event stream '{path}': {source}")]
    StreamRead {
        /// The stream file that failed.
        path: PathBuf,
        /// The underlying I/O failure.
        #[source]
        source: io::Error,
    },
    /// The recorded event stream is not the compression its manifest declares.
    #[error("cannot decode the recorded event stream '{path}' as gzip: {source}")]
    StreamDecode {
        /// The stream file that failed.
        path: PathBuf,
        /// The underlying decode failure.
        #[source]
        source: io::Error,
    },
    /// The reproduction disagrees with the recorded run.
    #[error("replay of '{path}' does not reproduce the recorded run: {detail}")]
    Mismatch {
        /// The artifact or manifest that disagrees.
        path: PathBuf,
        /// What disagrees, with the offset or hashes that differ.
        detail: String,
    },
}

/// Reproduce a completed run directory's canonical event stream.
///
/// The manifest is read from `directory/manifest.json`; the recorded scenario
/// source is re-loaded from the path the manifest records, resolved against the
/// current working directory; its content hash is checked against the
/// manifest's; and the kernel is re-run with the recorded seed, fixed step, and
/// tick count. The reproduced canonical trace is returned. With `verify`,
/// [`verify_reproduction`] additionally compares it with the recorded
/// `events.jsonl.gz` and the manifest's recorded stream hash.
///
/// Nothing is written either way, so replay never mutates the run directory.
pub fn replay_run_directory(directory: &Path, verify: bool) -> Result<Trace, ReplayError> {
    let manifest = read_manifest(directory)?;
    let trace = reproduce(&manifest)?;
    if verify {
        verify_reproduction(directory, &manifest, &trace)?;
    }
    Ok(trace)
}

/// Read the manifest of a completed run directory.
///
/// The completion marker is `manifest.json`, the same marker a batch reads, so
/// a directory without it is an interrupted run rather than something replay
/// could reproduce.
fn read_manifest(directory: &Path) -> Result<RunManifest, ReplayError> {
    if !directory.is_dir() {
        return Err(ReplayError::MissingRunDirectory {
            path: directory.to_path_buf(),
        });
    }
    let path = directory.join(MANIFEST_FILE);
    let json = match fs::read_to_string(&path) {
        Ok(json) => json,
        Err(source) if source.kind() == io::ErrorKind::NotFound => {
            return Err(ReplayError::IncompleteRunDirectory {
                path: directory.to_path_buf(),
            });
        }
        Err(source) => return Err(ReplayError::ManifestRead { path, source }),
    };
    serde_json::from_str(&json).map_err(|source| ReplayError::ManifestParse { path, source })
}

/// Re-run the recorded scenario with the recorded parameters.
///
/// The content hash is checked before the run, so a source edited since the run
/// fails as a provenance mismatch rather than as a confusing stream difference.
fn reproduce(manifest: &RunManifest) -> Result<Trace, ReplayError> {
    let source = PathBuf::from(&manifest.scenario.source_path);
    let (scenario, content_sha256) =
        load_scenario_hashed(&source).map_err(|error| ReplayError::Scenario {
            path: source.clone(),
            source: error,
        })?;
    if content_sha256 != manifest.scenario.content_sha256 {
        return Err(ReplayError::ScenarioChanged {
            path: source,
            recorded: manifest.scenario.content_sha256.clone(),
            actual: content_sha256,
        });
    }
    let config = RunConfig::new(manifest.seed).with_step(Seconds::from_secs(manifest.step_s));
    canonical_run(scenario, config, manifest.ticks)
        .map(|(trace, _)| trace)
        .map_err(|source| ReplayError::Init { source })
}

/// Compare a reproduction with the recorded artifacts it must match.
///
/// The stream is compared first because a byte difference names where the
/// streams diverge; the recorded hash is compared as well so a manifest whose
/// recorded hash disagrees with its own artifact is caught even when the bytes
/// happen to match.
fn verify_reproduction(
    directory: &Path,
    manifest: &RunManifest,
    trace: &Trace,
) -> Result<(), ReplayError> {
    let recorded = read_recorded_stream(directory, manifest)?;
    if let Some(difference) = describe_difference(&recorded, trace.bytes()) {
        return Err(ReplayError::Mismatch {
            path: directory.join(EVENT_STREAM_FILE),
            detail: format!(
                "the reproduced canonical event stream does not equal the recorded stream, {difference}"
            ),
        });
    }
    if trace.hash() != manifest.stream.uncompressed_sha256 {
        return Err(ReplayError::Mismatch {
            path: directory.join(MANIFEST_FILE),
            detail: format!(
                "the reproduced trace hash {} does not equal the manifest's recorded stream hash {}",
                trace.hash(),
                manifest.stream.uncompressed_sha256
            ),
        });
    }
    Ok(())
}

/// The recorded event stream, decompressed to canonical record order.
///
/// The manifest names its own stream file; replay verifies it is the file this
/// run-directory layout holds before reading it, so a manifest from another
/// layout fails as an unsupported descriptor rather than as a decode error.
fn read_recorded_stream(directory: &Path, manifest: &RunManifest) -> Result<Vec<u8>, ReplayError> {
    if manifest.stream.path != EVENT_STREAM_FILE
        || manifest.stream.compression != EVENT_STREAM_COMPRESSION
    {
        return Err(ReplayError::UnexpectedStream {
            path: directory.to_path_buf(),
            described: format!("{} ({})", manifest.stream.path, manifest.stream.compression),
        });
    }
    let path = directory.join(EVENT_STREAM_FILE);
    let compressed = fs::read(&path).map_err(|source| ReplayError::StreamRead {
        path: path.clone(),
        source,
    })?;
    let mut decoded = Vec::new();
    GzDecoder::new(&compressed[..])
        .read_to_end(&mut decoded)
        .map_err(|source| ReplayError::StreamDecode { path, source })?;
    Ok(decoded)
}

/// Where two streams first disagree, or `None` when they are equal.
fn describe_difference(recorded: &[u8], reproduced: &[u8]) -> Option<String> {
    if recorded == reproduced {
        return None;
    }
    let shared = recorded.len().min(reproduced.len());
    let offset = recorded[..shared]
        .iter()
        .zip(&reproduced[..shared])
        .position(|(recorded, reproduced)| recorded != reproduced)
        .unwrap_or(shared);
    Some(format!(
        "first differing byte at offset {offset} (recorded {} bytes, reproduced {} bytes)",
        recorded.len(),
        reproduced.len()
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A directory that does not exist is a missing run, not an incomplete one.
    #[test]
    fn an_absent_run_directory_is_reported_as_missing() {
        let directory =
            std::env::temp_dir().join(format!("tangle-replay-absent-{}", std::process::id()));
        let _ = fs::remove_dir_all(&directory);

        let error = replay_run_directory(&directory, false).expect_err("no directory");

        assert!(
            matches!(&error, ReplayError::MissingRunDirectory { path } if path == &directory),
            "unexpected error: {error}"
        );
        assert!(error.to_string().contains("does not exist"), "{error}");
    }

    /// A directory without the completion marker is an interrupted run.
    #[test]
    fn a_directory_without_a_manifest_is_reported_as_incomplete() {
        let directory =
            std::env::temp_dir().join(format!("tangle-replay-incomplete-{}", std::process::id()));
        let _ = fs::remove_dir_all(&directory);
        fs::create_dir_all(&directory).expect("scratch directory is created");

        let error = replay_run_directory(&directory, false).expect_err("no manifest");

        let _ = fs::remove_dir_all(&directory);
        assert!(
            matches!(&error, ReplayError::IncompleteRunDirectory { path } if path == &directory),
            "unexpected error: {error}"
        );
        assert!(error.to_string().contains("incomplete"), "{error}");
    }

    /// A stream difference names where it begins, which is what makes a
    /// verification failure actionable.
    #[test]
    fn a_stream_difference_names_its_first_offset() {
        assert_eq!(describe_difference(b"abc", b"abc"), None);

        let changed = describe_difference(b"abc", b"abd").expect("streams differ");
        assert!(changed.contains("offset 2"), "{changed}");

        // A prefix is where they stop agreeing, never past the shorter stream.
        let truncated = describe_difference(b"abcd", b"abc").expect("streams differ");
        assert!(truncated.contains("offset 3"), "{truncated}");
        assert!(
            truncated.contains("recorded 4 bytes, reproduced 3 bytes"),
            "{truncated}"
        );

        let appended = describe_difference(b"abc", b"abcd").expect("streams differ");
        assert!(appended.contains("offset 3"), "{appended}");
        assert!(
            appended.contains("recorded 3 bytes, reproduced 4 bytes"),
            "{appended}"
        );
    }
}
