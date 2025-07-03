# Builder stage with necessary build tools
FROM --platform=linux/amd64 rust:1.87 as builder

ARG GITHUB_TOKEN
ENV CARGO_REGISTRIES_CRATES_IO_PROTOCOL=sparse
ENV CARGO_BUILD_JOBS=4

# Install build dependencies
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

RUN rustup target add x86_64-unknown-linux-gnu

# Configure git credentials
RUN git config --global url."https://${GITHUB_TOKEN}@github.com/".insteadOf "https://github.com/"

# Create a new empty shell project
WORKDIR /usr/src/myapp

# Copy only the dependency files first
COPY Cargo.toml Cargo.lock ./

# Create a dummy src layout to build dependencies
RUN mkdir -p src && \
  echo 'fn main() { println!("Dummy implementation"); }' > src/server.rs && \
  echo 'pub fn dummy() { println!("Dummy lib"); }' > src/lib.rs && \
  # Build the dependencies
  cargo build --release --target=x86_64-unknown-linux-gnu && \
  # Remove the dummy source files but keep the compiled dependencies
  rm -rf src && \
  # Also remove the built binary as we only want to cache deps
  rm -f target/x86_64-unknown-linux-gnu/release/hibiki*

# Now copy the actual source code
COPY . .

# Build the application (will reuse cached dependencies)
RUN cargo build --release --target=x86_64-unknown-linux-gnu

# Start a new stage with minimal runtime dependencies
FROM --platform=linux/amd64 rust:1.87 as runner

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

# Expose the port
EXPOSE 50062

# Define the entry point
CMD ["hibiki"]
