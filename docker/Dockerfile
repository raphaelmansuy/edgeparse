# ─── Stage 1: Build ───────────────────────────────────────────────────────────
FROM rust:1.85-slim-bookworm AS builder

# Install only what the linker and PDF library need
RUN apt-get update && apt-get install -y --no-install-recommends \
    pkg-config \
    libssl-dev \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /build

# Cache cargo registry — copy manifests first
COPY Cargo.toml Cargo.lock ./
COPY crates/edgeparse-core/Cargo.toml   crates/edgeparse-core/
COPY crates/edgeparse-cli/Cargo.toml    crates/edgeparse-cli/
COPY crates/pdf-cos/Cargo.toml          crates/pdf-cos/
COPY crates/edgeparse-python/Cargo.toml crates/edgeparse-python/
COPY crates/edgeparse-node/Cargo.toml   crates/edgeparse-node/
COPY crates/edgeparse-wasm/Cargo.toml   crates/edgeparse-wasm/

# Build a dummy binary to pre-warm dependency compilation cache
RUN mkdir -p crates/edgeparse-core/src crates/edgeparse-cli/src crates/pdf-cos/src \
    crates/edgeparse-python/src crates/edgeparse-node/src crates/edgeparse-wasm/src \
    && echo "fn main() {}" > crates/edgeparse-cli/src/main.rs \
    && echo "" > crates/edgeparse-core/src/lib.rs \
    && echo "" > crates/pdf-cos/src/lib.rs \
    && echo "" > crates/edgeparse-python/src/lib.rs \
    && echo "" > crates/edgeparse-node/src/lib.rs \
    && echo "" > crates/edgeparse-wasm/src/lib.rs \
    && cargo build --release -p edgeparse-cli \
    && rm -rf crates/*/src

# Now copy real source and build
COPY crates/ crates/
RUN touch crates/edgeparse-core/src/lib.rs crates/edgeparse-cli/src/main.rs \
    && cargo build --release -p edgeparse-cli

# ─── Stage 2: Runtime ─────────────────────────────────────────────────────────
FROM gcr.io/distroless/cc-debian12:nonroot AS runtime

# distroless/cc ships glibc, libstdc++, and CA certs — just enough for our binary
COPY --from=builder /build/target/release/edgeparse /usr/local/bin/edgeparse

# Run as non-root by default (distroless nonroot = UID 65532)
USER nonroot

ENTRYPOINT ["/usr/local/bin/edgeparse"]
CMD ["--help"]
