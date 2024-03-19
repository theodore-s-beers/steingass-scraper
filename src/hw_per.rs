use crate::clean_simple;
use scraper::{Html, Selector};

#[must_use]
pub fn get_hw_per(parsed: &Html) -> String {
    let selector_pa = Selector::parse("hw pa").unwrap();
    let persian = parsed.select(&selector_pa).next().unwrap();
    let persian_text: String = persian.text().collect();

    clean_hw_per(&persian_text)
}

#[allow(clippy::let_and_return)]
fn clean_hw_per(input: &str) -> String {
    let precleaned = clean_simple(input);

    // Complex removals; maintain order!
    let no_space_kasra = precleaned.replace("\u{0020}\u{0650}", "");
    let no_kasra = no_space_kasra.replace('\u{0650}', "");

    // Complex swaps
    let swap_alif_fatha = no_kasra.replace("\u{0627}\u{064E}", "\u{0622}");
    let fix_kasratayn = swap_alif_fatha.replace("\u{0020}\u{064D}", "\u{064D}");

    // Word-specific fixes
    let fix_muwajahatan = fix_kasratayn.replace("\u{0020}\u{064C}", "\u{064B}");
    let fix_maris = fix_muwajahatan.replace("\u{06CC}\u{064E}", "\u{06CC}");
    let fix_yasiran = fix_maris.replace("\u{0020}\u{064F}", "\u{064B}");

    fix_yasiran
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::charsets::ARABIC_ALLOWED;
    use rusqlite::Connection;

    #[test]
    fn chars() {
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
    fn values() {
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
}
