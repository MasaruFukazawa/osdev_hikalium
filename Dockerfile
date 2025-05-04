FROM ubuntu:22.04

# タイムゾーンと非対話型で進めるための環境変数
ENV DEBIAN_FRONTEND=noninteractive
ENV TZ=Asia/Tokyo

# 必要なツールのインストール
RUN apt-get update && apt-get install -y \
    curl \
    build-essential \
    pkg-config \
    netcat-openbsd \
    qemu-system-x86 \
    qemu-utils \
    libssl-dev \
    gcc-x86-64-linux-gnu \
    && apt-get clean \
    && rm -rf /var/lib/apt/lists/*

# Rust を rustup でインストール
RUN curl https://sh.rustup.rs -sSf | sh -s -- -y

# PATHを通す
ENV PATH="/root/.cargo/bin:${PATH}"

# 作業ディレクトリ作成
WORKDIR /wasabi

# デフォルトコマンド
CMD [ "bash" ]
