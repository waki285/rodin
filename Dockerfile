# syntax=docker/dockerfile:1.7

########################################
# Builder stage
########################################
FROM rust:slim AS builder

# Install build tooling and pandoc (optional, enables markdown export)
RUN apt-get update && \
    apt-get install -y --no-install-recommends \
        build-essential pkg-config libssl-dev ca-certificates curl git pandoc \
        nodejs npm && \
    rm -rf /var/lib/apt/lists/*

# Enable pnpm (matches repo lockfile)
RUN corepack enable && corepack prepare pnpm@10.18.3 --activate

WORKDIR /app

# Install JS deps first (better layer caching)
COPY package.json pnpm-lock.yaml pnpm-workspace.yaml ./
RUN pnpm install --frozen-lockfile

# Copy the rest of the sources
COPY . .

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
