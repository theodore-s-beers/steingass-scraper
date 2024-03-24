use crate::{clean_simple, pandoc};
use scraper::{Html, Selector};

pub fn get_hw_lat(parsed: &Html) -> Result<String, anyhow::Error> {
    let selector_i = Selector::parse("hw i").unwrap();
    let latin_text = match parsed.select(&selector_i).next() {
        Some(latin) => pandoc(&latin.html())?,
        None => "N/A".to_owned(),
    };
    let cleaned = clean_hw_lat(&latin_text);

    Ok(cleaned)
}

#[allow(clippy::let_and_return)]
fn clean_hw_lat(input: &str) -> String {
    let mut cleaned = clean_simple(input);

    let swaps: [(char, &str); 6] = [
        ('\u{0022}', "\u{2018}\u{2018}"), // Double ayn
        ('\u{00E0}', "\u{0061}"),         // A grave
        ('\u{00E2}', "\u{0101}"),         // A hat
        ('\u{1E33}', "\u{006B}"),         // Dot k
        ('\u{1E61}', "\u{1E63}"),         // Dot s
        ('\u{2039}', "\u{012B}"),         // Left arrow
    ];

    for (from, to) in swaps {
        cleaned = cleaned.replace(from, to);
    }

    cleaned
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::charsets::OTHER_ALLOWED;
    use rusqlite::Connection;

    #[test]
    fn chars() {
        let conn = Connection::open("entries.sqlite").unwrap();
        let mut stmt = conn
            .prepare("SELECT id, headword_latin FROM entries")
            .unwrap();

        let entry_iter = stmt
            .query_map([], |row| {
                let id: u32 = row.get(0).unwrap();
                let headword_latin: String = row.get(1).unwrap();
                Ok((id, headword_latin))
            })
            .unwrap();

        for entry in entry_iter {
            let (id, headword_latin) = entry.unwrap();

            for c in headword_latin.chars() {
                assert!(
                    OTHER_ALLOWED.contains(&(c as u32)),
                    "Non-standard char in Latin headword (ID {}): U+{:04X}",
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
            .prepare("SELECT id, headword_latin FROM entries")
            .unwrap();

        let entry_iter = stmt
            .query_map([], |row| {
                let id: u32 = row.get(0).unwrap();
                let headword_latin: String = row.get(1).unwrap();
                Ok((id, headword_latin))
            })
            .unwrap();

        for entry in entry_iter {
            let (id, headword_latin) = entry.unwrap();
            let cleaned = clean_hw_lat(&headword_latin);

            // if cleaned != headword_latin {
            //     println!("Fixing ID {}", id);

            //     conn.execute(
            //         "UPDATE entries SET headword_latin = ?1 WHERE id = ?2",
            //         (cleaned, id),
            //     )
            //     .unwrap();
            // }

            assert_eq!(cleaned, headword_latin, "Mismatch in ID {}", id);
        }
    }

    // This will take a while to run; it has not yet run completely
    // #[test]
    fn _values_slow() {
        let conn = Connection::open("entries.sqlite").unwrap();
        let mut stmt = conn
            .prepare("SELECT id, raw_html, headword_latin FROM entries")
            .unwrap();

        let entry_iter = stmt
            .query_map([], |row| {
                let id: u32 = row.get(0).unwrap();
                let raw_html: String = row.get(1).unwrap();
                let headword_latin: String = row.get(2).unwrap();
                Ok((id, raw_html, headword_latin))
            })
            .unwrap();

        for entry in entry_iter {
            let (id, raw_html, headword_latin) = entry.unwrap();

            if id % 100 == 0 {
                println!("Checking ID {}", id);
            }

            let parsed = Html::parse_fragment(&raw_html);
            let hw_latin_regen = get_hw_lat(&parsed).unwrap();

            assert_eq!(hw_latin_regen, headword_latin);
        }
    }
}
