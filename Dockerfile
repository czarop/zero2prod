# We use the latest Rust stable release as base image
FROM rust:1.80.1
# Let's switch our working directory to `app` (equivalent to `cd app`)
# The `app` folder will be created for us by Docker in case it does not
# exist already.
WORKDIR /app
# Install the required system dependencies for our linking configuration
RUN apt update && apt install lld clang -y
# Copy all files from our working environment to our Docker image
COPY . .
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