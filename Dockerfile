# Use Alpine for both builder and runner - native musl, no cross-compile issues
FROM rust:1.87-alpine AS builder

ARG GITHUB_TOKEN

# Install build dependencies
RUN apk add --no-cache \
  musl-dev \
  openssl-dev \
  openssl-libs-static \
  protobuf-dev \
  protoc \
  pkgconfig \
  git \
  ca-certificates

# Configure git for private repo access
RUN git config --global url."https://${GITHUB_TOKEN}@github.com/".insteadOf "https://github.com/"
ENV CARGO_NET_GIT_FETCH_WITH_CLI=true
ENV OPENSSL_STATIC=1

WORKDIR /usr/src/myapp

# Cache dependencies: copy only manifests first
COPY Cargo.toml Cargo.lock* ./

# Create dummy src to build dependencies
RUN mkdir -p src && echo "fn main() {}" > src/server.rs

# Build dependencies only (cached unless Cargo.toml changes)
RUN cargo build --release || true
RUN rm -rf src

# Copy actual source and rebuild
COPY . .
RUN touch src/server.rs && cargo build --release

# Minimal Alpine runner (~5MB base)
FROM alpine:3.21 AS runner

RUN apk add --no-cache ca-certificates libgcc

# Copy only the binary
COPY --from=builder /usr/src/myapp/target/release/hibiki /usr/local/bin/hibiki

EXPOSE 50062

CMD ["hibiki"]
