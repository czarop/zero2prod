# this file generates a docker image to run the app in PRODUCTION - not related to the postgres SQLX docker!

# We use the latest Rust stable release as base image
FROM rust:1.80.1 AS builder
# Let's switch our working directory to `app` (equivalent to `cd app`)
# The `app` folder will be created for us by Docker in case it does not
# exist already.
WORKDIR /app
# Install the required system dependencies for our linking configuration
RUN apt update && apt install lld clang -y
# Copy all files from our working environment to our Docker image
COPY . .

# offline build - no access to sqlx database. this requires the queries to be pre-generated in .sqlx folder
# via: cargo sqlx prepare (args...)
ENV SQLX_OFFLINE=true

# Let's build our binary!
# We'll use the release profile to make it faaaast
RUN cargo build --release
# When `docker run` is executed, launch the binary!
ENTRYPOINT ["./target/release/zero2prod"]

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
