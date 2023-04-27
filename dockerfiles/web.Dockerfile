FROM rust:1.68

WORKDIR /usr/src/web
RUN rustup target add wasm32-unknown-unknown
RUN cargo install --locked trunk

COPY ./apps/web /usr/src/web
RUN mkdir -p /usr/crates

COPY ./crates /usr/crates

EXPOSE 8080
CMD ["trunk", "serve", "--address", "0.0.0.0"]