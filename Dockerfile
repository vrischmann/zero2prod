FROM rust:1.64

WORKDIR /app

RUN apt-get update && apt-get install -y lld clang
RUN cargo install cargo-deb

COPY Cargo.toml Cargo.toml
COPY Cargo.lock Cargo.lock
COPY sqlx-data.json sqlx-data.json
COPY templates templates
COPY src src

ENV RUSTFLAGS="-C target-cpu=ivybridge"

RUN cargo deb --no-strip
RUN ls -lh /app/target
