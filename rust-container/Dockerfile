# FROM lukemathwalker/cargo-chef:latest-rust-1 AS chef
# RUN cargo install cargo-chef
FROM lukemathwalker/cargo-chef:latest-rust-1 AS chef

# Install Oracle Instant Client dependencies and the client itself
RUN apt-get update && apt-get install -y libaio1 wget unzip && \
    mkdir -p /opt/oracle && \
    cd /opt/oracle && \
    wget https://download.oracle.com/otn_software/linux/instantclient/1922000/instantclient-basiclite-linux.x64-19.22.0.0.0dbru.zip && \
    unzip instantclient-basiclite-linux.x64-19.22.0.0.0dbru.zip && \
    rm instantclient-basiclite-linux.x64-19.22.0.0.0dbru.zip && \
    sh -c "echo /opt/oracle/instantclient_19_22 > /etc/ld.so.conf.d/oracle-instantclient.conf" && \
    ldconfig

ENV LD_LIBRARY_PATH=/opt/oracle/instantclient_19_22:$LD_LIBRARY_PATH

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

FROM buildcacherelease AS build
COPY . .
RUN cargo test
RUN cargo build --release

# We do not need the Rust toolchain to run the binary!
FROM debian:bookworm-slim AS runtime

RUN apt-get update && apt-get install -y libaio1 && rm -rf /var/lib/apt/lists/*
COPY --from=chef /opt/oracle /opt/oracle
RUN sh -c "echo /opt/oracle/instantclient_19_22 > /etc/ld.so.conf.d/oracle-instantclient.conf" && ldconfig
ENV LD_LIBRARY_PATH=/opt/oracle/instantclient_19_22:$LD_LIBRARY_PATH

COPY --from=build /app/target/release/push-cache /usr/local/bin/

# USER nonroot:nonroot
# COPY entrypoint.sh /
ENTRYPOINT ["/usr/local/bin/push-cache"]
# CMD ["-l", "sample01", "start"]


EXPOSE 8080 8079
