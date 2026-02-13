# Build stage
FROM golang:1.24-alpine AS builder

# Install dependencies
RUN apk add --no-cache git openssh-client

WORKDIR /app

# Copy go mod files
COPY go.mod go.sum ./
RUN go mod download

# Copy source code
COPY . .

# Build the applications
RUN CGO_ENABLED=0 GOOS=linux go build -o /agito ./cmd/agito
RUN CGO_ENABLED=0 GOOS=linux go build -o /agito-server ./cmd/agito-server

# Runtime stage
FROM alpine:latest

# Install git and openssh
RUN apk add --no-cache git openssh-server openssh-keygen

# Create necessary directories
RUN mkdir -p /var/lib/agito/repos /var/lib/agito/ssh

# Copy binaries from builder
COPY --from=builder /agito /usr/local/bin/agito
COPY --from=builder /agito-server /usr/local/bin/agito-server

# Copy web assets
COPY web /app/web

WORKDIR /app

# Expose ports
EXPOSE 3000 2222

# Run the server
CMD ["agito-server", "--repos", "/var/lib/agito/repos", "--http-port", "3000", "--ssh-port", "2222"]
