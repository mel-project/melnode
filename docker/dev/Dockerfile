FROM debian:stable-slim

RUN apt update
RUN apt -y install curl wget nmap

COPY themelio-node /usr/local/bin/themelio-node
COPY run.sh /usr/local/bin/run.sh

WORKDIR /tmp
ENV BATS_VERSION="1.7.0"
RUN wget "https://github.com/bats-core/bats-core/archive/refs/tags/v${BATS_VERSION}.tar.gz"
RUN tar -xf "v${BATS_VERSION}.tar.gz"

WORKDIR "bats-core-${BATS_VERSION}"
RUN ./install.sh /usr/local

WORKDIR /tmp
RUN rm -rf "v${BATS_VERSION}.tar.gz"
RUN rm -rf "bats-core-${BATS_VERSION}"

COPY ci.bats /tmp/ci.bats

WORKDIR /

ENTRYPOINT ["run.sh"]