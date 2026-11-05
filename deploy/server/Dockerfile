# syntax=docker/dockerfile:1
# rust-toolchain.toml selects the compiler, independently of the builder base.
FROM rust@sha256:e9da6f6c3ed5095fd5d33bf7cdf309830a178a8147888f1b2f76d5769bd9306c AS build
WORKDIR /build
COPY rust-toolchain.toml ./
RUN rustup show active-toolchain && rustup target add wasm32-unknown-unknown
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    cargo install cargo-leptos --version 0.3.11 --locked
COPY . .
ENV SQLX_OFFLINE=true
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/usr/local/cargo/git \
    --mount=type=cache,target=/build/target \
    cargo leptos build --release && \
    cargo build --release --package clacks-admin --locked && \
    mkdir /release && \
    cp target/release/clacks-server target/release/clacks-admin /release/ && \
    cp -r target/site /release/site

FROM debian@sha256:99c0b25c46d0214d5a2091092e7908907dcaecd6a962cde0ac700acee222b6f3
LABEL org.opencontainers.image.vendor="Technosorcery, LLC"
RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates \
    && rm -rf /var/lib/apt/lists/*
COPY --from=build /release/clacks-server /release/clacks-admin /usr/local/bin/
COPY --from=build /release/site /app/site
COPY deploy/server/database-roles.sql deploy/server/database-grants.sql /usr/share/clacks/database/
COPY LICENSE NOTICE /usr/share/licenses/clacks-log/
WORKDIR /app
ENV LEPTOS_SITE_ROOT=/app/site LEPTOS_OUTPUT_NAME=clacks-web LEPTOS_ENV=PROD
USER 65532:65532
EXPOSE 3000
STOPSIGNAL SIGTERM
CMD ["clacks-server", "--config", "/etc/clacks/server.toml"]
