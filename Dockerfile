# syntax=docker/dockerfile:1.7

########################################
# Base stage with toolchain, sccache, pnpm
########################################
FROM rustlang/rust:nightly-slim AS builder-base

RUN apt-get update && \
    apt-get install -y --no-install-recommends \
        build-essential pkg-config libssl-dev ca-certificates curl git pandoc \
        woff2 libharfbuzz-dev nodejs npm clang libclang-dev mold && \
    rm -rf /var/lib/apt/lists/*

# sccache
RUN curl -L https://github.com/mozilla/sccache/releases/latest/download/sccache-v0.12.0-x86_64-unknown-linux-musl.tar.gz \
    | tar xz && mv sccache-v0.12.0-x86_64-unknown-linux-musl/sccache /usr/local/bin/ && rm -rf sccache-v0.12.0-x86_64-unknown-linux-musl

# pnpm (lockfile matches repo)
RUN corepack enable && corepack prepare pnpm@10.24.0 --activate

WORKDIR /app

ENV RUSTC_WRAPPER=/usr/local/bin/sccache \
    SCCACHE_DIR=/sccache \
    RUSTFLAGS="-Clink-arg=-fuse-ld=mold -Zthreads=8"

# cargo-chef for dependency caching (prebuilt musl binary)
RUN curl -L https://github.com/LukeMathWalker/cargo-chef/releases/download/v0.1.73/cargo-chef-x86_64-unknown-linux-musl.tar.gz \
    | tar -xz -C /usr/local/bin cargo-chef

########################################
# Planner: analyze dependencies
########################################
FROM builder-base AS planner
COPY Cargo.toml Cargo.lock build.rs ./ 
COPY src src
COPY build build
COPY content content
COPY static static
RUN cargo chef prepare --recipe-path recipe.json

########################################
# Cook: build dependency layer
########################################
FROM builder-base AS cook
COPY --from=planner /app/recipe.json /app/recipe.json
RUN --mount=type=cache,target=/sccache,sharing=locked cargo chef cook --release --recipe-path recipe.json

########################################
# Builder: app build
########################################
FROM builder-base AS builder
COPY --from=cook /app/target /app/target

# JS deps first for cache friendliness
COPY package.json pnpm-lock.yaml pnpm-workspace.yaml ./ 
RUN pnpm install --frozen-lockfile

# Copy full source
COPY . .

# Build (build.rs runs Esbuild/Typst/pandoc as needed)
RUN --mount=type=cache,target=/sccache,sharing=locked cargo build --release

########################################
# Runtime stage
########################################
FROM rustlang/rust:nightly-slim AS runtime

RUN apt-get update && \
    apt-get install -y --no-install-recommends ca-certificates git && \
    rm -rf /var/lib/apt/lists/*

WORKDIR /app

COPY --from=builder /app/target/release/rodin /app/rodin
COPY --from=builder /app/target/release/rodin-content /app/rodin-content
COPY --from=builder /app/static /app/static

RUN git clone --depth=1 -b main https://github.com/waki285/rodin-content.git content

ENV RODIN_MARKDOWN_ENABLED=true \
    RUST_LOG=info \
    PORT=3000

EXPOSE 3000

CMD ["/app/rodin"]
