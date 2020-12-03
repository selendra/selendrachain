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

# FROM phusion/baseimage:0.11
# LABEL maintainer "saing@selendra.org"
# LABEL description="This is the build stage for Selendra."
# ARG PROFILE=release
COPY --from=builder /home/indracore/target/$PROFILE/indracore /usr/local/bin

RUN mv /usr/share/ca* /tmp && \
	rm -rf /usr/share/*  && \
	mv /tmp/ca-certificates /usr/share/ && \
	rm -rf /usr/lib/python* && \
	useradd -m -u 1000 -U -s /bin/sh -d /indracore indracore && \
	mkdir -p /indracore/.local/share/indracore && \
	chown -R indracore:indracore /indracore/.local && \
	ln -s /indracore/.local/share/indracore /data && \
	rm -rf /usr/bin /usr/sbin

USER indracore


# 30333 for p2p traffic
# 9933 for RPC call
# 9944 for Websocket
EXPOSE 30333 9933 9944

# VOLUME ["/data"]

# CMD ["/usr/local/bin/indracore --rpc-cors '"*"' --pruning archive"]