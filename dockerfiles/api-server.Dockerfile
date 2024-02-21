FROM rust:1.76 AS builder

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

FROM debian:stable-slim
WORKDIR /app
RUN apt update \
    && apt install -y openssl ca-certificates \
    && apt clean \
    && rm -rf /var/lib/apt/lists/* /tmp/* /var/tmp/*

COPY --from=builder /usr/src/target/release/spyglass ./

EXPOSE 4664
CMD ["./spyglass", "--api-only", "--read-only", "--addr", "0.0.0.0"]