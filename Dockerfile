# to be refactor into smaller file size
FROM --platform=linux/amd64 rust:1.87 as builder

ARG GITHUB_TOKEN

RUN rustup target add x86_64-unknown-linux-gnu
RUN apt-get update && apt-get install -y protobuf-compiler
RUN git config --global url."https://${GITHUB_TOKEN}@github.com/".insteadOf "https://github.com/"

# Create a new empty shell project
WORKDIR /usr/src/myapp
COPY . .

# Build the project in release mode
RUN cargo build --release --target=x86_64-unknown-linux-gnu

# Start a new stage
FROM --platform=linux/amd64 rust:1.87 as runner
RUN apt-get update && apt-get install -y libssl-dev && rm -rf /var/lib/apt/lists/*

# Copy the binary from the builder stage
COPY --from=builder /usr/src/myapp/target/x86_64-unknown-linux-gnu/release/hibiki /usr/local/bin/hibiki

EXPOSE 50062

CMD ["hibiki"]
