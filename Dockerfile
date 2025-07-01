# Builder stage with necessary build tools
FROM --platform=linux/amd64 rust:1.87-slim as builder

ARG GITHUB_TOKEN

# Install build dependencies in a single layer to reduce image size
RUN apt-get update && \
  apt-get install -y --no-install-recommends \
  protobuf-compiler \
  ca-certificates \
  git \
  build-essential \
  cmake \
  pkg-config \
  libssl-dev && \
  rm -rf /var/lib/apt/lists/*

# Set up Rust environment
RUN rustup target add x86_64-unknown-linux-gnu

# Configure git credentials (this is removed in the final image)
RUN git config --global url."https://${GITHUB_TOKEN}@github.com/".insteadOf "https://github.com/"

# Set up working directory
WORKDIR /usr/src/myapp
COPY . .

# Build the project
RUN cargo build --release --target=x86_64-unknown-linux-gnu

# Start a new stage with minimal runtime dependencies
FROM --platform=linux/amd64 debian:bookworm-slim as runner

# Install only the required runtime libraries
RUN apt-get update && \
  apt-get install -y --no-install-recommends \
  libssl-dev \
  ca-certificates && \
  rm -rf /var/lib/apt/lists/* && \
  # Create a non-root user
  groupadd -r appuser && \
  useradd --no-log-init -r -g appuser appuser

# Copy only the compiled binary from the builder stage
COPY --from=builder /usr/src/myapp/target/x86_64-unknown-linux-gnu/release/hibiki /usr/local/bin/hibiki

# Set proper permissions
RUN chmod 755 /usr/local/bin/hibiki

# Switch to non-root user
USER appuser

# Set up networking
EXPOSE 50061

# Define the entry point
CMD ["hibiki"]