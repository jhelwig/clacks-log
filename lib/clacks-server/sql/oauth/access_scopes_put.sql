INSERT INTO clacks.desktop_access_scopes (access_digest,scope) SELECT $1, unnest($2::text[])
