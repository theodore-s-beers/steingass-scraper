#![warn(clippy::pedantic, clippy::nursery)]
#![allow(
    clippy::missing_errors_doc,
    clippy::missing_panics_doc,
    clippy::uninlined_format_args
)]

use core::str;
use std::io::Write;
use std::process::Command;

use regex::Regex;
use reqwest::blocking::get;
use rusqlite::Connection;
use scraper::{ElementRef, Html, Selector};
use tempfile::NamedTempFile;

//
// Types
//

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Clone, Hash, Default)]
pub struct Entry {
    pub page: u16,
    pub raw_html: String,
    pub headword_full: String,
    pub headword_persian: String,
    pub headword_latin: String,
    pub definitions: String,
}

//
// Constants
//

const PREFIX: &str = "https://dsal.uchicago.edu/cgi-bin/app/steingass_query.py?page=";
const _MIN_PAGE: u16 = 1;
const _MAX_PAGE: u16 = 1539;
pub const BAD_PAGES: [u16; 1] = [2];

//
// Public functions
//

pub fn ensure_table(conn: &Connection) -> Result<(), anyhow::Error> {
    conn.execute(
        "CREATE TABLE IF NOT EXISTS entries (
            id INTEGER NOT NULL PRIMARY KEY,
            page INTEGER NOT NULL,
            raw_html TEXT NOT NULL,
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

pub fn select_full_headword(parsed: &Html) -> Result<String, anyhow::Error> {
    let selector = Selector::parse("hw").unwrap();
    let hw_full = parsed.select(&selector).next().unwrap();
    let hw_full_text = pandoc(&hw_full.html())?;
    Ok(hw_full_text)
}

pub fn headword_parts(parsed: &Html) -> Result<(String, String), anyhow::Error> {
    let selector_pa = Selector::parse("hw pa").unwrap();
    let selector_i = Selector::parse("hw i").unwrap();

    let persian = parsed.select(&selector_pa).next().unwrap();
    let latin = parsed.select(&selector_i).next().unwrap();

    let persian_text = pandoc(&persian.html())?;
    let latin_text = pandoc(&latin.html())?;

    Ok((persian_text, latin_text))
}

pub fn except_headword(input: &str) -> Result<String, anyhow::Error> {
    let re_c = Regex::new("<c>.*</c>").unwrap();
    let re_hw = Regex::new("<hw>.*</hw>").unwrap();
    let re_lang = Regex::new("<lang>.*</lang>").unwrap();

    let without_c = re_c.replace_all(input, "");
    let without_hw = re_hw.replace_all(&without_c, "");
    let without_lang = re_lang.replace_all(&without_hw, "");

    let processed = pandoc(&without_lang)?;

    Ok(processed)
}

pub fn insert_row(conn: &Connection, entry: Entry) -> Result<(), anyhow::Error> {
    conn.execute(
        "INSERT INTO entries (
            page,
            raw_html,
            headword_full,
            headword_persian,
            headword_latin,
            definitions
        ) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        (
            entry.page,
            entry.raw_html,
            entry.headword_full,
            entry.headword_persian,
            entry.headword_latin,
            entry.definitions,
        ),
    )?;

    Ok(())
}

// Private functions

fn pandoc(input: &str) -> Result<String, anyhow::Error> {
    let mut tempfile = NamedTempFile::new()?;
    write!(tempfile, "{}", input)?;

    let pandoc = Command::new("pandoc")
        .arg(tempfile.path())
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
