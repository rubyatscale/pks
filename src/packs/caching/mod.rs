use std::path::{Path, PathBuf};

use anyhow::Context;
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
/// A related but distinct hole: NFS's close-to-open consistency model means a
/// client can serve a stat cached from before another client's very recent
/// write, independent of granularity. Same cause category -- the stat lied --
/// same escape hatches.
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
    pub fn from_path(path: &Path) -> Option<SourceStat> {
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

/// Appended to every content digest so a parsing-behavior upgrade invalidates
/// every entry, not just the files that actually changed. The stat fast path
/// checks for it too, since it never compares the digest itself.
pub const DIGEST_VERSION_SUFFIX: &str = concat!("-", env!("CARGO_PKG_VERSION"));

/// Everything about a source file that can be known without reading it: where
/// its cache entry lives, and the stat used to check that entry against the file.
///
/// Split out from [`EmptyCacheEntry`] because reading + MD5-ing the source is the
/// expensive half of a cache lookup, and on a warm cache the stat settles the
/// overwhelming majority of files without it. A lookup that ends on the fast path
/// never becomes an `EmptyCacheEntry` at all.
#[derive(Debug)]
pub struct CacheLookup {
    pub filepath: PathBuf,
    pub cache_file_path: PathBuf,
    pub source_stat: Option<SourceStat>,
}

impl CacheLookup {
    pub fn new(cache_directory: &Path, filepath: &Path) -> CacheLookup {
        let file_name_digest =
            format!("{:x}", md5::compute(filepath.to_str().unwrap()));

        CacheLookup {
            filepath: filepath.to_owned(),
            cache_file_path: cache_directory.join(file_name_digest),
            source_stat: SourceStat::from_path(filepath),
        }
    }

    /// Reads and hashes the file, producing the entry needed to write the cache.
    ///
    /// Consuming the lookup is what makes the digest structural for a real cache:
    /// [`per_file_cache::PerFileCache::write`] only ever receives an
    /// `EmptyCacheEntry` built here, so it can never be handed a placeholder
    /// digest -- which would produce an entry that never matches, silently
    /// making that file uncacheable forever. [`noop_cache::NoopCache`] is the one
    /// exception: it builds a placeholder via `Default` instead, which is sound
    /// only because its `write` ignores the argument entirely and never persists
    /// anything.
    pub fn read_contents(self) -> anyhow::Result<EmptyCacheEntry> {
        let file_contents_digest = format!(
            "{}{}",
            file_content_digest(&self.filepath)
                .context("Failed to create cache entry")?,
            DIGEST_VERSION_SUFFIX
        );

        Ok(EmptyCacheEntry {
            file_contents_digest,
            filepath: self.filepath,
            cache_file_path: self.cache_file_path,
            source_stat: self.source_stat,
        })
    }
}

/// A cache entry that has not been written yet. Carries a digest from
/// [`CacheLookup::read_contents`] in every case that matters -- the exception
/// is [`noop_cache::NoopCache`], which never reads a digest and never persists
/// one either; see `read_contents` for why that is safe.
#[derive(Debug, Default)]
pub struct EmptyCacheEntry {
    #[allow(dead_code)]
    pub filepath: PathBuf,
    pub file_contents_digest: String,
    pub cache_file_path: PathBuf,
    pub source_stat: Option<SourceStat>,
}

pub fn create_cache_dir_idempotently(cache_dir: &Path) {
    std::fs::create_dir_all(cache_dir)
        .expect("Failed to create cache directory");
}
