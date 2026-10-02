use crate::packs::ProcessedFile;
use serde::{Deserialize, Serialize};

use anyhow::Context;
use std::fs::File;
use std::io::Write;
use std::path::Path;
use std::path::PathBuf;
use tracing::warn;

use super::cache::Cache;
use super::CacheLookup;
use super::CacheResult;
use super::EmptyCacheEntry;
use super::SourceStat;
use super::DIGEST_VERSION_SUFFIX;

pub struct PerFileCache {
    pub cache_dir: PathBuf,
}

impl Cache for PerFileCache {
    fn get(&self, path: &Path) -> anyhow::Result<CacheResult> {
        // Deliberately does not read the source file yet. On a warm cache the
        // stat below settles the overwhelming majority of files, and reading
        // every source file to MD5 it was roughly half the cost of this phase.
        let lookup = CacheLookup::new(&self.cache_dir, path);

        let Some(cache_entry) = CacheEntry::from_lookup(&lookup)? else {
            return Ok(CacheResult::Miss(lookup.read_contents()?));
        };

        // Fast path: the file has the same mtime and length as when we cached
        // it, so it cannot have changed in any way we care about.
        //
        // The version check stands in for the digest comparison this path
        // skips. Without it, an entry written by another version of pks would
        // be served whenever its file is untouched, and an upgrade that changes
        // parsing would keep the old results.
        //
        // `is_some()` is not redundant with the equality check and must not be
        // folded into it. Both sides are `None` whenever no usable stat exists --
        // on a filesystem too coarse to be trusted, every file every run (see
        // `SourceStat`) -- and `None == None` is true. Without this, "we have no
        // idea whether the file changed" would read as "the file is unchanged",
        // serving stale entries on exactly the filesystems the stat check exists
        // to protect. Covered by `test_whole_second_mtime_is_not_trusted`.
        if cache_entry.source_stat.is_some()
            && cache_entry.source_stat == lookup.source_stat
            && cache_entry
                .file_contents_digest
                .ends_with(DIGEST_VERSION_SUFFIX)
        {
            return Ok(CacheResult::Processed(cache_entry.processed_file));
        }

        // Slow path: no stat recorded (entry predates this feature, or was
        // written by packwerk), or the stat moved. The content digest is still
        // the authority, so fall back to it.
        let empty_cache_entry = lookup.read_contents()?;
        if cache_entry.file_contents_digest
            != empty_cache_entry.file_contents_digest
        {
            return Ok(CacheResult::Miss(empty_cache_entry));
        }

        // Contents are unchanged but the stat differs -- a checkout, a `touch`,
        // or an entry written before stats were recorded. Refresh the entry so
        // the next run takes the fast path.
        //
        // Only when there is actually a usable stat to record, and it differs
        // from what is on disk. Without this guard, a filesystem too coarse to
        // produce a trustworthy stat (see `SourceStat`) would yield `None` on
        // every run, never match, and rewrite every cache entry every time --
        // turning a read-mostly cache into a full rewrite of itself.
        let stat_is_worth_recording = empty_cache_entry.source_stat.is_some()
            && empty_cache_entry.source_stat != cache_entry.source_stat;

        if stat_is_worth_recording {
            // A failure here is not fatal: the result we return is still
            // correct, we just re-hash this file on the next run too. It is
            // warned about rather than ignored, because a persistent failure
            // (an unwritable cache dir, a full disk) degrades every subsequent
            // run and would otherwise be invisible -- the tool would simply be
            // slow forever with no clue why.
            if let Err(e) =
                self.write(&empty_cache_entry, &cache_entry.processed_file)
            {
                warn!(
                    "Failed to refresh cache entry {:?}; it will be re-hashed \
                     on every run until this succeeds: {}",
                    empty_cache_entry.cache_file_path, e
                );
            }
        }

        Ok(CacheResult::Processed(cache_entry.processed_file))
    }

    fn write(
        &self,
        empty_cache_entry: &EmptyCacheEntry,
        processed_file: &ProcessedFile,
    ) -> anyhow::Result<()> {
        let cache_entry = &CacheEntry {
            file_contents_digest: empty_cache_entry
                .file_contents_digest
                .to_owned(),
            source_stat: empty_cache_entry.source_stat,
            // Ideally we could pass by reference here, but in practice this cost should be paid on few files
            // that have changed and need to be reprocessed.
            processed_file: processed_file.clone(),
        };

        let cache_data = serde_json::to_string(&cache_entry)
            .context("Failed to serialize references")?;

        // Ensure parent directory exists
        if let Some(parent) = empty_cache_entry.cache_file_path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| {
                anyhow::Error::new(e).context(format!(
                    "Failed to create cache directory {:?}",
                    parent
                ))
            })?;
        }

        let mut file = File::create(&empty_cache_entry.cache_file_path)
            .map_err(|e| {
                anyhow::Error::new(e).context(format!(
                    "Failed to create cache file {:?}",
                    empty_cache_entry.cache_file_path
                ))
            })?;

        file.write_all(cache_data.as_bytes())
            .context("Failed to write cache file")?;
        Ok(())
    }
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct CacheEntry {
    pub file_contents_digest: String,
    /// Absent in entries written by packwerk, or by versions of pks before the
    /// stat fast path existed. `serde(default)` keeps those entries readable;
    /// they simply fall back to comparing the content digest.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_stat: Option<SourceStat>,
    pub processed_file: ProcessedFile,
}

impl CacheEntry {
    pub fn from_lookup(
        lookup: &CacheLookup,
    ) -> anyhow::Result<Option<CacheEntry>> {
        let cache_file_path = &lookup.cache_file_path;

        if cache_file_path.exists() {
            match read_json_file(cache_file_path) {
                Ok(cache_entry) => Ok(Some(cache_entry)),
                Err(e) => {
                    warn!(
                        "Failed to read cache file {:?}: {}",
                        cache_file_path, e
                    );
                    Ok(None)
                }
            }
        } else {
            Ok(None)
        }
    }
}

pub fn read_json_file(path: &PathBuf) -> anyhow::Result<CacheEntry> {
    let file = std::fs::File::open(path)
        .context(format!("Failed to open file {:?}", path))?;
    let reader = std::io::BufReader::new(file);
    let data = serde_json::from_reader(reader)
        .context("Failed to deserialize CacheEntry")?;
    Ok(data)
}

#[cfg(test)]
mod tests {
    use std::fs;

    use crate::packs::{
        self, configuration,
        file_utils::file_content_digest,
        parsing::{Range, UnresolvedReference},
    };

    use super::*;

    fn teardown() {
        packs::delete_cache(
            configuration::get(&PathBuf::from("tests/fixtures/simple_app"))
                .unwrap(),
        );
    }

    #[test]
    fn test_file_content_digest() {
        let file_path =
            "tests/fixtures/simple_app/packs/bar/app/services/bar.rb";
        let expected_digest = "305bc58696c2e664057b6751064cf2e3";

        let digest = file_content_digest(&PathBuf::from(file_path));

        assert!(digest.is_ok());
        assert_eq!(digest.unwrap(), expected_digest);

        teardown();
    }

    #[test]
    fn test_compatible_with_packwerk() {
        let contents: String = String::from(
            r#"{
  "file_contents_digest":"8f9efdcf2caa22fb7b1b4a8274e68d11",
  "processed_file": {
    "absolute_path":"/tests/fixtures/simple_app/packs/foo/app/services/bar/foo.rb",
    "unresolved_references":[
      {
        "name":"Bar",
        "namespace_path":["Foo","Bar"],
        "location":{"start_row":8,"start_col":22,"end_row":8,"end_col":25}
      }],
    "definitions":[]
  }
}"#,
        );

        let expected_serialized = CacheEntry {
            file_contents_digest: "8f9efdcf2caa22fb7b1b4a8274e68d11".to_owned(),
            // A packwerk-written entry carries no stat; it must still deserialize.
            source_stat: None,
            processed_file: ProcessedFile {
                absolute_path: PathBuf::from("/tests/fixtures/simple_app/packs/foo/app/services/bar/foo.rb"),
                unresolved_references: vec![UnresolvedReference {
                    name: "Bar".to_owned(),
                    namespace_path: vec!["Foo".to_owned(), "Bar".to_owned()],
                    location: Range {
                        start_row: 8,
                        start_col: 22,
                        end_row: 8,
                        end_col: 25,
                    }
                }],
                definitions: vec![],
            }
        };

        let actual_serialized =
            serde_json::from_str::<CacheEntry>(&contents).unwrap();

        assert_eq!(expected_serialized, actual_serialized);

        teardown();
    }

    #[test]
    fn test_corrupt_cache() -> anyhow::Result<()> {
        let sha = "e57a05216069923190a4e03d264d9677";
        let corrupt_contents: String = String::from(
            r#"{
  "file_contents_digest":"e57a05216069923190a4e03d264d9677",
  "processed_file": 
}"#,
        );

        let cache_path = PathBuf::from("tests/fixtures/simple_app/tmp/cache/");
        fs::create_dir_all(&cache_path)
            .context("unable to create cache dir")?;
        let corrupt_file_path = cache_path.join(sha);
        fs::write(corrupt_file_path, corrupt_contents)
            .context("expected to write corrupt cache file")?;

        let lookup = CacheLookup::new(
            &cache_path,
            &PathBuf::from(
                "tests/fixtures/simple_app/packs/foo/app/services/foo/bar.rb",
            ),
        );

        let entry = CacheEntry::from_lookup(&lookup)?;
        assert!(entry.is_none());

        Ok(())
    }

    #[test]
    fn test_entry_from_another_pks_version_is_a_miss() -> anyhow::Result<()> {
        let cache_dir = tempfile::tempdir()?;
        let cache = PerFileCache {
            cache_dir: cache_dir.path().to_path_buf(),
        };
        let path = PathBuf::from(
            "tests/fixtures/simple_app/packs/bar/app/services/bar.rb",
        );
        let processed_file = ProcessedFile {
            absolute_path: path.clone(),
            unresolved_references: vec![],
            definitions: vec![],
        };

        let CacheResult::Miss(empty_cache_entry) = cache.get(&path)? else {
            panic!("expected a miss on an empty cache");
        };
        cache.write(&empty_cache_entry, &processed_file)?;
        assert!(matches!(cache.get(&path)?, CacheResult::Processed(_)));

        // The bare content digest is what entries held before they recorded
        // the pks version. No stat forces the digest comparison, and the
        // file's own stat takes the fast path; both must miss.
        let content_digest = file_content_digest(&path)?;
        for source_stat in [None, empty_cache_entry.source_stat] {
            for file_contents_digest in
                [content_digest.clone(), format!("{content_digest}-0.0.0")]
            {
                let entry = CacheEntry {
                    file_contents_digest,
                    source_stat,
                    processed_file: processed_file.clone(),
                };
                fs::write(
                    &empty_cache_entry.cache_file_path,
                    serde_json::to_string(&entry)?,
                )?;
                assert!(
                    matches!(cache.get(&path)?, CacheResult::Miss(_)),
                    "served an entry with digest {} and stat {:?}",
                    entry.file_contents_digest,
                    entry.source_stat
                );
            }
        }

        Ok(())
    }
}
