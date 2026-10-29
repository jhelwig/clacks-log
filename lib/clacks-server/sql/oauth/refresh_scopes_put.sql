INSERT INTO clacks.desktop_refresh_scopes (refresh_digest,scope) SELECT $1, unnest($2::text[])
