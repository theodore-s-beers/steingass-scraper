#![warn(clippy::pedantic, clippy::nursery)]
#![allow(
    clippy::missing_errors_doc,
    clippy::missing_panics_doc,
    clippy::module_name_repetitions,
    clippy::uninlined_format_args
)]
#![feature(variant_count)]

use core::str;
use std::io::Write;
use std::process::Command;

use reqwest::blocking::get;
use rusqlite::Connection;
use scraper::{ElementRef, Html, Selector};
use tempfile::NamedTempFile;

pub mod charsets;
pub mod defs;
pub mod hw_full;
pub mod hw_lat;
pub mod hw_per;
pub mod langs;

use langs::Lang;

//
// Types
//

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Clone, Hash, Default)]
pub struct Entry {
    pub page: u16,
    pub raw_html: String,
    pub lang: Lang,
    pub headword_full: String,
    pub headword_persian: String,
    pub headword_latin: String,
    pub definitions: String,
}

//
// Constants
//

pub const MIN_PAGE: u16 = 1;
pub const MAX_PAGE: u16 = 1539;
pub const BAD_PAGES: [u16; 6] = [2, 41, 486, 520, 665, 666];

const PREFIX: &str = "https://dsal.uchicago.edu/cgi-bin/app/steingass_query.py?page=";

//
// Public functions
//

pub fn ensure_table(conn: &Connection) -> Result<(), anyhow::Error> {
    conn.execute(
        "CREATE TABLE IF NOT EXISTS entries (
            id INTEGER NOT NULL PRIMARY KEY,
            page INTEGER NOT NULL,
            raw_html TEXT NOT NULL,
            lang TEXT NOT NULL,
            headword_full TEXT NOT NULL,
            headword_persian TEXT NOT NULL,
            headword_latin TEXT NOT NULL,
            definitions TEXT NOT NULL
        )",
        [],
    )?;

    Ok(())
}

pub fn fetch_html(page: u16) -> Result<Html, anyhow::Error> {
    let url = format!("{}{}", PREFIX, page);
    let response_text = get(url)?.text()?;
    let parsed = Html::parse_document(&response_text);
    Ok(parsed)
}

#[must_use]
pub fn select_results(parsed: &Html) -> Vec<ElementRef> {
    let selector = Selector::parse("#results_display .container div").unwrap();
    parsed.select(&selector).collect()
}

pub fn count_page_entries(conn: &Connection, page: u16) -> Result<usize, anyhow::Error> {
    let mut stmt = conn.prepare("SELECT COUNT(*) FROM entries WHERE page = ?")?;
    let count: usize = stmt.query_row([page], |row| row.get(0))?;
    Ok(count)
}

pub fn insert_row(conn: &Connection, entry: Entry) -> Result<(), anyhow::Error> {
    conn.execute(
        "INSERT INTO entries (
            page,
            raw_html,
            lang,
            headword_full,
            headword_persian,
            headword_latin,
            definitions
        ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        (
            entry.page,
            entry.raw_html,
            entry.lang.as_str(),
            entry.headword_full,
            entry.headword_persian,
            entry.headword_latin,
            entry.definitions,
        ),
    )?;

    Ok(())
}

//
// Private functions
//

#[allow(clippy::let_and_return, clippy::similar_names)]
fn clean_simple(input: &str) -> String {
    let mut cleaned = input.trim().to_owned();

    let swaps_simple: [(char, &str); 22] = [
        ('\u{02BB}', "\u{2018}"), // Left turned comma to left single quote
        ('\u{02BC}', "\u{2019}"), // Weird apostrophe to right single quote
        ('\u{0320}', "\u{0331}"), // Minus sign below to macron below
        ('\u{0643}', "\u{06A9}"), // Arabic k to Persian k
        ('\u{0649}', "\u{06CC}"), // Alif maqsura to Persian y
        ('\u{064A}', "\u{06CC}"), // Arabic y to Persian y
        ('\u{066E}', "\u{0628}"), // Dotless b
        ('\u{0680}', "\u{067E}"), // Quad p
        ('\u{06B1}', "\u{06AF}"), // Ngoeh (?)
        ('\u{06BE}', "\u{0647}"), // H do-chashmeh
        ('\u{200D}', ""),         // Remove ZWJ
        ('\u{200F}', ""),         // Remove RLM
        ('\u{FB58}', "\u{067E}"), // P initial
        ('\u{FB59}', "\u{067E}"), // P medial
        ('\u{FB7D}', "\u{0686}"), // Ch medial
        ('\u{FB8A}', "\u{0698}"), // Zh isolated
        ('\u{FB8B}', "\u{0698}"), // Zh final
        ('\u{FB94}', "\u{06AF}"), // G initial
        ('\u{FBA9}', "\u{0647}"), // H medial
        ('\u{FE81}', "\u{0622}"), // Alif madda isolated
        ('\u{FE8A}', "\u{0626}"), // Hamza y
        ('\u{FEEB}', "\u{0647}"), // H initial
    ];

    for (from, to) in swaps_simple {
        cleaned = cleaned.replace(from, to);
    }

    let swaps_complex: [(&str, &str); 2] = [
        ("\u{0020}\u{064B}", "\u{064B}"), // Remove space before fathatayn
        ("\u{0065}\u{0306}", "\u{0115}"), // E breve
    ];

    for (from, to) in swaps_complex {
        cleaned = cleaned.replace(from, to);
    }

    cleaned
}

fn pandoc(input: &str) -> Result<String, anyhow::Error> {
    let mut file = NamedTempFile::new()?;
    write!(file, "{}", input)?;

    let pandoc = Command::new("pandoc")
        .arg(file.path())
        .arg("-f")
        .arg("html")
        .arg("-t")
        .arg("markdown_strict")
        .arg("--wrap=none")
        .output()?;

    let output = str::from_utf8(&pandoc.stdout)?;
    let cleaned = output.trim().trim_start_matches(',').trim_start();

    Ok(cleaned.to_owned())
}

//
// Tests
//

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn confirm_html() {
        let conn_dev = Connection::open("entries.sqlite").unwrap();
        let conn_backup = Connection::open("html_backup.sqlite").unwrap();

        let mut stmt_count_dev = conn_dev.prepare("SELECT COUNT(*) FROM entries").unwrap();
        let mut stmt_count_backup = conn_backup.prepare("SELECT COUNT(*) FROM entries").unwrap();

        let count_dev: u32 = stmt_count_dev.query_row([], |row| row.get(0)).unwrap();
        let count_backup: u32 = stmt_count_backup.query_row([], |row| row.get(0)).unwrap();

        assert_eq!(count_dev, count_backup);

        let mut stmt_entries_dev = conn_dev
            .prepare("SELECT id, raw_html FROM entries")
            .unwrap();

        let entry_iter_dev = stmt_entries_dev
            .query_map([], |row| {
                let id: u32 = row.get(0).unwrap();
                let raw_html: String = row.get(1).unwrap();
                Ok((id, raw_html))
            })
            .unwrap();

        for entry in entry_iter_dev {
            let (id, html_dev) = entry.unwrap();

            // Skipping problematic entry for abjad
            if html_dev.contains(".jpg") {
                continue;
            }

            // Skipping problematic entry for ātish-ālūd
            if html_dev.contains("Suffused with fire") {
                continue;
            }

            let mut stmt_entry_backup = conn_backup
                .prepare("SELECT raw_html FROM entries WHERE id = ?")
                .unwrap();

            let html_backup: String = stmt_entry_backup.query_row([id], |row| row.get(0)).unwrap();

            assert_eq!(html_dev, html_backup);
        }
    }

    #[test]
    fn min_max_pages() {
        let conn = Connection::open("entries.sqlite").unwrap();

        let mut stmt_min = conn.prepare("SELECT MIN(page) FROM entries").unwrap();
        let min: u16 = stmt_min.query_row([], |row| row.get(0)).unwrap();
        assert_eq!(min, MIN_PAGE);

        let mut stmt_max = conn.prepare("SELECT MAX(page) FROM entries").unwrap();
        let max: u16 = stmt_max.query_row([], |row| row.get(0)).unwrap();
        assert_eq!(max, MAX_PAGE);
    }

    #[test]
    fn page_count() {
        let total_pages = MAX_PAGE - MIN_PAGE + 1;
        let good_pages = total_pages - u16::try_from(BAD_PAGES.len()).unwrap();

        let conn = Connection::open("entries.sqlite").unwrap();
        let mut stmt = conn
            .prepare("SELECT COUNT(DISTINCT page) FROM entries")
            .unwrap();
        let count: u16 = stmt.query_row([], |row| row.get(0)).unwrap();

        assert_eq!(count, good_pages);
    }
}
