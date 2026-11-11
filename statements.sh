#! /usr/bin/env bash

set -euo pipefail

# Run after importing entries_slim.sqlite into a fresh D1 database.
# Creates the same search schema as steingass-v2. Existing indexes are an error.
# FTS tables are snapshots: rebuild their indexes if entries are edited later.
if [[ $# -ne 1 || ! "$1" =~ ^v[1-9][0-9]*$ ]]; then
	echo "Usage: $0 vN" >&2
	echo "Example (after importing entries): $0 v3" >&2
	exit 1
fi

database="steingass-$1"

execute() {
	pnpx wrangler d1 execute "$database" --remote --command "$1"
}

# Explicit rowids keep FTS results aligned with entries.id, even if IDs have gaps.
execute "CREATE VIRTUAL TABLE ft_all USING fts5(headword_full, definitions, content='entries', content_rowid='id')"
execute "INSERT INTO ft_all (rowid, headword_full, definitions) SELECT id, headword_full, definitions FROM entries ORDER BY id"

execute "CREATE VIRTUAL TABLE ft_def USING fts5(definitions, content='entries', content_rowid='id')"
execute "INSERT INTO ft_def (rowid, definitions) SELECT id, definitions FROM entries ORDER BY id"

execute "CREATE VIRTUAL TABLE ft_hw USING fts5(headword_full, content='entries', content_rowid='id')"
execute "INSERT INTO ft_hw (rowid, headword_full) SELECT id, headword_full FROM entries ORDER BY id"

execute "CREATE VIRTUAL TABLE ft_per USING fts5(headword_persian, content='entries', content_rowid='id')"
execute "INSERT INTO ft_per (rowid, headword_persian) SELECT id, headword_persian FROM entries ORDER BY id"

execute "CREATE INDEX idx_page ON entries(page)"
execute "CREATE INDEX idx_per ON entries(headword_persian)"
execute "CREATE INDEX idx_abjad ON entries(abjad)"
