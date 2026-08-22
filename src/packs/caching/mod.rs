use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use super::{file_utils::file_content_digest, ProcessedFile};
pub(crate) mod cache;
pub(crate) mod noop_cache;
pub(crate) mod per_file_cache;

pub enum CacheResult {
    Processed(ProcessedFile),
    Miss(EmptyCacheEntry),
}

/// Cheap identity for a source file, obtained from one `stat` call.
///
/// The content digest remains the authority on whether a cache entry is valid.
/// This exists only so that the common case -- nothing changed since the last
/// run -- can be settled without opening and hashing the file.
///
/// # Only used where the filesystem timestamps finely enough to be trusted
///
/// Treating a matching (mtime, len) as "unchanged" is only sound if every write
/// moves the mtime, which is a filesystem property rather than a guarantee. On a
/// filesystem with one-second granularity -- some Docker bind mounts on macOS,
/// NFS, SMB, FAT -- a file edited to *the same length* within the same second as
/// it was cached keeps both its mtime and its length, and a stat-only check
/// would happily serve the stale entry.
///
/// Rather than assume, this detects it per file: a filesystem that reports a
/// non-zero sub-second component is one that tracks sub-second time, so an edit
/// at any other instant *would* have moved the mtime. When the component is zero
/// the stat is discarded and the caller falls back to hashing the contents,
/// which is always correct.
///
/// The consequences of being wrong run the safe direction in both cases:
///
/// - Coarse filesystem: every mtime is a whole second, every file falls back to
///   the digest, and the fast path simply does not engage. Correct, no faster.
/// - Fine filesystem, and a file whose mtime lands exactly on a second boundary:
///   a 1-in-10^9 coincidence that costs one extra hash for that file. Measured
///   on a 20,003-file Rails application: **zero** files hit it.
///
/// This narrows rather than closes the window. A filesystem with, say,
/// millisecond granularity reports a non-zero sub-second component and is
/// trusted, so two same-length writes inside one millisecond would still be
/// missed. That is six orders of magnitude tighter than the one-second case and
/// requires machine-speed edits to reach.
///
/// # The remaining hole: mtimes that are copied rather than set by writing
///
/// The check above establishes that the *filesystem* would have moved the mtime.
/// It cannot establish that nobody moved it back. Tools that deliberately
/// preserve timestamps -- `rsync -t`, `tar -p`, `cp -p`, unzip, some
/// backup/restore and container-image flows -- can install different content
/// carrying an mtime from somewhere else. If that mtime and the length both
/// happen to match what was cached, the fast path serves a stale entry.
///
/// In practice this needs the replacement to match the cached version in both
/// mtime and byte length, which usually means restoring a near-identical copy of
/// what was already there. It is not specific to this design: `make`, `ccache`
/// and every other mtime-driven cache have the same hole, which is why they all
/// document `touch` as a way to force a rebuild.
///
/// If it ever bites, `--no-cache` is the escape hatch, and `pks delete-cache`
/// clears the state. A tool-side fix would mean giving up on stat-only
/// validation and always hashing, which is precisely the cost this exists to
/// avoid.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub struct SourceStat {
    /// Nanoseconds since the unix epoch. u64 is good until the year 2554.
    pub mtime_ns: u64,
    pub len: u64,
}

const NANOS_PER_SEC: u64 = 1_000_000_000;

impl SourceStat {
    /// `None` whenever the stat cannot be trusted as a change detector: the file
    /// cannot be stat'd, has no mtime, has one before the unix epoch or too far
    /// in the future to represent, or -- see the type docs -- carries no
    /// sub-second precision. Every such case falls back to the content digest,
    /// which is authoritative anyway, and which will produce a sensible error if
    /// the file is genuinely unreadable.
    pub fn of(path: &Path) -> Option<SourceStat> {
        let metadata = std::fs::metadata(path).ok()?;
        let since_epoch = metadata
            .modified()
            .ok()?
            .duration_since(std::time::UNIX_EPOCH)
            .ok()?;

        // `try_from` rather than `as`, which would silently wrap a far-future
        // mtime into a small value that could collide with a real one.
        let mtime_ns = u64::try_from(since_epoch.as_nanos()).ok()?;

        // No sub-second component means this filesystem cannot tell us about a
        // change made within the same second. Do not trust it.
        if mtime_ns % NANOS_PER_SEC == 0 {
            return None;
        }

        Some(SourceStat {
            mtime_ns,
            len: metadata.len(),
        })
    }
}

#[derive(Debug, Default)]
pub struct EmptyCacheEntry {
    #[allow(dead_code)]
    pub filepath: PathBuf,
    /// `None` until [`Self::populate_digest`] computes it. Private so that
    /// "not computed yet" cannot be mistaken for a digest: writing an entry
    /// without one would persist a value that never matches, quietly making
    /// that file uncacheable forever.
    file_contents_digest: Option<String>,
    #[allow(dead_code)]
    pub file_name_digest: String,
    pub cache_file_path: PathBuf,
    pub source_stat: Option<SourceStat>,
}

impl EmptyCacheEntry {
    /// The parts of a cache entry that can be derived without reading the file's
    /// contents. Reading + MD5-ing the source is the expensive half, so it is
    /// deferred until something actually needs the digest.
    pub fn without_digest(
        cache_directory: &Path,
        filepath: &Path,
    ) -> anyhow::Result<EmptyCacheEntry> {
        let file_digest = md5::compute(filepath.to_str().unwrap());
        let file_name_digest = format!("{:x}", file_digest);
        let cache_file_path = cache_directory.join(&file_name_digest);

        Ok(EmptyCacheEntry {
            filepath: filepath.to_owned(),
            file_contents_digest: None,
            cache_file_path,
            file_name_digest,
            source_stat: SourceStat::of(filepath),
        })
    }

    /// Reads and hashes the file, at most once per entry.
    pub fn populate_digest(&mut self) -> anyhow::Result<&str> {
        if self.file_contents_digest.is_none() {
            self.file_contents_digest =
                Some(file_content_digest(&self.filepath)?);
        }
        Ok(self
            .file_contents_digest
            .as_deref()
            .expect("just populated above"))
    }

    /// The digest, if it has been computed. `None` means no one has called
    /// [`Self::populate_digest`] -- see the field comment for why that must not
    /// be treated as an empty digest.
    pub fn digest(&self) -> Option<&str> {
        self.file_contents_digest.as_deref()
    }
}

pub fn create_cache_dir_idempotently(cache_dir: &Path) {
    std::fs::create_dir_all(cache_dir)
        .expect("Failed to create cache directory");
}
