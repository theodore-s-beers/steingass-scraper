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
    let cleaned = clean_hw_full(&hw_full_text);

    Ok(cleaned)
}

#[must_use]
pub fn get_hw_per(parsed: &Html) -> String {
    let selector_pa = Selector::parse("hw pa").unwrap();
    let persian = parsed.select(&selector_pa).next().unwrap();
    let persian_text: String = persian.text().collect();

    clean_hw_per(&persian_text)
}

pub fn get_hw_lat(parsed: &Html) -> Result<String, anyhow::Error> {
    let selector_i = Selector::parse("hw i").unwrap();
    let latin_text = match parsed.select(&selector_i).next() {
        Some(latin) => pandoc(&latin.html())?,
        None => "N/A".to_owned(),
    };

    Ok(latin_text)
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
fn clean_hw_full(input: &str) -> String {
    let precleaned = clean_simple(input);

    // Complex removals; maintain order!
    let no_space_kasra = precleaned.replace("\u{0020}\u{0650}", "");
    let no_kasra = no_space_kasra.replace('\u{0650}', "");

    // Simple swaps
    let swap_ch = no_kasra.replace('\u{FB7A}', "\u{0686}");
    let swap_double_ayn = swap_ch.replace('\u{0022}', "\u{02BB}\u{02BB}");
    let swap_a_hat = swap_double_ayn.replace('\u{00E2}', "\u{0101}");
    let swap_quad_p = swap_a_hat.replace('\u{0680}', "\u{067E}");
    let swap_dot_s = swap_quad_p.replace('\u{1E61}', "\u{1E63}");
    let swap_dot_k = swap_dot_s.replace('\u{1E33}', "\u{006B}");
    let swap_left_arrow = swap_dot_k.replace('\u{2039}', "\u{012B}");
    let swap_a_grave = swap_left_arrow.replace('\u{00E0}', "\u{0061}");

    // Complex swaps
    let swap_alif_fatha = swap_a_grave.replace("\u{0627}\u{064E}", "\u{0622}");
    let swap_e_breve = swap_alif_fatha.replace("\u{0065}\u{0306}", "\u{0115}");

    // Word-specific fixes
    let fix_maris = swap_e_breve.replace("\u{06CC}\u{064E}", "\u{06CC}");
    let fix_muwajahatan = fix_maris.replace("\u{0020}\u{064C}", "\u{064B}");
    let fix_yasiran = fix_muwajahatan.replace("\u{0020}\u{064F}", "\u{064B}");

    fix_yasiran
}

#[allow(clippy::let_and_return)]
fn clean_hw_per(input: &str) -> String {
    let precleaned = clean_simple(input);

    // Complex removals; maintain order!
    let no_space_kasra = precleaned.replace("\u{0020}\u{0650}", "");
    let no_kasra = no_space_kasra.replace('\u{0650}', "");

    // Complex swaps
    let swap_alif_fatha = no_kasra.replace("\u{0627}\u{064E}", "\u{0622}");

    // Fix space before kasratayn
    let fix_kasratayn = swap_alif_fatha.replace("\u{0020}\u{064D}", "\u{064D}");

    // Word-specific fixes
    let fix_muwajahatan = fix_kasratayn.replace("\u{0020}\u{064C}", "\u{064B}");
    let fix_maris = fix_muwajahatan.replace("\u{06CC}\u{064E}", "\u{06CC}");
    let fix_yasiran = fix_maris.replace("\u{0020}\u{064F}", "\u{064B}");

    fix_yasiran
}

#[allow(clippy::let_and_return, clippy::similar_names)]
fn clean_simple(input: &str) -> String {
    let trimmed = input.trim();

    // Simple removals
    let no_zwj = trimmed.replace('\u{200D}', "");
    let no_rlm = no_zwj.replace('\u{200F}', "");

    // Simple swaps
    let swap_h_med = no_rlm.replace('\u{FBA9}', "\u{0647}");
    let swap_ch = swap_h_med.replace('\u{FB7D}', "\u{0686}");
    let swap_p_init = swap_ch.replace('\u{FB58}', "\u{067E}");
    let swap_p_med = swap_p_init.replace('\u{FB59}', "\u{067E}");
    let swap_zh = swap_p_med.replace('\u{FB8A}', "\u{0698}");
    let swap_g = swap_zh.replace('\u{FB94}', "\u{06AF}");
    let swap_zh_fin = swap_g.replace('\u{FB8B}', "\u{0698}");
    let swap_h_init = swap_zh_fin.replace('\u{FEEB}', "\u{0647}");
    let swap_madda = swap_h_init.replace('\u{FE81}', "\u{0622}");
    let swap_hamza_y = swap_madda.replace('\u{FE8A}', "\u{0626}");
    let swap_ngoeh = swap_hamza_y.replace('\u{06B1}', "\u{06AF}");
    let swap_h_do = swap_ngoeh.replace('\u{06BE}', "\u{0647}");
    let swap_dotless_b = swap_h_do.replace('\u{066E}', "\u{0628}");

    swap_dotless_b
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

    const ARABIC_ALLOWED: [u32; 47] = [
        0x621, 0x622, 0x623, 0x624, 0x625, 0x626, 0x627, 0x628, 0x629, 0x62A, 0x62B, 0x62C, 0x62D,
        0x62E, 0x62F, 0x630, 0x631, 0x632, 0x633, 0x634, 0x635, 0x636, 0x637, 0x638, 0x639, 0x63A,
        0x641, 0x642, 0x643, 0x644, 0x645, 0x646, 0x647, 0x648, 0x649, 0x64A, 0x64B, 0x64D, 0x651,
        0x670, 0x67E, 0x686, 0x698, 0x6A9, 0x6AF, 0x6C0, 0x6CC,
    ];

    const OTHER_ALLOWED: [u32; 79] = [
        0x0020, 0x0026, 0x0027, 0x0028, 0x0029, 0x002A, 0x002B, 0x002C, 0x002D, 0x002E, 0x0030,
        0x0031, 0x0032, 0x0033, 0x0034, 0x0035, 0x0036, 0x0037, 0x0038, 0x0039, 0x003B, 0x003D,
        0x003F, 0x0041, 0x0050, 0x0051, 0x0053, 0x005A, 0x0061, 0x0062, 0x0063, 0x0064, 0x0065,
        0x0066, 0x0067, 0x0068, 0x0069, 0x006A, 0x006B, 0x006C, 0x006D, 0x006E, 0x006F, 0x0070,
        0x0071, 0x0072, 0x0073, 0x0074, 0x0075, 0x0076, 0x0077, 0x0079, 0x007A, 0x00E1, 0x00EE,
        0x00FC, 0x0101, 0x0113, 0x0115, 0x012B, 0x014D, 0x016B, 0x02BB, 0x02BC, 0x02CC, 0x0320,
        0x0324, 0x1E25, 0x1E35, 0x1E43, 0x1E47, 0x1E5B, 0x1E63, 0x1E6D, 0x1E89, 0x1E93, 0x1E95,
        0x1E96, 0x2014,
    ];

    #[test]
    fn char_lists_sorted() {
        let mut arabic = ARABIC_ALLOWED;
        let mut other = OTHER_ALLOWED;

        arabic.sort_unstable();
        other.sort_unstable();

        assert_eq!(arabic, ARABIC_ALLOWED);
        assert_eq!(other, OTHER_ALLOWED);
    }

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

            let mut stmt_entry_backup = conn_backup
                .prepare("SELECT raw_html FROM entries WHERE id = ?")
                .unwrap();

            let html_backup: String = stmt_entry_backup.query_row([id], |row| row.get(0)).unwrap();

            assert_eq!(html_dev, html_backup);
        }
    }

    #[test]
    fn hw_full_chars() {
        let conn = Connection::open("entries.sqlite").unwrap();
        let mut stmt = conn
            .prepare("SELECT id, headword_full FROM entries")
            .unwrap();

        let entry_iter = stmt
            .query_map([], |row| {
                let id: u32 = row.get(0).unwrap();
                let headword_full: String = row.get(1).unwrap();
                Ok((id, headword_full))
            })
            .unwrap();

        for entry in entry_iter {
            let (id, headword_full) = entry.unwrap();

            // The entry on ātish-ālūd is messed up; make sure to fix later
            // The entry on najaz also seems off
            // Will allow semicolon for now, but it should probably be removed
            // Will allow ō for now, but it should probably be replaced with o
            // Use of U+02CC is odd, but will allow it for now
            // Apostrophe is used exactly once; might replace later
            // Need to replace ṭ eventually, but it isn't wrong in a consistent way
            // Same problem with î: wrong in two different ways
            // Em dash should be removed eventually, but will allow it for now
            // The use of ĕ is strange but ok; it's really in the printed Steingass

            for c in headword_full.chars() {
                assert!(
                    ARABIC_ALLOWED.contains(&(c as u32))
                        || OTHER_ALLOWED.contains(&(c as u32))
                        || c as u32 == 0x0674,
                    "Non-standard char in full headword (ID {}): U+{:04X}",
                    id,
                    c as u32
                );
            }
        }
    }

    #[test]
    fn hw_full_values_fast() {
        let conn = Connection::open("entries.sqlite").unwrap();
        let mut stmt = conn
            .prepare("SELECT id, headword_full FROM entries")
            .unwrap();

        let entry_iter = stmt
            .query_map([], |row| {
                let id: u32 = row.get(0).unwrap();
                let headword_full: String = row.get(1).unwrap();
                Ok((id, headword_full))
            })
            .unwrap();

        for entry in entry_iter {
            let (id, headword_full) = entry.unwrap();

            let cleaned_simple = clean_simple(&headword_full);
            let cleaned_further = clean_hw_full(&cleaned_simple);

            // if cleaned_further != headword_full {
            //     println!("Fixing ID {}", id);

            //     conn.execute(
            //         "UPDATE entries SET headword_full = ?1 WHERE id = ?2",
            //         (cleaned_further, id),
            //     )
            //     .unwrap();
            // }

            assert_eq!(cleaned_further, headword_full, "Mismatch in ID {}", id);
        }
    }

    // This takes about 35 minutes to run; it has run successfully
    // #[test]
    fn _hw_full_values_slow() {
        let conn = Connection::open("entries.sqlite").unwrap();
        let mut stmt = conn
            .prepare("SELECT id, raw_html, headword_full FROM entries")
            .unwrap();

        let entry_iter = stmt
            .query_map([], |row| {
                let id: u32 = row.get(0).unwrap();
                let raw_html: String = row.get(1).unwrap();
                let headword_full: String = row.get(2).unwrap();
                Ok((id, raw_html, headword_full))
            })
            .unwrap();

        for entry in entry_iter {
            let (id, raw_html, headword_full) = entry.unwrap();
            println!("Checking ID {}...", id);

            let parsed = Html::parse_fragment(&raw_html);
            let hw_full_regen = select_full_headword(&parsed).unwrap();

            assert_eq!(hw_full_regen, headword_full);
        }
    }

    #[test]
    fn hw_per_chars() {
        let conn = Connection::open("entries.sqlite").unwrap();
        let mut stmt = conn
            .prepare("SELECT id, headword_persian FROM entries")
            .unwrap();

        let entry_iter = stmt
            .query_map([], |row| {
                let id: u32 = row.get(0).unwrap();
                let headword_persian: String = row.get(1).unwrap();
                Ok((id, headword_persian))
            })
            .unwrap();

        for entry in entry_iter {
            let (id, headword_persian) = entry.unwrap();

            // Need to deal with U+0674 later; it affects 300+ entries
            for c in headword_persian.chars() {
                assert!(
                    ARABIC_ALLOWED.contains(&(c as u32))
                        || c as u32 == 0x0674
                        || c as u32 == 0x0020,
                    "Non-standard char in Persian headword (ID {}): U+{:04X}",
                    id,
                    c as u32
                );
            }
        }
    }

    #[test]
    fn hw_per_values() {
        let conn = Connection::open("entries.sqlite").unwrap();
        let mut stmt = conn
            .prepare("SELECT id, raw_html, headword_persian FROM entries")
            .unwrap();

        let entry_iter = stmt
            .query_map([], |row| {
                let id: u32 = row.get(0).unwrap();
                let raw_html: String = row.get(1).unwrap();
                let headword_persian: String = row.get(2).unwrap();
                Ok((id, raw_html, headword_persian))
            })
            .unwrap();

        for entry in entry_iter {
            let (id, raw_html, headword_persian) = entry.unwrap();
            let parsed = Html::parse_fragment(&raw_html);
            let persian_regen = get_hw_per(&parsed);

            if headword_persian == "\u{0639}" {
                continue;
            }

            // if persian_regen != headword_persian {
            //     println!("Fixing ID {}", id);

            //     conn.execute(
            //         "UPDATE entries SET headword_persian = ?1 WHERE id = ?2",
            //         (persian_regen, id),
            //     )
            //     .unwrap();
            // }

            assert_eq!(
                persian_regen, headword_persian,
                "Mismatch (ID {}): {} != {}",
                id, persian_regen, headword_persian
            );
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
}
