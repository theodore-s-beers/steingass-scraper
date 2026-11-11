//! Offline reconstruction from the immutable HTML backup.
//!
//! Run from the repository root, with Pandoc 3.13 available on PATH:
//!
//! ```text
//! cargo run --locked --bin rebuild -- target/rebuilt
//! cargo run --locked --bin rebuild -- compare target/rebuilt/entries.sqlite entries.sqlite
//! cargo run --locked --bin rebuild -- compare target/rebuilt/entries_slim.sqlite entries_slim.sqlite
//! ```
//!
//! The output directory must not exist. A failed run may leave an incomplete
//! directory; only a run that exits successfully and ends its audit.txt with
//! `Complete` has produced both databases. The command never publishes or replaces
//! the checked-in files, and never fetches pages. Plain `cargo run` remains inert.
//!
//! Reconstruction order: read original HTML -> exact editorial HTML corrections
//! -> existing extraction/normalization -> exact field overrides -> abjad values
//! -> full database -> slim projection. Corrections are in `src/corrections.rs`.
//! Add an entry ID, exact before/after values, and a reason or issue reference there.
//! Corrections apply in list order, so subsequent edits can require a prior edit
//! as their before-value. Changing HTML may require revisiting field overrides.
//! Expected-value guards deliberately stop the build on conversion drift; do not
//! update their expectations without reviewing the differences.
//!
//! Targets are used only by `compare`, never as reconstruction inputs. Comparisons
//! check every stored value by ID/column, not SQLite byte layout or file hashes.
//! The original backup (first saved in 8c92d96246ff) has SHA-256
//! `ce85ce9e105c5a38a87f79d502ea029f5380a3e5df278b9d208d5c08f3e0e496`.
//! Pandoc's version is recorded in each audit; 3.13 reproduces the reference corpus.
//!
//! Both outputs are vacuumed before success is reported. Repeated builds produce
//! identical SQLite bytes for the same input, corrections, code, and toolchain.
//! Use `--locked` to keep the bundled SQLite version fixed; Pandoc 3.13 is checked
//! at startup. Dependency/toolchain upgrades require rechecking reproducibility.
//! This guarantee covers the SQLite files, not path-dependent audit text.
use std::collections::{BTreeMap, BTreeSet};
use std::io::{BufRead, BufReader, Read, Write};
use std::path::Path;
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};

use abjad::{Abjad, AbjadPrefs};
use anyhow::{Context, Result, bail, ensure};
use rusqlite::{Connection, OpenFlags, params, types::Value};
use scraper::Html;

use crate::corrections::{Correction, FIELDS, HTML};
use crate::{Entry, defs, hw_full, hw_lat, hw_per, langs};

const SCHEMA: &str = "CREATE TABLE entries (
    id INTEGER NOT NULL PRIMARY KEY,
    page INTEGER NOT NULL,
    raw_html TEXT NOT NULL,
    lang TEXT NOT NULL,
    headword_full TEXT NOT NULL,
    headword_persian TEXT NOT NULL,
    abjad INTEGER NOT NULL,
    headword_latin TEXT NOT NULL,
    definitions TEXT NOT NULL
)";
const SHARED_COLUMNS: &str =
    "id, page, lang, headword_full, headword_persian, abjad, headword_latin, definitions";

fn read_only(path: &Path) -> Result<Connection> {
    Ok(Connection::open_with_flags(
        path,
        OpenFlags::SQLITE_OPEN_READ_ONLY,
    )?)
}

// Keep startup costs out of the per-entry loop without combining documents.
struct Pandoc {
    child: Child,
    input: Option<ChildStdin>,
    output: BufReader<ChildStdout>,
}

impl Pandoc {
    fn start() -> Result<Self> {
        let mut child = Command::new("pandoc")
            .args(["lua", "-e", include_str!("pandoc_stream.lua")])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .spawn()
            .context("starting Pandoc (requires the `pandoc lua` command)")?;
        let input = child.stdin.take();
        let output = BufReader::new(child.stdout.take().unwrap());
        Ok(Self {
            child,
            input,
            output,
        })
    }

    fn convert(&mut self, html: &str) -> Result<String> {
        let input = self.input.as_mut().context("Pandoc is closed")?;
        writeln!(input, "{}", html.len())?;
        input.write_all(html.as_bytes())?;
        input.flush()?;
        let mut length = String::new();
        ensure!(
            self.output.read_line(&mut length)? > 0,
            "Pandoc stopped before responding"
        );
        let length: usize = length
            .trim()
            .parse()
            .context("invalid Pandoc response length")?;
        ensure!(
            length < 32 * 1024 * 1024,
            "unexpectedly large Pandoc response"
        );
        let mut bytes = vec![0; length];
        self.output.read_exact(&mut bytes)?;
        let output = String::from_utf8(bytes)?;
        Ok(output
            .trim()
            .trim_start_matches(',')
            .trim_start()
            .to_owned())
    }

    fn finish(&mut self) -> Result<()> {
        self.input.take();
        ensure!(self.child.wait()?.success(), "Pandoc failed");
        Ok(())
    }
}

impl Drop for Pandoc {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

fn apply(value: &mut String, correction: &Correction) -> Result<()> {
    ensure!(
        value == correction.before,
        "entry {} {}: correction precondition failed ({})\nexpected: {:?}\nactual: {:?}",
        correction.id,
        correction.field,
        correction.reason,
        correction.before,
        value
    );
    correction.after.clone_into(value);
    Ok(())
}

fn extract(id: u32, page: u16, html: String, pandoc: &mut Pandoc) -> Result<Entry> {
    let parsed = Html::parse_fragment(&html);
    let mut convert = |input: &str| pandoc.convert(input);
    let entry = Entry {
        page,
        lang: langs::get_lang(&parsed),
        headword_full: hw_full::select_full_headword_with(&parsed, &mut convert)?,
        headword_persian: hw_per::get_hw_per(&parsed),
        headword_latin: hw_lat::get_hw_lat_with(&parsed, &mut convert)?,
        definitions: defs::except_headword_with(&html, &mut convert)?,
        raw_html: html,
    };
    ensure!(
        !entry.headword_full.is_empty(),
        "empty headword in entry {id}"
    );
    Ok(entry)
}

/// Create fresh full/slim databases. Never opens the source or reference for writing.
pub fn rebuild(backup: &Path, destination: &Path) -> Result<()> {
    let source = read_only(backup).context("opening original HTML backup read-only")?;
    let version = Command::new("pandoc").arg("--version").output()?;
    ensure!(version.status.success(), "pandoc --version failed");
    let version = String::from_utf8(version.stdout)?;
    ensure!(
        version.lines().next() == Some("pandoc 3.13"),
        "reproducible rebuilding requires Pandoc 3.13; found {}",
        version.lines().next().unwrap_or("unknown version")
    );
    // create_dir deliberately rejects an existing destination, even an empty one.
    std::fs::create_dir(destination).context("output directory must not already exist")?;
    let mut audit = std::fs::File::create(destination.join("audit.txt"))?;
    writeln!(audit, "Source: {}\n{}", backup.display(), version)?;
    writeln!(audit, "SQLite: {}", rusqlite::version())?;
    let mut output = Connection::open(destination.join("entries.sqlite"))?;
    configure_output(&output, "main")?;
    output.execute_batch(SCHEMA)?;
    let transaction = output.transaction()?;
    let mut pandoc = Pandoc::start()?;
    let mut seen = BTreeSet::new();
    let mut count = 0;
    let mut statement = source.prepare("SELECT id, page, raw_html FROM entries ORDER BY id")?;
    let rows = statement.query_map([], |row| {
        Ok((
            row.get::<_, u32>(0)?,
            row.get::<_, u16>(1)?,
            row.get::<_, String>(2)?,
        ))
    })?;
    for row in rows {
        let (id, page, mut html) = row?;
        for correction in HTML.iter().filter(|c| c.id == id) {
            apply(&mut html, correction)?;
            writeln!(audit, "{id}\traw_html\t{}", correction.reason)?;
            seen.insert((id, correction.field));
        }
        let mut entry = extract(id, page, html, &mut pandoc)
            .with_context(|| format!("extracting entry {id} (page {page})"))?;
        for correction in FIELDS.iter().filter(|c| c.id == id) {
            let value = match correction.field {
                "headword_persian" => &mut entry.headword_persian,
                "headword_latin" => &mut entry.headword_latin,
                "headword_full" => &mut entry.headword_full,
                "definitions" => &mut entry.definitions,
                field => bail!("unsupported correction field: {field}"),
            };
            apply(value, correction)?;
            writeln!(audit, "{id}\t{}\t{}", correction.field, correction.reason)?;
            seen.insert((id, correction.field));
        }
        let abjad = entry.headword_persian.abjad(AbjadPrefs::default());
        transaction.execute(
            "INSERT INTO entries VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![
                id,
                entry.page,
                entry.raw_html,
                entry.lang.as_str(),
                entry.headword_full,
                entry.headword_persian,
                abjad,
                entry.headword_latin,
                entry.definitions
            ],
        )?;
        count += 1;
        if count % 5000 == 0 {
            eprintln!("Rebuilt {count} entries");
        }
    }
    for correction in HTML.iter().chain(FIELDS) {
        ensure!(
            seen.contains(&(correction.id, correction.field)),
            "correction for missing entry {}",
            correction.id
        );
    }
    pandoc.finish()?;
    transaction.commit()?;
    write_slim(&output, destination)?;
    // Run outside the insertion transaction, after both schemas are populated.
    output.execute_batch("VACUUM main; VACUUM slim; DETACH DATABASE slim;")?;
    output.close().map_err(|(_, error)| error)?;
    writeln!(audit, "Vacuumed: entries.sqlite, entries_slim.sqlite")?;
    writeln!(audit, "Complete: {count} entries")?;
    eprintln!("Complete: {count} entries in {}", destination.display());
    Ok(())
}

// Fix format choices instead of inheriting SQLite compile-time defaults.
fn configure_output(output: &Connection, schema: &str) -> Result<()> {
    output.execute_batch(&format!(
        "PRAGMA {schema}.page_size = 4096;
         PRAGMA {schema}.auto_vacuum = NONE;
         PRAGMA {schema}.encoding = 'UTF-8';
         PRAGMA {schema}.journal_mode = DELETE;
         PRAGMA {schema}.synchronous = FULL;"
    ))?;
    Ok(())
}

// Generate the slim database from the full output, not a second extraction.
fn write_slim(output: &Connection, destination: &Path) -> Result<()> {
    output.execute(
        "ATTACH DATABASE ?1 AS slim",
        [destination
            .join("entries_slim.sqlite")
            .to_str()
            .context("invalid output path")?],
    )?;
    configure_output(output, "slim")?;
    output.execute_batch(&format!(
        "CREATE TABLE slim.entries (
            id INTEGER NOT NULL PRIMARY KEY, page INTEGER NOT NULL, lang TEXT NOT NULL,
            headword_full TEXT NOT NULL, headword_persian TEXT NOT NULL, abjad INTEGER NOT NULL,
            headword_latin TEXT NOT NULL, definitions TEXT NOT NULL);
         INSERT INTO slim.entries SELECT {SHARED_COLUMNS} FROM main.entries ORDER BY id;"
    ))?;
    Ok(())
}

type StoredRows = BTreeMap<i64, Vec<Value>>;

/// Compare all stored values by ID and column name, ignoring SQLite file layout.
pub fn compare(actual: &Path, expected: &Path) -> Result<()> {
    fn read(path: &Path) -> Result<(Vec<String>, StoredRows)> {
        let connection = read_only(path)?;
        let mut statement = connection.prepare("SELECT * FROM entries ORDER BY id")?;
        let columns: Vec<String> = statement.column_names().map(ToString::to_string).collect();
        let id_column = columns
            .iter()
            .position(|c| c == "id")
            .context("missing id column")?;
        let rows = statement
            .query_map([], |row| {
                let values = (0..columns.len())
                    .map(|i| row.get(i))
                    .collect::<rusqlite::Result<Vec<Value>>>()?;
                Ok((row.get::<_, i64>(id_column)?, values))
            })?
            .collect::<rusqlite::Result<BTreeMap<_, _>>>()?;
        Ok((columns, rows))
    }
    let (columns, actual) = read(actual)?;
    let (expected_columns, expected) = read(expected)?;
    ensure!(
        columns.iter().collect::<BTreeSet<_>>() == expected_columns.iter().collect(),
        "column sets differ"
    );
    let indices: Vec<usize> = columns
        .iter()
        .map(|c| expected_columns.iter().position(|e| e == c).unwrap())
        .collect();
    let mut differences = 0;
    for id in actual
        .keys()
        .chain(expected.keys().filter(|id| !actual.contains_key(id)))
    {
        if let (Some(a), Some(e)) = (actual.get(id), expected.get(id)) {
            for (index, column) in columns.iter().enumerate() {
                if a[index] != e[indices[index]] {
                    println!(
                        "{id}\t{column}\tactual={:?}\texpected={:?}",
                        a[index], e[indices[index]]
                    );
                    differences += 1;
                }
            }
        } else {
            println!("{id}\tmissing row in one database");
            differences += 1;
        }
    }
    ensure!(differences == 0, "{differences} differences");
    println!(
        "All {} rows and {} columns match",
        actual.len(),
        columns.len()
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn corrections_reject_unexpected_values() {
        for correction in HTML.iter().chain(FIELDS) {
            let mut value = correction.before.to_owned();
            apply(&mut value, correction).unwrap();
            assert_eq!(value, correction.after);
            // A second application must fail rather than hide a changed input.
            assert!(apply(&mut value, correction).is_err());
        }
    }

    #[test]
    fn rebuilds_corrected_entries_without_changing_source() {
        let directory = tempfile::tempdir().unwrap();
        let backup = directory.path().join("backup.sqlite");
        let source = Connection::open(&backup).unwrap();
        source
            .execute_batch(
                "CREATE TABLE entries (id INTEGER PRIMARY KEY, page INTEGER, raw_html TEXT)",
            )
            .unwrap();
        let original = read_only(Path::new("html_backup.sqlite")).unwrap();
        let ids: BTreeSet<_> = HTML.iter().chain(FIELDS).map(|c| c.id).collect();
        for id in &ids {
            let (page, html): (u16, String) = original
                .query_row(
                    "SELECT page, raw_html FROM entries WHERE id = ?",
                    [id],
                    |row| Ok((row.get(0)?, row.get(1)?)),
                )
                .unwrap();
            source
                .execute(
                    "INSERT INTO entries VALUES (?1, ?2, ?3)",
                    params![id, page, html],
                )
                .unwrap();
        }
        drop(source);
        let bytes = std::fs::read(&backup).unwrap();
        let destination = directory.path().join("rebuilt");
        rebuild(&backup, &destination).unwrap();
        assert_eq!(std::fs::read(&backup).unwrap(), bytes);
        assert!(rebuild(&backup, &destination).is_err());
        let repeated = directory.path().join("repeated");
        rebuild(&backup, &repeated).unwrap();

        let reference = read_only(Path::new("entries.sqlite")).unwrap();
        for filename in ["entries.sqlite", "entries_slim.sqlite"] {
            assert_eq!(
                std::fs::read(destination.join(filename)).unwrap(),
                std::fs::read(repeated.join(filename)).unwrap(),
                "Repeated builds must produce identical bytes: {filename}"
            );
            let output = read_only(&destination.join(filename)).unwrap();
            for (pragma, expected) in [
                ("page_size", 4096),
                ("freelist_count", 0),
                ("auto_vacuum", 0),
                ("schema_version", 2),
            ] {
                // CREATE TABLE sets schema_version to 1; VACUUM increments it.
                let actual: i64 = output
                    .query_row(&format!("PRAGMA {pragma}"), [], |r| r.get(0))
                    .unwrap();
                assert_eq!(actual, expected, "{filename}: {pragma}");
            }
            let statement = output.prepare("SELECT * FROM entries").unwrap();
            let columns = statement.column_names().collect::<Vec<_>>().join(", ");
            let column_count = statement.column_count();
            let count: u32 = output
                .query_row("SELECT COUNT(*) FROM entries", [], |r| r.get(0))
                .unwrap();
            assert_eq!(usize::try_from(count).unwrap(), ids.len());
            for id in &ids {
                let read_row = |row: &rusqlite::Row<'_>| {
                    (0..column_count)
                        .map(|i| row.get::<_, Value>(i))
                        .collect::<rusqlite::Result<Vec<_>>>()
                };
                let query = format!("SELECT {columns} FROM entries WHERE id = ?");
                let actual = output.query_row(&query, [id], read_row).unwrap();
                let expected = reference.query_row(&query, [id], read_row).unwrap();
                assert_eq!(actual, expected, "{filename}, entry {id}");
            }
        }
    }

    #[test]
    fn comparison_detects_changes_even_when_counts_match() {
        let directory = tempfile::tempdir().unwrap();
        let first = directory.path().join("first.sqlite");
        let second = directory.path().join("second.sqlite");
        let a = Connection::open(&first).unwrap();
        let b = Connection::open(&second).unwrap();
        a.execute_batch("CREATE TABLE entries (id INTEGER PRIMARY KEY, text TEXT); INSERT INTO entries VALUES (1, 'original');").unwrap();
        b.execute_batch("CREATE TABLE entries (text TEXT, id INTEGER PRIMARY KEY); INSERT INTO entries VALUES ('original', 1);").unwrap();
        compare(&first, &second).unwrap();
        b.execute("UPDATE entries SET text = 'changed'", [])
            .unwrap();
        assert!(compare(&first, &second).is_err());
        b.execute("UPDATE entries SET id = 2, text = 'original'", [])
            .unwrap();
        assert!(compare(&first, &second).is_err());
    }

    #[test]
    fn persistent_pandoc_matches_independent_processes() {
        let mut pandoc = Pandoc::start().unwrap();
        let source = read_only(Path::new("html_backup.sqlite")).unwrap();
        let mut statement = source
            .prepare(
                "SELECT raw_html FROM entries WHERE id IN (83, 479, 5267, 18269, 39208, 63287)",
            )
            .unwrap();
        for html in statement.query_map([], |r| r.get::<_, String>(0)).unwrap() {
            let html = html.unwrap();
            assert_eq!(
                pandoc.convert(&html).unwrap(),
                crate::pandoc(&html).unwrap()
            );
        }
        for html in [
            "",
            "<i>one</i>",
            "<a href='https://example.org'>link</a>",
            "<p>two</p><p>three</p>",
        ] {
            assert_eq!(pandoc.convert(html).unwrap(), crate::pandoc(html).unwrap());
        }
        pandoc.finish().unwrap();
    }
}
