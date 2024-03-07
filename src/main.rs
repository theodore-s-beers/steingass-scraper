#![warn(clippy::pedantic, clippy::nursery)]
#![allow(clippy::uninlined_format_args)]

use std::thread::sleep;
use std::time::Duration;

use rusqlite::Connection;
use scraper::Html;

use steingass_scraper::{
    count_page_entries, ensure_table, except_headword, fetch_html, get_lang, headword_parts,
    insert_row, select_full_headword, select_results, Entry, BAD_PAGES, MAX_PAGE, MIN_PAGE,
};

fn main() -> Result<(), anyhow::Error> {
    println!("Ensuring DB connection...");
    let conn = Connection::open("entries.sqlite")?;
    ensure_table(&conn)?;

    let start_page = MIN_PAGE;
    let stop_page = MAX_PAGE;

    for page in start_page..=stop_page {
        println!("----------------");

        if BAD_PAGES.contains(&page) {
            println!("Skipping p. {}...", page);
            continue;
        }

        if page > start_page {
            println!("Pausing for 3 seconds...");
            sleep(Duration::from_secs(3));
        }

        println!("Fetching p. {}...", page);
        let page_html = fetch_html(page)?;

        let results = select_results(&page_html);
        let results_count = results.len();
        println!("Found {} entries on p. {}", results_count, page);

        let db_count = count_page_entries(&conn, page)?;
        println!("Rows for p. {} in DB: {}", page, db_count);

        // Scraping has been completed; this is to confirm that DB entries match fetched results
        assert_eq!(db_count, results_count);

        // if db_count == results_count {
        //     println!("No further entries for p. {}", page);
        //     continue;
        // }

        // assert!(db_count == 0, "Partial coverage in DB for p. {}", page);

        for (i, result) in results.iter().enumerate() {
            let html = result.html();

            // The entry for "abjad" on p. 5 has an img tag, which can cause problems
            // Everything else is fine, and I've manually checked the "abjad" entry
            if html.contains(".jpg") {
                println!(
                    "p. {}, entry {}/{}: Skipping problematic entry",
                    page,
                    i + 1,
                    results_count
                );
                continue;
            }

            let mut stmt = conn.prepare(
                "SELECT COUNT(*)
                FROM entries
                WHERE raw_html = ?",
            )?;
            let count: u32 = stmt.query_row([&html], |row| row.get(0))?;
            if count == 1 {
                println!(
                    "p. {}, entry {}/{}: Exactly one exact match in DB",
                    page,
                    i + 1,
                    results_count,
                );
                continue;
            }

            assert_eq!(count, 1, "{}", html); // Remaining code in this loop is unreachable

            let parsed = Html::parse_fragment(&html);

            let lang = get_lang(&parsed);
            let headword_full = select_full_headword(&parsed)?;
            let (headword_persian, headword_latin) = headword_parts(&parsed)?;
            let definitions = except_headword(&html)?;

            let entry = Entry {
                page,
                raw_html: html,
                lang,
                headword_full,
                headword_persian,
                headword_latin,
                definitions,
            };

            insert_row(&conn, entry)?;
            println!("Inserted entry {}/{} for p. {}", i + 1, results_count, page);
        }

        // let db_count_new = count_page_entries(&conn, page)?;
        // println!("Updated row count for p. {} in DB: {}", page, db_count_new);
        // assert_eq!(db_count_new, results_count);
    }

    println!("----------------");
    println!("Done");

    Ok(())
}
