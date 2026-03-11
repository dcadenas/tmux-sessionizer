FROM rust:1.93-bookworm

RUN apt-get update && apt-get install -y tmux && rm -rf /var/lib/apt/lists/*

WORKDIR /app

# Cache dependencies by copying manifests first
COPY Cargo.toml Cargo.lock rust-toolchain.toml ./
RUN mkdir src && echo "fn main() {}" > src/main.rs && echo "" > src/lib.rs \
    && cargo build --tests 2>/dev/null || true \
    && rm -rf src

# Copy actual source
COPY . .

# Build tests (touch to invalidate the dummy src)
RUN touch src/main.rs src/lib.rs && cargo test --no-run

CMD ["cargo", "test", "--", "--nocapture"]
