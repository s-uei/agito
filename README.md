# Agito - Self-Hosted Git Platform

Agito is a lightweight, self-hosted Git repository server with a simple web viewer, SSH authentication, and CI/CD capabilities through server-side hooks. It provides an alternative to services like GitLab or Gitea for teams that want complete control over their Git infrastructure.

## Features

- 🚀 **Git Alternative Command**: CLI tool that wraps git and adds server management capabilities
- 🌐 **Simple Web Viewer**: Clean web interface to browse repositories, commits, and files
- 🔐 **SSH Authentication**: Secure git operations using SSH key-based authentication
- 📦 **Remote Repository Creation**: Create bare repositories on the server via SSH
- 🔄 **CI/CD Support**: Server-side git hooks for automated pipelines
- 🐳 **Docker Compose**: Easy deployment with Docker containers

## Quick Start

### Using Docker Compose (Recommended)

1. Clone the repository:
```bash
git clone https://github.com/s-uei/agito.git
cd agito
```

2. Start the server:
```bash
docker-compose up -d
```

3. Access the web interface at `http://localhost:3000`

### Manual Installation

#### Prerequisites
- Rust 1.75 or later
- Git
- OpenSSH

#### Build from source

```bash
# Clone the repository
git clone https://github.com/s-uei/agito.git
cd agito

# Build the binaries
cargo build --release

# Install the binaries
sudo cp target/release/agito /usr/local/bin/
sudo cp target/release/agito-server /usr/local/bin/
```

#### Start the server

```bash
# Create directories for repositories and SSH keys
sudo mkdir -p /var/lib/agito/repos /var/lib/agito/ssh

# Start the server
sudo agito-server \
  --repos /var/lib/agito/repos \
  --http-port 3000 \
  --ssh-port 2222 \
  --ssh-key /var/lib/agito/ssh/host_key \
  --authorized-keys /var/lib/agito/ssh/authorized_keys
```

## Usage

### Client Commands

The `agito` CLI provides git operations plus server management:

```bash
# Create a repository on the server
export AGITO_SERVER=localhost:2222
export AGITO_USER=git
agito create myrepo

# Clone a repository
agito clone ssh://git@localhost:2222/myrepo.git

# Use like normal git
cd myrepo
agito status
agito add .
agito commit -m "Initial commit"
agito push
```

### Setting up SSH Authentication

1. Generate an SSH key (if you don't have one):
```bash
ssh-keygen -t rsa -b 4096 -C "your_email@example.com"
```

2. Add your public key to the server's authorized_keys:
```bash
# Copy your public key
cat ~/.ssh/id_rsa.pub

# Add it to the server
sudo bash -c 'cat >> /var/lib/agito/ssh/authorized_keys' << EOF
<paste your public key here>
EOF
```

3. Test the connection:
```bash
ssh git@localhost -p 2222
```

### Web Interface

Access the web interface at `http://localhost:3000` to:
- Browse all repositories
- View repository files and commits
- Read README files
- Navigate through branches

## CI/CD with Server-Side Hooks

Agito includes server-side git hooks for automated workflows:

### Pre-Receive Hook
Validates pushes before accepting them. Located at `<repo>/hooks/pre-receive`.

### Post-Receive Hook
Triggers after a successful push. Located at `<repo>/hooks/post-receive`.

Example: Create a CI/CD pipeline script:

```bash
# In your repository, create agito-ci.sh
cat > agito-ci.sh << 'EOF'
#!/bin/bash
# CI/CD Pipeline

echo "Running CI/CD Pipeline..."

# Run tests
echo "Running tests..."
# npm test || exit 1

# Build
echo "Building..."
# npm run build || exit 1

# Deploy (if on main branch)
if [ "$1" = "main" ]; then
    echo "Deploying to production..."
    # ./deploy.sh
fi

echo "Pipeline completed successfully"
EOF

chmod +x agito-ci.sh
git add agito-ci.sh
git commit -m "Add CI/CD pipeline"
git push
```

The post-receive hook will automatically execute this script after each push.

### Update Hook
Validates individual ref updates. Located at `<repo>/hooks/update`.

## Configuration

### Server Configuration

Environment variables:
- `AGITO_REPOS_DIR`: Directory for repositories (default: `/var/lib/agito/repos`)
- `AGITO_HTTP_PORT`: HTTP port (default: `3000`)
- `AGITO_SSH_PORT`: SSH port (default: `2222`)

Command-line flags:
```bash
agito-server --help
```

### Client Configuration

Environment variables:
- `AGITO_SERVER`: Server address (default: `localhost:2222`)
- `AGITO_USER`: SSH user (default: `git`)

## Architecture

```
┌─────────────────┐
│   Agito CLI     │  Git wrapper + server commands
└────────┬────────┘
         │
         │ SSH (port 2222)
         │
┌────────▼────────┐
│  Agito Server   │
├─────────────────┤
│  SSH Server     │  Handle git operations
│  Web Server     │  Repository viewer (port 3000)
│  Git Hooks      │  CI/CD automation
└────────┬────────┘
         │
         │
    ┌────▼─────┐
    │   Bare   │
    │  Repos   │
    └──────────┘
```

## Docker Deployment

The included `docker-compose.yml` provides a complete setup:

- **agito-server**: Main server container
  - Port 3000: Web interface
  - Port 2222: SSH for git operations
  - Volumes for persistent data

- **agito-runner**: CI/CD runner (optional)
  - Executes pipeline scripts
  - Has Docker access for containerized builds

### Customization

Edit `docker-compose.yml` to customize:
- Port mappings
- Volume locations
- Environment variables

## Security Considerations

1. **SSH Keys**: Use strong SSH keys (RSA 4096-bit or Ed25519)
2. **Authorized Keys**: Regularly review `/var/lib/agito/ssh/authorized_keys`
3. **Firewall**: Restrict SSH port (2222) access to trusted networks
4. **HTTPS**: Use a reverse proxy (nginx/traefik) for HTTPS on the web interface
5. **Hooks**: Review git hooks for security before allowing execution

## Development

### Project Structure

```
agito/
├── cmd/
│   ├── agito/          # CLI client
│   └── agito-server/   # Server application
├── internal/
│   ├── git/           # Git operations
│   ├── server/        # Web server
│   └── ssh/           # SSH server
├── web/
│   ├── templates/     # HTML templates
│   └── static/        # Static assets
├── scripts/
│   └── runner.sh      # CI/CD runner
├── Dockerfile
├── Dockerfile.runner
└── docker-compose.yml
```

### Building

```bash
# Build CLI
cargo build --release --bin agito

# Build server
cargo build --release --bin agito-server

# Run tests
cargo test
```

## Contributing

Contributions are welcome! Please feel free to submit a Pull Request.

## License

MIT License - see LICENSE file for details.

## Acknowledgments

Built with:
- Rust programming language
- russh for SSH server
- axum for HTTP server
- tokio for async runtime
- Git for version control