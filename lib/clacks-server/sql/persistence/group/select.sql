SELECT name, identifier_config, bears_location, catalog_binding, ordinary_bindings FROM clacks.group_payload WHERE payload_id = $1 AND account = $2 AND record_id = $3
