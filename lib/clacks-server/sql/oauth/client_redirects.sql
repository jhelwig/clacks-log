SELECT redirect_uri AS value FROM clacks.desktop_client_redirects WHERE client_id = $1 ORDER BY redirect_uri
