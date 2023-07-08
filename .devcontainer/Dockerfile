FROM mcr.microsoft.com/devcontainers/rust:0-1-bullseye

RUN apt-get update && \
    apt-get install -y \
    cmake llvm-dev libclang-dev clang \
    libgstreamer1.0-dev gstreamer1.0-plugins-base \
    gstreamer1.0-plugins-good gstreamer1.0-plugins-bad \
    gstreamer1.0-libav && \
    apt-get clean

RUN git clone --depth 1 -b v2.0.0 https://github.com/fraunhoferhhi/vvdec.git && \
    cd vvdec && \
    make install-release-shared install-vvdecapp=1 install-prefix=/usr && \
    rm -rf vvdec
