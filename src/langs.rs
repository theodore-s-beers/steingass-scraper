use scraper::{Html, Selector};
use std::str::FromStr;

//
// Type definitions
//

// Keep the enum and its test-only variant list in sync from one declaration.
macro_rules! enum_with_variants {
    (
        $(#[$enum_attr:meta])*
        $vis:vis enum $name:ident {
            $($(#[$variant_attr:meta])* $variant:ident),* $(,)?
        }
    ) => {
        $(#[$enum_attr])*
        $vis enum $name {
            $($(#[$variant_attr])* $variant),*
        }

        #[cfg(test)]
        impl $name {
            const VARIANTS: &'static [Self] = &[$(Self::$variant),*];
        }
    };
}

enum_with_variants! {
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
}

#[derive(Debug)]
pub struct LangParseError;

impl Lang {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
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

impl FromStr for Lang {
    type Err = LangParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let parsed = match s {
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
        };

        Ok(parsed)
    }
}

//
// Functions
//

#[must_use]
pub fn get_lang(parsed: &Html) -> Lang {
    let selector = Selector::parse("lang").unwrap();

    #[allow(clippy::useless_let_if_seq)]
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

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::Connection;
    use std::collections::BTreeSet;

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

            let lang_from_str = Lang::from_str(&lang).unwrap();
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
    fn variants() {
        let expected: BTreeSet<&str> = Lang::VARIANTS.iter().map(|lang| lang.as_str()).collect();

        let conn = Connection::open("entries.sqlite").unwrap();
        let mut stmt = conn.prepare("SELECT DISTINCT lang FROM entries").unwrap();
        let labels: BTreeSet<String> = stmt
            .query_map([], |row| row.get(0))
            .unwrap()
            .collect::<Result<_, _>>()
            .unwrap();
        let actual: BTreeSet<&str> = labels.iter().map(String::as_str).collect();

        assert_eq!(actual, expected);
    }
}
