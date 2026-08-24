FROM rust:latest

WORKDIR /workspace
COPY . .

RUN rustup component add rustfmt clippy \
    && cargo fmt --all -- --check \
    && cargo clippy --all-targets --all-features -- -D warnings \
    && cargo test --all-targets \
    && cargo run --quiet -- --validate

CMD ["cargo", "run"]
