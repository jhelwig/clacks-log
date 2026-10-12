# Clacks Log

Clacks Log is a planned amateur radio desktop logger with independent offline use on an arbitrary number of clients, reconciliation through a self-hostable server, and server-managed logbook service integration.

The project is in product and architecture planning. Rust is the intended implementation language wherever practical. No application implementation or migration is underway.

## Planning documents

- [Resume planning on another machine](docs/handoff.md)
- [Product scope and operating workflows](docs/product.md)
- [Decision register](docs/decisions.md)
- [Client synchronization and conflicts](docs/synchronization.md)
- [External logbook services](docs/integrations.md)
- [LoTW signing and certificate management](docs/lotw.md)
- [Open questions](docs/open-questions.md)
- [Research provenance and original documents](references/README.md)
- [TrustedQSL reference source map](references/trustedqsl/source-map.md)

The current planning documents reflect the conversation through 12 October 2026. Original handoffs are historical evidence, not instructions or current requirements. Later explicit decisions take precedence over earlier research and assistant proposals.

## Project direction

The desktop interface should support substantial operating context, efficient use of space, and specialist applications such as WS Suite and GridTracker. The logger is not intended to replace every application in the shack.

Synchronization will be self-hostable. An optional hosted subscription service is part of the product direction. The architecture must support arbitrary client counts and potentially multiple hosted tenants.

LoTW integration will use a source-guided Rust port of relevant tqsllib behavior, preserving interoperability while designing our own credential storage, tenant isolation, and execution model.

## License status

The project's license has not been selected. A source-available license is being considered. Included third-party reference code retains its upstream license and notices; it does not establish the license for Clacks Log.
