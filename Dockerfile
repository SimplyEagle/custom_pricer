# ---------------------------------------------------
# Stage 1: Build Environment (Heavy)
# ---------------------------------------------------
FROM rust:bookworm AS builder

# Set the working directory
WORKDIR /usr/src/custom_pricer

# Copy your source code into the container
COPY . .

# Build the release version of your application
# This takes time, but only happens during the build phase
RUN cargo build --release

# ---------------------------------------------------
# Stage 2: Production Environment (Lightweight)
# ---------------------------------------------------
FROM debian:bookworm-slim

# Install OpenSSL and CA certificates (Required for reqwest to make HTTPS API calls)
RUN apt-get update \
    && apt-get install -y libssl-dev ca-certificates \
    && rm -rf /var/lib/apt/lists/*

# Copy ONLY the compiled binary from the builder stage
COPY --from=builder /usr/src/custom_pricer/target/release/custom_pricer /usr/local/bin/custom_pricer

# Expose the port your Axum server listens on
EXPOSE 8000

# Set the entrypoint to run your application
CMD ["custom_pricer"]