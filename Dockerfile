# syntax=docker/dockerfile:1.7

########################################
# Builder stage
########################################
FROM rust:slim AS builder

# Install build tooling, sccache, and pandoc (optional, enables markdown export)
RUN apt-get update && \
    apt-get install -y --no-install-recommends \
        build-essential pkg-config libssl-dev ca-certificates curl git pandoc \
        nodejs npm && \
    rm -rf /var/lib/apt/lists/*

# Install sccache
RUN curl -L https://github.com/mozilla/sccache/releases/latest/download/sccache-v0.12.0-x86_64-unknown-linux-musl.tar.gz \
    | tar xz && mv sccache-v0.12.0-x86_64-unknown-linux-musl/sccache /usr/local/bin/ && rm -rf sccache-v0.12.0-x86_64-unknown-linux-musl

# Enable pnpm (matches repo lockfile)
RUN corepack enable && corepack prepare pnpm@10.23.0 --activate

WORKDIR /app

# Install JS deps first (better layer caching)
COPY package.json pnpm-lock.yaml pnpm-workspace.yaml ./
RUN pnpm install --frozen-lockfile

# Copy the rest of the sources
COPY . .

# Use sccache for Rust compilation
ENV RUSTC_WRAPPER=/usr/local/bin/sccache \
    SCCACHE_DIR=/sccache

# Warm cache to speed up subsequent builds
RUN cargo fetch

# Build release binary (build.rs will run tailwind/pandoc steps)
RUN cargo build --release

########################################
# Runtime stage
########################################
FROM debian:bookworm-slim AS runtime

RUN apt-get update && \
    apt-get install -y --no-install-recommends ca-certificates && \
    rm -rf /var/lib/apt/lists/*

WORKDIR /app

# Copy binary and static assets
COPY --from=builder /app/target/release/rodin /app/rodin
COPY --from=builder /app/static /app/static

ENV RODIN_MARKDOWN_ENABLED=true \
    RUST_LOG=info \
    PORT=3000

EXPOSE 3000

CMD ["/app/rodin"]
