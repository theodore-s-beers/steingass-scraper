#![warn(clippy::pedantic, clippy::nursery)]
#![allow(clippy::uninlined_format_args)]

use core::str;
use std::io::Write;
use std::process::Command;

use regex::Regex;
use reqwest::blocking::get;
use scraper::{ElementRef, Html, Selector};
use tempfile::NamedTempFile;

const PREFIX: &str = "https://dsal.uchicago.edu/cgi-bin/app/steingass_query.py?page=";

fn main() -> Result<(), anyhow::Error> {
    let page = 290; // Arbitrary example
    let page_html = fetch_html(page)?;
    let results = select_results(&page_html);

    println!("Found {} entries on page {}", results.len(), page);

    for entry in results {
        let html = entry.html();
        let parsed = Html::parse_fragment(&html);

        // Full headword
        let hw_full = headword_full(&parsed);
        let hw_full_text = pandoc(&hw_full.html())?;

        // Persian- and Latin-script parts of headword
        let (hw_per, hw_lat) = headword_parts(&parsed);
        let hw_per_text = pandoc(&hw_per.html())?;
        let hw_lat_text = pandoc(&hw_lat.html())?;

        // Definition(s)
        let def = except_headword(&html);
        let def_text = pandoc(&def)?;

        println!("----------------");
        println!("Headword (full): {}", hw_full_text);
        println!("Headword (Persian): {}", hw_per_text);
        println!("Headword (Latin): {}", hw_lat_text);
        println!("Definition(s): {}", def_text);
    }

    Ok(())
}

fn fetch_html(page: u32) -> Result<Html, anyhow::Error> {
    let url = format!("{}{}", PREFIX, page);
    let response_text = get(url)?.text()?;
    let parsed = Html::parse_document(&response_text);
    Ok(parsed)
}

fn select_results(parsed: &Html) -> Vec<ElementRef> {
    let selector = Selector::parse("#results_display .container div").unwrap();
    parsed.select(&selector).collect()
}

fn headword_full(parsed: &Html) -> ElementRef {
    let selector = Selector::parse("hw").unwrap();
    parsed.select(&selector).next().unwrap()
}

fn headword_parts(parsed: &Html) -> (ElementRef, ElementRef) {
    let selector_pa = Selector::parse("hw pa").unwrap();
    let selector_i = Selector::parse("hw i").unwrap();

    let persian = parsed.select(&selector_pa).next().unwrap();
    let latin = parsed.select(&selector_i).next().unwrap();

    (persian, latin)
}

fn except_headword(input: &str) -> String {
    let re_c = Regex::new("<c>.*</c>").unwrap();
    let re_hw = Regex::new("<hw>.*</hw>").unwrap();
    let re_lang = Regex::new("<lang>.*</lang>").unwrap();

    let without_c = re_c.replace_all(input, "");
    let without_hw = re_hw.replace_all(&without_c, "");
    let without_lang = re_lang.replace_all(&without_hw, "");

    without_lang.to_string()
}

fn pandoc(input: &str) -> Result<String, anyhow::Error> {
    let mut tempfile = NamedTempFile::new()?;
    write!(tempfile, "{}", input)?;

    let pandoc = Command::new("pandoc")
        .arg(tempfile.path())
        .arg("-t")
        .arg("plain")
        .arg("--wrap=none")
        .output()?;

    let output = str::from_utf8(&pandoc.stdout)?;
    let cleaned = output.trim().trim_start_matches(',').trim_start();

    Ok(cleaned.to_owned())
}
