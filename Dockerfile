FROM archlinux:latest as builder

RUN pacman -Syu mariadb-clients rustup base-devel --noconfirm
RUN rustup default nightly

WORKDIR /build

COPY Cargo.toml .
COPY src ./src

RUN cargo build --release


FROM archlinux:latest

COPY --from=builder /build/target/release/foobot .

CMD ["./foobot"]
