# to be refactor into smaller file size
FROM --platform=linux/amd64 rust:1.87

ARG GITHUB_TOKEN

# Install only essential build dependencies
RUN apt-get update && \
    apt-get install -y --no-install-recommends \
    protobuf-compiler \
    git \
    pkg-config \
    libssl-dev && \
    rm -rf /var/lib/apt/lists/*
    
# Configure git credentials for private repos
RUN git config --global url."https://${GITHUB_TOKEN}@github.com/".insteadOf "https://github.com/"

COPY ./ ./

RUN cargo build --release

CMD ["./target/release/hibiki"]

