FROM rust:buster as builder

WORKDIR /selendra-collator
COPY . /selendra-collator

RUN rustup default nightly-2021-11-07 && \
	rustup target add wasm32-unknown-unknown --toolchain nightly-2021-11-07

RUN apt-get update && \
	apt-get dist-upgrade -y -o Dpkg::Options::="--force-confold" && \
	apt-get install -y cmake pkg-config libssl-dev git clang libclang-dev

ARG GIT_COMMIT=
ENV GIT_COMMIT=$GIT_COMMIT
ARG BUILD_ARGS

RUN cargo build --release $BUILD_ARGS

FROM phusion/baseimage:bionic-1.0.0

LABEL description="Docker image for Selendra-collator Chain" \
	io.parity.image.type="builder" \
	io.parity.image.authors="nath@selendra.org" \
	io.parity.image.vendor="Selendra-collator" \
	io.parity.image.description="Selendra-collator: selendra-collator chain" \
	io.parity.image.source="https://github.com/selendra/selendra-chain/blob/${VCS_REF}/scripts/docker/indra.Dockerfile" \
	io.parity.image.documentation="https://github.com/selendra/selendra-chain"

COPY --from=builder /selendra-collator/target/release/selendra-collator /usr/local/bin

RUN useradd -m -u 1000 -U -s /bin/sh -d /selendra-collator selendra-collator && \
	mkdir -p /data /selendra-collator/.local/share && \
	chown -R selendra-collator:selendra-collator /data && \
	ln -s /data /selendra-collator/.local/share/selendra-collator && \
# unclutter and minimize the attack surface
	rm -rf /usr/bin /usr/sbin && \
# check if executable works in this container
	/usr/local/bin/selendra-collator --version

USER selendra-collator

EXPOSE 30333 9933 9944 9615
VOLUME ["/data"]

ENTRYPOINT ["/usr/local/bin/selendra-collator"]
