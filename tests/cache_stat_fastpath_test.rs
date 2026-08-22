use assert_cmd::cargo::cargo_bin_cmd;
use std::error::Error;
use std::fs;
use std::path::{Path, PathBuf};

mod common;

/// The stat-based cache fast path skips reading and hashing a source file when
/// its mtime and length are unchanged. These tests pin the behaviors that make
/// that safe: a changed file must still invalidate, and an entry with no recorded
/// stat must still be usable.
///
/// Uses `common::Fixture` so each test gets a private copy -- these tests write
/// caches and edit source files, which is exactly the shared-state hazard that
/// helper exists to remove.
fn fixture_app() -> Result<common::Fixture, Box<dyn Error>> {
    let fixture = common::Fixture::new("simple_app");

    // The fixture ships with the cache disabled; these tests are about the cache.
    let packwerk_yml = fixture.path("packwerk.yml");
    let contents = fs::read_to_string(&packwerk_yml)?;
    fs::write(
        &packwerk_yml,
        contents.replace("cache: false", "cache: true"),
    )?;

    Ok(fixture)
}

fn check(app: &Path) -> Result<String, Box<dyn Error>> {
    let output = cargo_bin_cmd!("pks")
        .arg("--project-root")
        .arg(app)
        .arg("check")
        .output()?;
    Ok(String::from_utf8(output.stdout)?)
}

/// Runs `pks` with arbitrary arguments against the fixture.
fn run(
    app: &Path,
    args: &[&str],
) -> Result<(String, Option<i32>), Box<dyn Error>> {
    let output = cargo_bin_cmd!("pks")
        .arg("--project-root")
        .arg(app)
        .args(args)
        .output()?;
    let mut combined = String::from_utf8(output.stdout)?;
    combined.push_str(&String::from_utf8(output.stderr)?);
    Ok((combined, output.status.code()))
}

/// `pks check` emits violations in `HashSet` iteration order, and `RandomState`
/// is seeded per process, so the order of otherwise identical output varies from
/// run to run. Compare sorted lines so these tests assert on content rather than
/// on an ordering the tool does not currently guarantee.
fn check_sorted(app: &Path) -> Result<Vec<String>, Box<dyn Error>> {
    let mut lines: Vec<String> = check(app)?
        .lines()
        .filter(|l| !l.trim().is_empty())
        .map(str::to_owned)
        .collect();
    lines.sort();
    Ok(lines)
}

/// Every cache entry whose contents we hashed also records a stat, and reading
/// the entry back on a second run produces identical results.
fn cache_entries(app: &Path) -> Vec<(PathBuf, serde_json::Value)> {
    let mut found = Vec::new();
    let mut stack = vec![app.join("tmp/cache")];
    while let Some(dir) = stack.pop() {
        let Ok(entries) = fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
            } else if let Ok(text) = fs::read_to_string(&path) {
                if let Ok(json) =
                    serde_json::from_str::<serde_json::Value>(&text)
                {
                    if json.get("file_contents_digest").is_some() {
                        found.push((path, json));
                    }
                }
            }
        }
    }
    found
}

#[test]
fn test_cache_records_source_stat() -> Result<(), Box<dyn Error>> {
    let fixture = fixture_app()?;
    let app = fixture.root();

    check(app)?;

    let entries = cache_entries(app);
    assert!(
        !entries.is_empty(),
        "expected the run to populate the cache"
    );

    for (path, entry) in &entries {
        let stat = entry.get("source_stat").unwrap_or_else(|| {
            panic!("cache entry {} has no source_stat", path.display())
        });
        assert!(
            stat.get("mtime_ns").and_then(|v| v.as_u64()).is_some(),
            "cache entry {} has no mtime_ns",
            path.display()
        );
        assert!(
            stat.get("len").and_then(|v| v.as_u64()).is_some(),
            "cache entry {} has no len",
            path.display()
        );
    }

    Ok(())
}

/// A warm run must produce byte-identical output to the cold run that populated
/// the cache. This is the fast path actually being exercised.
#[test]
fn test_warm_run_matches_cold_run() -> Result<(), Box<dyn Error>> {
    let fixture = fixture_app()?;
    let app = fixture.root();

    let cold = check_sorted(app)?;
    let warm = check_sorted(app)?;

    assert_eq!(cold, warm, "warm cache changed the result");
    assert!(
        cold.iter().any(|l| l.contains("Dependency violation")),
        "fixture should report violations, got: {cold:?}"
    );

    Ok(())
}

/// Editing a file changes both its length and mtime, so the fast path must miss
/// and the file must be reparsed.
#[test]
fn test_edited_file_invalidates_cache() -> Result<(), Box<dyn Error>> {
    let fixture = fixture_app()?;
    let app = fixture.root();

    let before = check(app)?;
    assert!(before.contains("::Bar"), "expected a ::Bar violation");

    // Point foo.rb at a constant that does not exist in another pack, which
    // removes the cross-pack reference and therefore the violations.
    let foo = fixture.path("packs/foo/app/services/foo.rb");
    let contents = fs::read_to_string(&foo)?;
    fs::write(&foo, contents.replace("Bar", "SomethingLocal"))?;

    let after = check(app)?;
    assert_ne!(
        before, after,
        "editing a source file did not invalidate the cache"
    );
    assert!(
        !after.contains("::Bar"),
        "stale ::Bar violation survived an edit: {after}"
    );

    Ok(())
}

/// A same-length edit still moves the mtime, so it must invalidate too. This is
/// the case a length-only check would miss.
#[test]
fn test_same_length_edit_invalidates_cache() -> Result<(), Box<dyn Error>> {
    let fixture = fixture_app()?;
    let app = fixture.root();

    let before = check(app)?;
    assert!(before.contains("::Bar"));

    let foo = fixture.path("packs/foo/app/services/foo.rb");
    let contents = fs::read_to_string(&foo)?;
    // "Bar" -> "Baz": identical byte length, different content.
    let edited = contents.replace("Bar", "Baz");
    assert_eq!(contents.len(), edited.len(), "edit changed the file length");
    fs::write(&foo, edited)?;

    let after = check(app)?;
    assert!(
        !after.contains("::Bar"),
        "a same-length edit was not detected: {after}"
    );

    Ok(())
}

/// Entries written by packwerk have no `source_stat`. Those must still be
/// honored via the content digest rather than treated as a miss, and they get
/// upgraded in place so later runs take the fast path.
#[test]
fn test_entry_without_stat_is_still_valid() -> Result<(), Box<dyn Error>> {
    let fixture = fixture_app()?;
    let app = fixture.root();

    let before = check_sorted(app)?;

    // Strip source_stat from every entry to simulate a packwerk-written cache.
    for (path, mut entry) in cache_entries(app) {
        entry
            .as_object_mut()
            .expect("cache entry is a json object")
            .remove("source_stat");
        fs::write(&path, serde_json::to_string(&entry)?)?;
    }

    let after = check_sorted(app)?;
    assert_eq!(
        before, after,
        "a cache entry without source_stat produced a different result"
    );

    // The entries should have been rewritten with a stat, so the next run can
    // use the fast path.
    for (path, entry) in cache_entries(app) {
        assert!(
            entry.get("source_stat").is_some(),
            "entry {} was not upgraded with a stat",
            path.display()
        );
    }

    Ok(())
}

/// A filesystem that reports whole-second mtimes cannot tell us about an edit
/// made within the same second, so the stat must not be recorded at all and the
/// digest must carry the entry instead.
///
/// Simulated by forcing exact-second mtimes on the source files, which is what a
/// one-second-granularity filesystem (some Docker bind mounts, NFS, SMB) reports
/// for everything.
#[test]
fn test_whole_second_mtime_is_not_trusted() -> Result<(), Box<dyn Error>> {
    let fixture = fixture_app()?;
    let app = fixture.root();

    // Round every source file's mtime down to a whole second before the first
    // run, so no entry is ever written with a stat. This must cover everything
    // pks caches, not just .rb -- simple_app also contains an .erb, and missing
    // it leaves one entry with a fine-grained stat.
    for path in source_files(app) {
        set_whole_second_mtime(&path)?;
    }

    let before = check_sorted(app)?;

    let entries = cache_entries(app);
    assert!(!entries.is_empty(), "expected a populated cache");
    for (path, entry) in &entries {
        assert!(
            entry.get("source_stat").is_none(),
            "entry {} recorded a whole-second stat, which cannot detect a \
             same-second edit",
            path.display()
        );
    }

    // The digest still has to do its job: a second run agrees, and an edit is
    // still caught even though no stat is available.
    assert_eq!(
        before,
        check_sorted(app)?,
        "warm run disagreed with cold run"
    );

    let foo = fixture.path("packs/foo/app/services/foo.rb");
    let contents = fs::read_to_string(&foo)?;
    fs::write(&foo, contents.replace("Bar", "Baz"))?;
    set_whole_second_mtime(&foo)?;

    assert!(
        !check(app)?.contains("::Bar"),
        "an edit went undetected when the mtime carried no sub-second part"
    );

    Ok(())
}

/// Every file pks will parse and cache under `root`, excluding the cache itself.
fn source_files(root: &Path) -> Vec<PathBuf> {
    let mut found = Vec::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let Ok(entries) = fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                // Skip the cache; only source files matter here.
                if path.file_name().is_some_and(|n| n == "tmp") {
                    continue;
                }
                stack.push(path);
            } else if path
                .extension()
                .is_some_and(|e| e == "rb" || e == "erb" || e == "rake")
            {
                found.push(path);
            }
        }
    }
    found
}

/// Give a file a whole-second mtime -- what a filesystem with one-second
/// granularity reports for everything.
///
/// The same fixed timestamp for every file is deliberate, not laziness. It means
/// a file keeps its exact mtime across an edit, so the edit changes neither the
/// mtime nor (for a same-length change) the length. That is the adversarial
/// case: if the sub-second guard stopped working, the stat would compare equal
/// and the stale entry would be served. Rounding each file's own mtime down
/// would instead give the edited file a *later* second, which the stat could
/// catch on its own -- and the test would pass without proving anything.
fn set_whole_second_mtime(path: &Path) -> Result<(), Box<dyn Error>> {
    // `touch -t` takes [[CC]YY]MMDDhhmm[.SS], which has no sub-second field.
    let status = std::process::Command::new("touch")
        .arg("-t")
        .arg("202601011200.00")
        .arg(path)
        .status()?;
    assert!(status.success(), "touch failed for {}", path.display());
    Ok(())
}

/// A stat that no longer matches, with contents that do -- what a `git checkout`
/// or a `touch` produces. The digest must rescue the entry rather than forcing a
/// reparse, and the stale stat must be corrected in place so the next run is
/// back on the fast path.
///
/// This is the one branch where a write failure is tolerated rather than
/// propagated, so leaving it untested would mean the least observable code in
/// the change is also the least defended.
#[test]
fn test_stale_stat_with_matching_digest_is_repaired(
) -> Result<(), Box<dyn Error>> {
    let fixture = fixture_app()?;
    let app = fixture.root();

    let before = check_sorted(app)?;

    // Corrupt only the stat. The digest still describes the file correctly, so
    // this is "the file looks touched but is byte-identical".
    let entries = cache_entries(app);
    assert!(!entries.is_empty(), "expected a populated cache");
    for (path, mut entry) in entries {
        entry.as_object_mut().expect("json object").insert(
            "source_stat".to_string(),
            serde_json::json!({ "mtime_ns": 1, "len": 999_999 }),
        );
        fs::write(&path, serde_json::to_string(&entry)?)?;
    }

    let after = check_sorted(app)?;
    assert_eq!(
        before, after,
        "a stale stat with a matching digest changed the result"
    );

    // Every entry should now carry the file's real stat again.
    for (path, entry) in cache_entries(app) {
        let stat = entry
            .get("source_stat")
            .unwrap_or_else(|| panic!("{} lost its stat", path.display()));
        assert_ne!(
            stat.get("mtime_ns").and_then(|v| v.as_u64()),
            Some(1),
            "entry {} kept its stale mtime instead of being repaired",
            path.display()
        );
        assert_ne!(
            stat.get("len").and_then(|v| v.as_u64()),
            Some(999_999),
            "entry {} kept its stale length instead of being repaired",
            path.display()
        );
    }

    Ok(())
}

/// `pks update` writes `package_todo.yml` to disk, so a stale cache here does not
/// just print the wrong answer -- it persists it. Every other fixture in the
/// suite runs with `cache: false`, so this path had no coverage at all.
#[test]
fn test_update_is_correct_on_a_warm_cache() -> Result<(), Box<dyn Error>> {
    let fixture = fixture_app()?;
    let app = fixture.root();
    let todo = fixture.path("packs/foo/package_todo.yml");

    // Cold: records the ::Bar violation.
    run(app, &["update"])?;
    let recorded = fs::read_to_string(&todo)?;
    assert!(
        recorded.contains("::Bar"),
        "expected the cold update to record ::Bar, got: {recorded}"
    );

    // Same-length edit that removes the violation entirely.
    let foo = fixture.path("packs/foo/app/services/foo.rb");
    let contents = fs::read_to_string(&foo)?;
    let edited = contents.replace("Bar", "Baz");
    assert_eq!(contents.len(), edited.len());
    fs::write(&foo, edited)?;

    // Warm: must notice, and must rewrite what it already put on disk.
    run(app, &["update"])?;
    let after = fs::read_to_string(&todo).unwrap_or_default();
    assert!(
        !after.contains("::Bar"),
        "a warm-cache update left a stale violation on disk: {after}"
    );

    Ok(())
}

/// The experimental parser writes to `tmp/cache/packwerk/experimental` rather
/// than `.../zeitwerk`, but shares this cache implementation. A regression that
/// only affected that subdirectory would otherwise be invisible.
#[test]
fn test_experimental_parser_uses_the_fast_path_correctly(
) -> Result<(), Box<dyn Error>> {
    let fixture = fixture_app()?;
    let app = fixture.root();

    let (cold, _) = run(app, &["--experimental-parser", "check"])?;
    assert!(cold.contains("::Bar"), "expected violations, got: {cold}");

    assert!(
        app.join("tmp/cache/packwerk/experimental").is_dir(),
        "expected the experimental parser to use its own cache directory"
    );
    assert!(
        cache_entries(app)
            .iter()
            .all(|(_, e)| e.get("source_stat").is_some()),
        "experimental cache entries should record a stat too"
    );

    let foo = fixture.path("packs/foo/app/services/foo.rb");
    let contents = fs::read_to_string(&foo)?;
    fs::write(&foo, contents.replace("Bar", "Baz"))?;

    let (warm, _) = run(app, &["--experimental-parser", "check"])?;
    assert!(
        !warm.contains("::Bar"),
        "experimental parser served a stale cache entry: {warm}"
    );

    Ok(())
}

/// `source_stat` is deserialized from a file anyone can edit, and a partial write
/// can leave it malformed. Every shape below must degrade to "no usable stat"
/// and be carried by the digest -- never crash, never silently drop a violation.
#[test]
fn test_malformed_source_stat_degrades_to_the_digest(
) -> Result<(), Box<dyn Error>> {
    let fixture = fixture_app()?;
    let app = fixture.root();

    let before = check_sorted(app)?;

    let malformed = [
        "\"banana\"",                        // not an object
        "{\"mtime_ns\":\"nope\",\"len\":1}", // wrong field type
        "{\"mtime_ns\":1}",                  // missing field
        "null",
        "{\"mtime_ns\":-5,\"len\":1}", // negative, cannot be a u64
    ];

    for (i, (path, entry)) in cache_entries(app).into_iter().enumerate() {
        // Swap the whole stat value for a malformed one, cycling the shapes.
        // Serializing a placeholder and substituting it keeps the surrounding
        // entry byte-for-byte valid, so only the stat is under test.
        let mut with_placeholder = entry;
        with_placeholder
            .as_object_mut()
            .expect("json object")
            .insert("source_stat".into(), serde_json::json!("PLACEHOLDER"));
        let text = serde_json::to_string(&with_placeholder)?
            .replace("\"PLACEHOLDER\"", malformed[i % malformed.len()]);
        fs::write(&path, text)?;
    }

    let (after_text, code) = run(app, &["check"])?;
    assert!(
        !after_text.contains("panicked"),
        "a malformed source_stat panicked: {after_text}"
    );
    assert_eq!(
        code,
        Some(1),
        "expected the usual violations-found exit, got {code:?}: {after_text}"
    );
    assert_eq!(
        before,
        check_sorted(app)?,
        "a malformed source_stat changed the result"
    );

    Ok(())
}
