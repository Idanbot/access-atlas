FROM rust:1.88-bookworm AS build

WORKDIR /src
COPY . .
RUN cargo build --locked --release

FROM debian:bookworm-slim

ENV TERM=xterm-256color \
    LANG=C.UTF-8

COPY --from=build /src/target/release/access-atlas /usr/local/bin/access-atlas

ENTRYPOINT ["access-atlas"]
CMD ["--demo-only"]
