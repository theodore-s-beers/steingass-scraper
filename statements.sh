#! /usr/bin/env bash

set -Eeuo pipefail

npx wrangler d1 execute steingass-v2 --remote --command \
	"CREATE VIRTUAL TABLE ft_all USING fts5(headword_full, definitions, content='entries', content_rowid='id')" &&
	npx wrangler d1 execute steingass-v2 --remote --command \
		"INSERT INTO ft_all (headword_full, definitions) SELECT headword_full, definitions FROM entries" &&
	npx wrangler d1 execute steingass-v2 --remote --command \
		"CREATE VIRTUAL TABLE ft_def USING fts5(definitions, content='entries', content_rowid='id')" &&
	npx wrangler d1 execute steingass-v2 --remote --command \
		"INSERT INTO ft_def (definitions) SELECT definitions FROM entries" &&
	npx wrangler d1 execute steingass-v2 --remote --command \
		"CREATE VIRTUAL TABLE ft_hw USING fts5(headword_full, content='entries', content_rowid='id')" &&
	npx wrangler d1 execute steingass-v2 --remote --command \
		"INSERT INTO ft_hw (headword_full) SELECT headword_full FROM entries" &&
	npx wrangler d1 execute steingass-v2 --remote --command \
		"CREATE VIRTUAL TABLE ft_per USING fts5(headword_persian, content='entries', content_rowid='id')" &&
	npx wrangler d1 execute steingass-v2 --remote --command \
		"INSERT INTO ft_per (headword_persian) SELECT headword_persian FROM entries" &&
	npx wrangler d1 execute steingass-v2 --remote --command \
		"CREATE INDEX idx_page ON entries(page)" &&
	npx wrangler d1 execute steingass-v2 --remote --command \
		"CREATE INDEX idx_per ON entries(headword_persian)" &&
	npx wrangler d1 execute steingass-v2 --remote --command \
		"CREATE INDEX idx_abjad ON entries(abjad)"
