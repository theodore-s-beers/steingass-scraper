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
    let precleaned = clean_simple(input);

    // Simple swaps
    let swap_double_ayn = precleaned.replace('\u{0022}', "\u{02BB}\u{02BB}");
    let swap_a_hat = swap_double_ayn.replace('\u{00E2}', "\u{0101}");
    let swap_dot_s = swap_a_hat.replace('\u{1E61}', "\u{1E63}");
    let swap_dot_k = swap_dot_s.replace('\u{1E33}', "\u{006B}");
    let swap_left_arrow = swap_dot_k.replace('\u{2039}', "\u{012B}");
    let swap_a_grave = swap_left_arrow.replace('\u{00E0}', "\u{0061}");

    // Complex swap
    let swap_e_breve = swap_a_grave.replace("\u{0065}\u{0306}", "\u{0115}");

    swap_e_breve
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
}
