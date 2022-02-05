FROM rust:buster as builder

WORKDIR /selendra
COPY . /selendra

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

LABEL description="Docker image for Selendra Chain" \
	io.parity.image.type="builder" \
	io.parity.image.authors="nath.selendra@gmail.com" \
	io.parity.image.vendor="Selendra" \
	io.parity.image.description="Selendra: selendra chain" \
	io.parity.image.source="https://github.com/selendra/selendra-chain/blob/${VCS_REF}/scripts/docker/Dockerfile" \
	io.parity.image.documentation="https://github.com/selendra/selendra-chain"

COPY --from=builder /selendra/target/release/selendra /usr/local/bin

RUN useradd -m -u 1000 -U -s /bin/sh -d /selendra selendra && \
	mkdir -p /data /selendra/.local/share && \
	chown -R selendra:selendra /data && \
	ln -s /data /selendra/.local/share/selendra && \
# unclutter and minimize the attack surface
	rm -rf /usr/bin /usr/sbin && \
# check if executable works in this container
	/usr/local/bin/selendra --version

USER selendra

EXPOSE 30333 9933 9944 9615
VOLUME ["/data"]

ENTRYPOINT ["/usr/local/bin/selendra"]
