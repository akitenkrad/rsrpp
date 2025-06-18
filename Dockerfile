FROM ubuntu:24.04

# Install Rust and Cargo
RUN apt update -y && \
    apt install -y curl build-essential libssl-dev poppler-utils libopencv-dev clang libclang-dev && \
    apt clean && \
    rm -rf /var/lib/apt/lists/*

# Install Rust using rustup
ENV RUST_HOME /usr/local/lib/rust
ENV RUSTUP_HOME ${RUST_HOME}/rustup
ENV CARGO_HOME ${RUST_HOME}/cargo
RUN mkdir /usr/local/lib/rust && chmod 0755 $RUST_HOME
RUN curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs > ${RUST_HOME}/rustup.sh \
    && chmod +x ${RUST_HOME}/rustup.sh \
    && ${RUST_HOME}/rustup.sh -y --default-toolchain nightly --no-modify-path
ENV PATH $PATH:$CARGO_HOME/bin

# Install dependencies
RUN cargo install cargo-make && \
    cargo install cargo-nextest