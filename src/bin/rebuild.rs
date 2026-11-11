use std::path::Path;

fn main() -> anyhow::Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.as_slice() {
        [flag] if flag == "--help" || flag == "-h" => {
            println!(
                "Usage: rebuild OUTPUT_DIR\n       rebuild compare ACTUAL.sqlite EXPECTED.sqlite\n\nRun from the repository root; Pandoc 3.13 is required.\nReads html_backup.sqlite read-only; writes entries.sqlite, entries_slim.sqlite,\nand audit.txt in a new OUTPUT_DIR. Both databases are vacuumed before completion.\nIdentical inputs and toolchain produce identical SQLite bytes; build with --locked.\nNever fetches pages or replaces source files.\ncompare checks every stored field and exits unsuccessfully on differences.\nSee src/rebuild.rs for the workflow and src/corrections.rs for editorial changes."
            );
            Ok(())
        }
        [command, actual, expected] if command == "compare" => {
            steingass_scraper::rebuild::compare(Path::new(actual), Path::new(expected))
        }
        [destination] if !destination.starts_with('-') => steingass_scraper::rebuild::rebuild(
            Path::new("html_backup.sqlite"),
            Path::new(destination),
        ),
        _ => anyhow::bail!(
            "Usage: rebuild OUTPUT_DIR\n       rebuild compare ACTUAL.sqlite EXPECTED.sqlite\nOUTPUT_DIR must not exist. Run from the repository root. No network access."
        ),
    }
}
