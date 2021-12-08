FROM rust:buster as builder
WORKDIR /app

RUN rustup default nightly-2021-11-07 && \
	rustup target add wasm32-unknown-unknown --toolchain nightly-2021-11-07

RUN apt-get update && \
	apt-get dist-upgrade -y -o Dpkg::Options::="--force-confold" && \
	apt-get install -y cmake pkg-config libssl-dev git clang libclang-dev

ARG GIT_COMMIT=
ENV GIT_COMMIT=$GIT_COMMIT
ARG BUILD_ARGS

COPY . .

RUN cargo build --release $BUILD_ARGS

# =============

FROM phusion/baseimage:bionic-1.0.0
LABEL maintainer="info@selendra.org"

# RUN mv /usr/share/ca* /tmp && \
# 	rm -rf /usr/share/*  && \
# 	mv /tmp/ca-certificates /usr/share/ && \
# 	useradd -m -u 1000 -U -s /bin/sh -d /selendra selendra

RUN useradd -m -u 1000 -U -s /bin/sh -d /selendra selendra

COPY --from=builder /app/target/release/selendra /usr/local/bin

# checks
RUN ldd /usr/local/bin/selendra && \
	/usr/local/bin/selendra --version

# Shrinking
RUN rm -rf /usr/lib/python* && \
	rm -rf /usr/bin /usr/sbin /usr/share/man

USER selendra
EXPOSE 30333 9933 9944

RUN mkdir /selendra/data

VOLUME ["/selendra/data"]

ENTRYPOINT ["/usr/local/bin/selendra"]