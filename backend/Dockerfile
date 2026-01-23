# FROM lukemathwalker/cargo-chef:latest-rust-1 AS chef
# RUN cargo install cargo-chef
FROM lukemathwalker/cargo-chef:latest-rust-1 AS chef

WORKDIR /app

FROM chef AS planner
COPY . .
RUN cargo chef prepare --recipe-path recipe.json

FROM chef AS buildcache
COPY --from=planner /app/recipe.json recipe.json

FROM buildcache AS dev
# Install dev tools
RUN cargo install cargo-watch
# Grab dependencies into target
RUN cargo chef cook --recipe-path recipe.json
# This enables us to keep /app/target inside the container when we mount /app from host
VOLUME /app/target
# So we hold the daemon mode of docker run
CMD sleep infinity

FROM buildcache AS buildcacherelease
# Build dependencies - this is the caching Docker layer!
RUN cargo chef cook --release --recipe-path recipe.json
# Build application

FROM ghcr.io/polecatworks/hams:main AS sofile
# This is a dummy stage to get the shared object file

FROM buildcacherelease AS build
COPY --from=sofile /usr/local/lib/libhams.so /usr/local/lib/
RUN ldconfig
ENV RUSTFLAGS="-L /usr/local/lib/"
COPY . .
RUN cargo build --release

# We do not need the Rust toolchain to run the binary!
FROM debian:trixie-slim AS runtime
# https://github.com/GoogleContainerTools/distroless
# FROM gcr.io/distroless/cc-debian12 as runtime
WORKDIR /app
COPY --from=sofile /usr/local/lib/libhams.so /usr/local/lib/
RUN ldconfig
COPY --from=build /app/target/release/push-cache /usr/local/bin/

# COPY entrypoint.sh /
ENTRYPOINT ["/usr/local/bin/push-cache"]
# CMD ["-l", "sample01", "start"]


EXPOSE 8080 8079
