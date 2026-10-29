INSERT INTO clacks.desktop_code_scopes (code_digest,scope) SELECT $1, unnest($2::text[])
