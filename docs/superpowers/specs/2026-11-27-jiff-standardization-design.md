# Jiff standardization

## Approach

Jiff supplies application calendar dates, clock times, instants and signed
elapsed durations. Its `jiff-sqlx` integration supplies PostgreSQL encoding and
decoding. Standard-library and Tokio timing facilities continue to supply
monotonic clocks, waits and external API types.

Retaining `time` throughout the server would leave a second general-purpose
application time library. Building a custom SQLx codec would duplicate an
available integration. Neither is needed: `jiff-sqlx` 1.0 supports SQLx 0.9,
matching the project's selected SQLx line.

## Representations and integration boundaries

- `UtcSecond` and `UtcTimestamp` retain their native ranges and serialization.
  Calendar conversion remains fallible for values outside the calendar range.
- Application instants use `jiff::Timestamp`; dates and clock times use Jiff
  civil types. UTC calendar calculations explicitly select UTC.
- SQLx adapter types encode and decode existing PostgreSQL timestamp columns.
  Explicit projection overrides and offline metadata retain compile-time query
  checking.
- Fallible conversions map `oauth-as` standard-library `SystemTime` values to
  Jiff timestamps with subsecond precision.
- `axum_session` configuration takes `time::Duration`. Application-owned login
  lifetimes use Jiff signed durations.
- X.509 validity values are converted through the parser's epoch accessors to
  UTC civil date-times. This represents the complete certificate calendar,
  including the undefined-expiry value `9999-12-31T23:59:59Z`.
- `CoreIdTokenClaims::new` requires Chrono issuance and expiration values.
  The synthetic provider converts `SystemTime` through `Into`, eliminating
  direct Chrono dependencies. The desktop compiles the same provider through
  a shared test source include. Release tooling calculates UTC months in Jiff.
- qso-io 0.4.0 uses Jiff. The remaining transitive Chrono paths are through
  `openidconnect` 4.1.0 and `oauth2` 5.0.0.

## Sources

- [Jiff SQLx integration](https://docs.rs/jiff-sqlx/latest/jiff_sqlx/) and its
  [dependency manifest](https://docs.rs/crate/jiff-sqlx/1.0.0/source/Cargo.toml).
- Selected `openidconnect` 4.1.0 `IdTokenClaims::new` and `axum_session`
  `SessionConfig` signatures establish the external API exceptions.
