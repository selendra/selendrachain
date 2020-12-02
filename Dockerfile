# Inspired by Polkadot Dockerfile

FROM phusion/baseimage:0.11 as builder
LABEL maintainer "saing@selendra.org"
LABEL description="This is the build stage for Selendra."

ARG PROFILE=release
WORKDIR /home/indracore

COPY . /home/indracore

# Download Selendra repo
RUN apt-get update && \
	apt-get upgrade -y && \
	apt-get install -y cmake pkg-config libssl-dev git clang

# Download rust dependencies and build the rust binary
RUN curl https://sh.rustup.rs -sSf | sh -s -- -y && \
	export PATH=$PATH:$HOME/.cargo/bin && \
	scripts/init.sh && \
	cargo build --$PROFILE

USER indracore

# 30333 for p2p traffic
# 9933 for RPC call
# 9944 for Websocket
EXPOSE 30333 9933 9944