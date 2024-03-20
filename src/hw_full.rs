use crate::{clean_simple, pandoc};
use scraper::{Html, Selector};

pub fn select_full_headword(parsed: &Html) -> Result<String, anyhow::Error> {
    let selector = Selector::parse("hw").unwrap();
    let hw_full = parsed.select(&selector).next().unwrap();
    let hw_full_text = pandoc(&hw_full.html())?;
    let cleaned = clean_hw_full(&hw_full_text);

    Ok(cleaned)
}

#[allow(clippy::let_and_return)]
fn clean_hw_full(input: &str) -> String {
    let mut cleaned = clean_simple(input);

    let swaps_ordered: [(&str, &str); 2] = [
        ("\u{0020}\u{0650}", ""), // Space kasra
        ("\u{0650}", ""),         // Kasra
    ];

    for (from, to) in swaps_ordered {
        cleaned = cleaned.replace(from, to);
    }

    let swaps_simple: [(char, &str); 8] = [
        ('\u{FB7A}', "\u{0686}"),         // Ch
        ('\u{0022}', "\u{02BB}\u{02BB}"), // Double ayn
        ('\u{00E2}', "\u{0101}"),         // A hat
        ('\u{0680}', "\u{067E}"),         // Quad p
        ('\u{1E61}', "\u{1E63}"),         // Dot s
        ('\u{1E33}', "\u{006B}"),         // Dot k
        ('\u{2039}', "\u{012B}"),         // Left arrow
        ('\u{00E0}', "\u{0061}"),         // A grave
    ];

    for (from, to) in swaps_simple {
        cleaned = cleaned.replace(from, to);
    }

    let swaps_complex: [(&str, &str); 5] = [
        ("\u{0627}\u{064E}", "\u{0622}"), // Alif fatha
        ("\u{0065}\u{0306}", "\u{0115}"), // E breve
        ("\u{06CC}\u{064E}", "\u{06CC}"), // Fix maris
        ("\u{0020}\u{064C}", "\u{064B}"), // Fix muwajahatan
        ("\u{0020}\u{064F}", "\u{064B}"), // Fix yasiran
    ];

    for (from, to) in swaps_complex {
        cleaned = cleaned.replace(from, to);
    }

    cleaned
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::charsets::{ARABIC_ALLOWED, OTHER_ALLOWED};
    use rusqlite::Connection;

    #[test]
    fn chars() {
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

            // Entry on ātish-ālūd is messed up; make sure to fix later
            // Entry on najaz also seems off
            // Will allow semicolon for now, but it should probably be removed
            // Will allow ō for now, but it should probably be replaced with o
            // Use of U+02CC is odd, but will allow it for now
            // Apostrophe is used exactly once; might replace later
            // Need to replace ṭ eventually, but it isn't wrong in a consistent way
            // Same problem with î: wrong in two different ways
            // Em dash should be removed eventually, but will allow it for now
            // Use of ĕ is strange but ok; it's really in the printed Steingass

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
    fn values_fast() {
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

            let cleaned_further = clean_hw_full(&headword_full);

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
    fn _values_slow() {
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

            if id % 100 == 0 {
                println!("Checking ID {}", id);
            }

            let parsed = Html::parse_fragment(&raw_html);
            let hw_full_regen = select_full_headword(&parsed).unwrap();

            assert_eq!(hw_full_regen, headword_full);
        }
    }
}
