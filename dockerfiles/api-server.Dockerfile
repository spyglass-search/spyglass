FROM rust:1.68 AS builder

WORKDIR /usr/src
# Need for a successful whisper-rs build for some reason...
RUN rustup component add rustfmt
# cmake/clang required for llama-rs/whisper-rs builds
RUN apt update -y && apt upgrade -y
RUN apt install build-essential -y \
    cmake \
    clang

COPY . .
RUN cargo build -p spyglass --bin spyglass --release

FROM scratch
COPY --from=builder /usr/src/target/release/spyglass .

CMD ["spyglass", "--api-only", "--read-only"]