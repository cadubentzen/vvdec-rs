FROM mcr.microsoft.com/devcontainers/rust:latest

RUN apt-get update && \
    apt-get install -y cmake llvm-dev libclang-dev clang && \
    apt-get clean

RUN git clone --depth 1 -b v2.3.0 https://github.com/fraunhoferhhi/vvdec.git && \
    cd vvdec && \
    make install-release-shared install-vvdecapp=1 install-prefix=/usr && \
    rm -rf vvdec
