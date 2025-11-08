# A Docker image to check the app behavior in a clean and resource-limited enviornment.

FROM rust:1.91

RUN rustup toolchain install nightly
RUN cargo install bat

WORKDIR /app
COPY Cargo.toml Cargo.lock* ./

CMD ["bash"]
