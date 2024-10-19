# this file generates a docker image to run the app in PRODUCTION - not related to the postgres SQLX docker!

# cargo-chef allows for incremental updates
FROM lukemathwalker/cargo-chef:latest-rust-1.80.1 AS chef
WORKDIR /app
RUN apt update && apt install lld clang -y
FROM chef AS planner
COPY . .
# Compute a lock-like file for our project
RUN cargo chef prepare --recipe-path recipe.json
FROM chef AS builder
COPY --from=planner /app/recipe.json recipe.json
# Build our project dependencies, not our application!
RUN cargo chef cook --release --recipe-path recipe.json
# Up to this point, if our dependency tree stays the same,
# all layers should be cached.
COPY . .
ENV SQLX_OFFLINE true
# Build our project
RUN cargo build --release --bin zero2prod













# # We use the latest Rust stable release as base image
# FROM rust:1.80.1 AS builder
# # Let's switch our working directory to `app` (equivalent to `cd app`)
# # The `app` folder will be created for us by Docker in case it does not
# # exist already.
# WORKDIR /app
# # Install the required system dependencies for our linking configuration
# RUN apt update && apt install lld clang -y
# # Copy all files from our working environment to our Docker image
# COPY . .

# # offline build - no access to sqlx database. this requires the queries to be pre-generated in .sqlx folder
# # via: cargo sqlx prepare (args...)
# ENV SQLX_OFFLINE=true

# # Let's build our binary!
# # We'll use the release profile to make it faaaast
# RUN cargo build --release
# # When `docker run` is executed, launch the binary!
# ENTRYPOINT ["./target/release/zero2prod"]

# call:
# docker build --tag zero2prod .
# to build the docker image

# Using '.' we are telling Docker to use the current directory as the build context for this image; COPY . app
# will therefore copy all files from the current directory (including our source code!) into the app directory of
# our Docker image.

# Runtime stage
FROM debian:bookworm-slim AS runtime
WORKDIR /app
# Install OpenSSL - it is dynamically linked by some of our dependencies
# Install ca-certificates - it is needed to verify TLS certificates
# when establishing HTTPS connections
RUN apt-get update -y \
    && apt-get install -y --no-install-recommends openssl ca-certificates \
    # Clean up
    && apt-get autoremove -y \
    && apt-get clean -y \
    && rm -rf /var/lib/apt/lists/*
COPY --from=builder /app/target/release/zero2prod zero2prod
COPY configuration configuration
ENV APP_ENVIRONMENT=production
ENTRYPOINT ["./zero2prod"]
