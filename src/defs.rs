use crate::{clean_simple, pandoc};
use regex::Regex;

pub fn except_headword(input: &str) -> Result<String, anyhow::Error> {
    let re_c = Regex::new("<c>.*</c>").unwrap();
    let re_hw = Regex::new("<hw>.*</hw>").unwrap();
    let re_lang = Regex::new("<lang>.*</lang>").unwrap();

    let without_c = re_c.replace_all(input, "");
    let without_hw = re_hw.replace_all(&without_c, "");
    let without_lang = re_lang.replace_all(&without_hw, "");

    let processed = pandoc(&without_lang)?;
    let cleaned = clean_defs(&processed);

    Ok(cleaned)
}

#[allow(clippy::let_and_return)]
fn clean_defs(input: &str) -> String {
    let precleaned = clean_simple(input);

    // Simple swaps
    let swap_ae = precleaned.replace('\u{04D4}', "\u{00C6}");
    let swap_quad_p = swap_ae.replace('\u{0680}', "\u{067E}");
    let swap_u_hat = swap_quad_p.replace('\u{00FB}', "\u{016B}");
    let swap_madda = swap_u_hat.replace('\u{FE81}', "\u{0622}");
    let swap_dot = swap_madda.replace('\u{00B7}', "\u{02BB}");
    let swap_lira = swap_dot.replace('\u{20A4}', "\u{00A3}");
    let swap_z_dot = swap_lira.replace('\u{017C}', "\u{1E93}");
    let swap_a_acute = swap_z_dot.replace('\u{00C1}', "\u{0041}");

    // Complex swaps; maintain order!
    let swap_e_breve = swap_a_acute.replace("\u{0065}\u{0306}", "\u{0115}");
    let swap_breve = swap_e_breve.replace('\u{0306}', "\u{02D8}");

    // Single-instance fix
    let fix_lone_madda = swap_breve.replace(
        "\u{002F}\u{061F}\u{002F}",
        "\u{0640}\u{0640}\u{0653}\u{0640}",
    );

    fix_lone_madda
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::charsets::{ARABIC_ALLOWED, DEFS_GREEK, DEFS_HEBREW, DEFS_MISC, OTHER_ALLOWED};
    use rusqlite::Connection;

    #[test]
    fn chars() {
        let conn = Connection::open("entries.sqlite").unwrap();
        let mut stmt = conn.prepare("SELECT id, definitions FROM entries").unwrap();

        let entry_iter = stmt
            .query_map([], |row| {
                let id: u32 = row.get(0).unwrap();
                let definitions: String = row.get(1).unwrap();
                Ok((id, definitions))
            })
            .unwrap();

        for entry in entry_iter {
            let (id, definitions) = entry.unwrap();

            for c in definitions.chars() {
                // U+005C is used to escape other characters; can reflect problems
                // U+00ED is wrong, but not in a consistent way; perhaps fix later

                assert!(
                    ARABIC_ALLOWED.contains(&(c as u32))
                        || OTHER_ALLOWED.contains(&(c as u32))
                        || DEFS_MISC.contains(&(c as u32))
                        || DEFS_GREEK.contains(&(c as u32))
                        || DEFS_HEBREW.contains(&(c as u32))
                        || c as u32 == 0x0674,
                    "Non-standard char in definitions (ID {}): U+{:04X}",
                    id,
                    c as u32
                );
            }
        }
    }

    #[test]
    fn values_fast() {
        let conn = Connection::open("entries.sqlite").unwrap();
        let mut stmt = conn.prepare("SELECT id, definitions FROM entries").unwrap();

        let entry_iter = stmt
            .query_map([], |row| {
                let id: u32 = row.get(0).unwrap();
                let definitions: String = row.get(1).unwrap();
                Ok((id, definitions))
            })
            .unwrap();

        for entry in entry_iter {
            let (id, definitions) = entry.unwrap();

            let cleaned = clean_defs(&definitions);

            // if cleaned != definitions {
            //     println!("Fixing ID {}", id);

            //     conn.execute(
            //         "UPDATE entries SET definitions = ?1 WHERE id = ?2",
            //         (cleaned, id),
            //     )
            //     .unwrap();
            // }

            assert_eq!(cleaned, definitions, "Mismatch in ID {}", id);
        }
    }
}
