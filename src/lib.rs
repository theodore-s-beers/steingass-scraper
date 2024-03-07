#![warn(clippy::pedantic, clippy::nursery)]
#![allow(
    clippy::missing_errors_doc,
    clippy::missing_panics_doc,
    clippy::uninlined_format_args
)]
#![feature(variant_count)]

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

    Arabic,     // A, B (single-occurrence typo)
    English,    // E
    Greek,      // G
    Hebrew,     // HE
    Hindi,      // H
    Latin,      // L
    Mongolian,  // M
    Persian,    // P
    Portuguese, // PORT
    Russian,    // R
    Sanskrit,   // S
    Spanish,    // SP
    Syriac,     // SY
    Turkish,    // T
    Urdu,       // U

    ArabicGreek,   // A G
    ArabicTurkish, // A T

    PersianArabic,    // a, ā, o (single-occurrence typo), A a, A P
    PersianGreek,     // g
    PersianHindi,     // h
    PersianMongolian, // m
    PersianRussian,   // r
    PersianTurkish,   // t

    PersianArabicGreek,   // g a
    PersianArabicHindi,   // a h
    PersianArabicTurkish, // a t, t a, a p t
}

impl Lang {
    const fn as_str(self) -> &'static str {
        match self {
            Self::Unmarked => "Unmarked (i.e., Persian)",

            Self::Arabic => "Arabic",
            Self::English => "English",
            Self::Greek => "Greek",
            Self::Hebrew => "Hebrew",
            Self::Hindi => "Hindi",
            Self::Latin => "Latin",
            Self::Mongolian => "Mongolian",
            Self::Persian => "Persian",
            Self::Portuguese => "Portuguese",
            Self::Russian => "Russian",
            Self::Sanskrit => "Sanskrit",
            Self::Spanish => "Spanish",
            Self::Syriac => "Syriac",
            Self::Turkish => "Turkish",
            Self::Urdu => "Urdu",

            Self::ArabicGreek => "Arabic & Greek",
            Self::ArabicTurkish => "Arabic & Turkish",

            Self::PersianArabic => "Arabic & Persian",
            Self::PersianGreek => "Greek & Persian",
            Self::PersianHindi => "Hindi & Persian",
            Self::PersianMongolian => "Mongolian & Persian",
            Self::PersianRussian => "Persian & Russian",
            Self::PersianTurkish => "Persian & Turkish",

            Self::PersianArabicGreek => "Arabic & Greek & Persian",
            Self::PersianArabicHindi => "Arabic & Hindi & Persian",
            Self::PersianArabicTurkish => "Arabic & Persian & Turkish",
        }
    }
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

#[must_use]
pub fn get_lang(parsed: &Html) -> Lang {
    let selector = Selector::parse("lang").unwrap();
    let mut lang = Lang::Unmarked;

    if let Some(result) = parsed.select(&selector).next() {
        let text: String = result.text().collect();
        let trimmed = text.trim().trim_end_matches('.');

        lang = match trimmed {
            "A" | "B" => Lang::Arabic, // "B" occurs once, on p. 975; apparently a typo
            "E" => Lang::English,
            "G" => Lang::Greek,
            "HE" => Lang::Hebrew,
            "H" => Lang::Hindi,
            "L" => Lang::Latin,
            "M" => Lang::Mongolian,
            "P" => Lang::Persian,
            "PORT" => Lang::Portuguese,
            "R" => Lang::Russian,
            "S" => Lang::Sanskrit,
            "SP" => Lang::Spanish,
            "SY" => Lang::Syriac,
            "T" => Lang::Turkish,
            "U" => Lang::Urdu,

            "A G" => Lang::ArabicGreek,
            "A T" => Lang::ArabicTurkish,

            // "o" occurs once, on p. 1,271; apparently a typo
            // "ā," a more obvious typo, occurs several times
            "a" | "ā" | "o" | "A a" | "A P" => Lang::PersianArabic,
            "g" => Lang::PersianGreek,
            "h" => Lang::PersianHindi,
            "m" => Lang::PersianMongolian,
            "r" => Lang::PersianRussian,
            "t" => Lang::PersianTurkish,

            "g a" => Lang::PersianArabicGreek,
            "a h" => Lang::PersianArabicHindi,
            "a t" | "t a" | "a p t" => Lang::PersianArabicTurkish,

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
    let persian_text: String = persian.text().collect();
    let persian_cleaned = clean(&persian_text);

    let latin_text = match parsed.select(&selector_i).next() {
        Some(latin) => pandoc(&latin.html())?,
        None => "N/A".to_owned(),
    };

    Ok((persian_cleaned, latin_text))
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

#[allow(clippy::let_and_return)]
fn clean(input: &str) -> String {
    let trimmed = input.trim();
    let no_zwj = trimmed.replace('\u{200D}', "");
    let no_rlm = no_zwj.replace('\u{200F}', "");
    let switch_h = no_rlm.replace('\u{FBA9}', "\u{0647}");

    switch_h
}

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
    use std::mem;

    impl Lang {
        fn from_str(s: &str) -> Self {
            match s {
                "Arabic" => Self::Arabic,
                "English" => Self::English,
                "Greek" => Self::Greek,
                "Hebrew" => Self::Hebrew,
                "Hindi" => Self::Hindi,
                "Latin" => Self::Latin,
                "Mongolian" => Self::Mongolian,
                "Persian" => Self::Persian,
                "Portuguese" => Self::Portuguese,
                "Russian" => Self::Russian,
                "Sanskrit" => Self::Sanskrit,
                "Spanish" => Self::Spanish,
                "Syriac" => Self::Syriac,
                "Turkish" => Self::Turkish,
                "Urdu" => Self::Urdu,

                "Arabic & Greek" => Self::ArabicGreek,
                "Arabic & Turkish" => Self::ArabicTurkish,

                "Arabic & Persian" => Self::PersianArabic,
                "Greek & Persian" => Self::PersianGreek,
                "Hindi & Persian" => Self::PersianHindi,
                "Mongolian & Persian" => Self::PersianMongolian,
                "Persian & Russian" => Self::PersianRussian,
                "Persian & Turkish" => Self::PersianTurkish,

                "Arabic & Greek & Persian" => Self::PersianArabicGreek,
                "Arabic & Hindi & Persian" => Self::PersianArabicHindi,
                "Arabic & Persian & Turkish" => Self::PersianArabicTurkish,

                _ => Self::Unmarked,
            }
        }
    }

    #[test]
    fn lang_values() {
        let conn = Connection::open("entries.sqlite").unwrap();
        let mut stmt = conn
            .prepare("SELECT page, raw_html, lang FROM entries")
            .unwrap();

        let entry_iter = stmt
            .query_map([], |row| {
                let page: u16 = row.get(0).unwrap();
                let raw_html: String = row.get(1).unwrap();
                let lang: String = row.get(2).unwrap();
                Ok((page, raw_html, lang))
            })
            .unwrap();

        for entry in entry_iter {
            let (page, raw_html, lang) = entry.unwrap();

            let lang_from_str = Lang::from_str(&lang);
            assert_eq!(
                lang_from_str.as_str(),
                lang,
                "Lang-to-str back-and-forth failure, p. {}",
                page
            );

            let parsed = Html::parse_fragment(&raw_html);
            let lang_regen = get_lang(&parsed);
            assert_eq!(
                lang_regen, lang_from_str,
                "Lang re-parsing failure, p. {}",
                page
            );
        }
    }

    #[test]
    fn lang_variants() {
        let variants = mem::variant_count::<Lang>();

        let conn = Connection::open("entries.sqlite").unwrap();
        let mut stmt = conn
            .prepare("SELECT COUNT(DISTINCT lang) FROM entries")
            .unwrap();
        let count: usize = stmt.query_row([], |row| row.get(0)).unwrap();

        assert_eq!(count, variants);
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
        let good_pages = total_pages - BAD_PAGES.len() as u16;

        let conn = Connection::open("entries.sqlite").unwrap();
        let mut stmt = conn
            .prepare("SELECT COUNT(DISTINCT page) FROM entries")
            .unwrap();
        let count: u16 = stmt.query_row([], |row| row.get(0)).unwrap();

        assert_eq!(count, good_pages);
    }

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
