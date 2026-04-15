# Stage 1: Build
FROM rust:1.94-slim AS builder

WORKDIR /app

# Copy manifests first for dependency caching
COPY Cargo.toml Cargo.lock ./
COPY crates/iiif-core/Cargo.toml crates/iiif-core/Cargo.toml
COPY crates/iiif-image/Cargo.toml crates/iiif-image/Cargo.toml
COPY crates/iiif-presentation/Cargo.toml crates/iiif-presentation/Cargo.toml
COPY crates/iiif-auth/Cargo.toml crates/iiif-auth/Cargo.toml
COPY crates/iiif-search/Cargo.toml crates/iiif-search/Cargo.toml
COPY crates/iiif-state/Cargo.toml crates/iiif-state/Cargo.toml
COPY crates/iiif-discovery/Cargo.toml crates/iiif-discovery/Cargo.toml
COPY crates/iiif-server/Cargo.toml crates/iiif-server/Cargo.toml

# Create dummy source files to cache dependency compilation
RUN mkdir -p crates/iiif-core/src && echo "pub fn dummy() {}" > crates/iiif-core/src/lib.rs && \
    mkdir -p crates/iiif-image/src && echo "pub fn dummy() {}" > crates/iiif-image/src/lib.rs && \
    mkdir -p crates/iiif-presentation/src && echo "pub fn dummy() {}" > crates/iiif-presentation/src/lib.rs && \
    mkdir -p crates/iiif-auth/src && echo "pub fn dummy() {}" > crates/iiif-auth/src/lib.rs && \
    mkdir -p crates/iiif-search/src && echo "pub fn dummy() {}" > crates/iiif-search/src/lib.rs && \
    mkdir -p crates/iiif-state/src && echo "pub fn dummy() {}" > crates/iiif-state/src/lib.rs && \
    mkdir -p crates/iiif-discovery/src && echo "pub fn dummy() {}" > crates/iiif-discovery/src/lib.rs && \
    mkdir -p crates/iiif-server/src && echo "fn main() {}" > crates/iiif-server/src/main.rs

RUN cargo build --release 2>/dev/null || true

# Copy real source and rebuild
COPY crates/ crates/
RUN touch crates/*/src/*.rs && cargo build --release

# Stage 2: Runtime
FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates && \
    rm -rf /var/lib/apt/lists/*

WORKDIR /app

COPY --from=builder /app/target/release/iiif-server /app/iiif-server
COPY config.docker.toml /app/config.toml

RUN mkdir -p /app/images

EXPOSE 8080

ENV RUST_LOG=info
ENV IIIF_CONFIG=/app/config.toml

CMD ["/app/iiif-server"]
