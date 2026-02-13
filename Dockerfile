# Build stage
FROM rust:1.75-alpine AS builder

# Install dependencies
RUN apk add --no-cache git openssh-client musl-dev

WORKDIR /app

# Copy Cargo files
COPY Cargo.toml Cargo.lock ./

# Copy source code
COPY src ./src

# Build the applications
RUN cargo build --release --bin agito
RUN cargo build --release --bin agito-server

# Runtime stage
FROM alpine:latest

# Install git and openssh
RUN apk add --no-cache git openssh-server openssh-keygen

# Create necessary directories
RUN mkdir -p /var/lib/agito/repos /var/lib/agito/ssh

# Copy binaries from builder
COPY --from=builder /app/target/release/agito /usr/local/bin/agito
COPY --from=builder /app/target/release/agito-server /usr/local/bin/agito-server

# Copy web assets
COPY web /app/web

WORKDIR /app

# Expose ports
EXPOSE 3000 2222

# Run the server
CMD ["agito-server", "--repos", "/var/lib/agito/repos", "--http-port", "3000", "--ssh-port", "2222"]
