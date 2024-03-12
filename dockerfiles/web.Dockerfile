FROM node:20 as node

WORKDIR /usr/src/web

COPY ./apps/web /usr/src/web
RUN npm i
RUN npm run build

FROM rust:1.76

WORKDIR /usr/src/web
RUN rustup target add wasm32-unknown-unknown
RUN cargo install --locked trunk

COPY --from=node /usr/src/web /usr/src/web
RUN mkdir -p /usr/crates

COPY ./crates /usr/crates

EXPOSE 8080
CMD ["trunk", "serve", "--address", "0.0.0.0"]