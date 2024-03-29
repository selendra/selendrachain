#
### Builder stage
#

FROM rust as builder

WORKDIR /usr/src/selendra
COPY selendra/  /usr/src/selendra
RUN apt-get update && \
  DEBIAN_FRONTEND=noninteractive apt-get install -y \
    ca-certificates \
    clang \
    curl \
    cmake \
    libssl1.1 \
    libssl-dev \
    pkg-config

RUN export PATH="$PATH:$HOME/.cargo/bin" && \
    rustup toolchain install nightly && \
    rustup target add wasm32-unknown-unknown --toolchain nightly && \
    rustup default stable


WORKDIR /usr/src/selendra

RUN cargo build --release --bin selendra --features disputes --verbose
RUN cp -v /usr/src/selendra/target/release/selendra /usr/local/bin

# check if executable works in this container
RUN /usr/local/bin/selendra --version

#
### Runtime
#

FROM debian:buster-slim as runtime
RUN apt-get update && \
    apt-get install -y curl tini

COPY --from=builder /usr/src/selendra/target/release/selendra /usr/local/bin
# Non-root user for security purposes.
#
# UIDs below 10,000 are a security risk, as a container breakout could result
# in the container being ran as a more privileged user on the host kernel with
# the same UID.
#
# Static GID/UID is also useful for chown'ing files outside the container where
# such a user does not exist.
RUN groupadd --gid 10001 nonroot && \
    useradd  --home-dir /home/nonroot \
            --create-home \
            --shell /bin/bash \
            --gid nonroot \
            --groups nonroot \
            --uid 10000 nonroot
WORKDIR /home/nonroot/selendra

RUN chown -R nonroot. /home/nonroot

# Use the non-root user to run our application
USER nonroot
# check if executable works in this container
RUN /usr/local/bin/selendra --version
# Tini allows us to avoid several Docker edge cases, see https://github.com/krallin/tini.
ENTRYPOINT ["tini", "--", "/usr/local/bin/selendra"]
