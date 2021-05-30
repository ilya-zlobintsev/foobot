FROM rust:alpine as builder

RUN apk add ca-certificates rustup mysql-client gcc musl-dev openssl-dev
RUN rustup default nightly

WORKDIR /build

COPY Cargo.toml .
COPY src ./src

RUN cargo build --release
RUN strip target/release/foobot

FROM alpine:latest

WORKDIR /app

COPY --from=builder /build/target/release/foobot .
COPY templates ./templates

CMD ["/app/foobot"]
