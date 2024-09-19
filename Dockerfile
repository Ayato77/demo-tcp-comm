# Stage 1: Build the Rust application
FROM rust:latest as builder

# Set the working directory
WORKDIR /usr/src/app

# Copy the Cargo.toml and Cargo.lock files
COPY Cargo.toml Cargo.lock ./

RUN mkdir src

# Copy the source code into the container
COPY src ./src

# Pre-fetch dependencies (this step is cached unless Cargo.toml or Cargo.lock changes)
RUN cargo fetch

# Build the project in release mode
RUN cargo build --release

# Stage 2: Create a lightweight image for runtime
FROM debian:bookworm-slim

# Set the application directory
ARG APP=/usr/src/app
RUN mkdir -p ${APP}

# Copy the compiled binary from the build stage
COPY --from=builder /usr/src/app/target/release/tcp-comm ${APP}/tcp-comm

# Expose the port used by the application
EXPOSE 8080

# Set the working directory
WORKDIR ${APP}

# Set the default command to run the compiled Rust binary
CMD ["./tcp-comm"]
