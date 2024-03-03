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
    pub lang: Lang,
    pub headword_full: String,
    pub headword_persian: String,
    pub headword_latin: String,
    pub definitions: String,
}

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Clone, Copy, Hash, Default)]
pub enum Lang {
    #[default]
    Unmarked,

    Arabic,    // A
    English,   // E
    Greek,     // G
    Hindi,     // H
    Mongolian, // M
    Persian,   // P
    Russian,   // R
    Sanskrit,  // S
    Syriac,    // SY
    Turkish,   // T

    ArabicTurkish, // A T

    PersianArabic,    // a
    PersianMongolian, // m
    PersianTurkish,   // t

    PersianTurkishArabic, // t a
}

impl Lang {
    const fn as_str(self) -> &'static str {
        match self {
            Self::Unmarked => "Unmarked (i.e., Persian)",
            Self::Arabic => "Arabic",
            Self::English => "English",
            Self::Greek => "Greek",
            Self::Hindi => "Hindi",
            Self::Mongolian => "Mongolian",
            Self::Persian => "Persian",
            Self::Russian => "Russian",
            Self::Sanskrit => "Sanskrit",
            Self::Syriac => "Syriac",
            Self::Turkish => "Turkish",
            Self::ArabicTurkish => "Arabic & Turkish",
            Self::PersianArabic => "Arabic & Persian",
            Self::PersianMongolian => "Mongolian & Persian",
            Self::PersianTurkish => "Persian & Turkish",
            Self::PersianTurkishArabic => "Arabic & Persian & Turkish",
        }
    }
}

//
// Constants
//

pub const BAD_PAGES: [u16; 2] = [2, 41];
const PREFIX: &str = "https://dsal.uchicago.edu/cgi-bin/app/steingass_query.py?page=";
const _MIN_PAGE: u16 = 1;
const _MAX_PAGE: u16 = 1539;

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

#[must_use]
pub fn get_lang(parsed: &Html) -> Lang {
    let selector = Selector::parse("lang").unwrap();
    let mut lang = Lang::Unmarked;

    if let Some(result) = parsed.select(&selector).next() {
        let text: String = result.text().collect();
        let trimmed = text.trim();

        lang = match trimmed {
            "A" => Lang::Arabic,
            "E" => Lang::English,
            "G" => Lang::Greek,
            "H" => Lang::Hindi,
            "M" => Lang::Mongolian,
            "P" => Lang::Persian,
            "R" => Lang::Russian,
            "S" => Lang::Sanskrit,
            "SY" => Lang::Syriac,
            "T" => Lang::Turkish,
            "A T" => Lang::ArabicTurkish,
            "a" => Lang::PersianArabic,
            "m" => Lang::PersianMongolian,
            "t" => Lang::PersianTurkish,
            "t a" => Lang::PersianTurkishArabic,
            _ => panic!("Unrecognized language: {}", trimmed),
        };
    }

    lang
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
    let persian_text = pandoc(&persian.html())?;

    let latin_text = match parsed.select(&selector_i).next() {
        Some(latin) => pandoc(&latin.html())?,
        None => "N/A".to_owned(),
    };

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

//
// Tests
//

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tadbir_arabic() {
        let p290 = fetch_html(290).unwrap();
        let results = select_results(&p290);
        let tadbir = results[0].html();
        let parsed = Html::parse_fragment(&tadbir);
        let lang = get_lang(&parsed);
        assert_eq!(lang, Lang::Arabic);
    }
}
