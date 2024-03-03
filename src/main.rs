#![warn(clippy::pedantic, clippy::nursery)]
#![allow(clippy::uninlined_format_args)]

use std::thread::sleep;
use std::time::Duration;

use rusqlite::Connection;
use scraper::Html;

use steingass_scraper::{
    count_page_entries, ensure_table, except_headword, fetch_html, get_lang, headword_parts,
    insert_row, select_full_headword, select_results, Entry, BAD_PAGES,
};

fn main() -> Result<(), anyhow::Error> {
    println!("Ensuring DB connection...");
    let conn = Connection::open("entries.sqlite")?;
    ensure_table(&conn)?;

    let start_page = 1;
    let stop_page = 100;

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

        if db_count == results_count {
            println!("No further entries for p. {}", page);
            continue;
        }

        assert!(db_count == 0, "Partial coverage in DB for p. {}", page);

        for (i, result) in results.iter().enumerate() {
            let html = result.html();
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

        let db_count_new = count_page_entries(&conn, page)?;
        println!("Updated row count for p. {} in DB: {}", page, db_count_new);
        assert_eq!(db_count_new, results_count);
    }

    println!("----------------");
    println!("Done");

    Ok(())
}
