INSERT INTO clacks.desktop_access_resources (access_digest,ordinal,resource) SELECT $1, (ordinality-1)::int, value FROM unnest($2::text[]) WITH ORDINALITY AS r(value,ordinality)
