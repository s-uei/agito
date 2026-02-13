# Agito Configuration Examples

## Server Configuration

### Using Command Line Flags

```bash
agito-server \
  --repos /var/lib/agito/repos \
  --http-port 3000 \
  --ssh-port 2222 \
  --ssh-key /var/lib/agito/ssh/host_key \
  --authorized-keys /var/lib/agito/ssh/authorized_keys
```

### Using Environment Variables

```bash
export AGITO_REPOS_DIR=/var/lib/agito/repos
export AGITO_HTTP_PORT=3000
export AGITO_SSH_PORT=2222

agito-server
```

## Docker Compose Configuration

See the main `docker-compose.yml` file in the repository root.

### Custom Docker Compose

```yaml
version: '3.8'

services:
  agito:
    image: agito:latest
    ports:
      - "80:3000"      # HTTP
      - "2222:2222"    # SSH
    volumes:
      - /data/agito/repos:/var/lib/agito/repos
      - /data/agito/ssh:/var/lib/agito/ssh
    environment:
      - AGITO_REPOS_DIR=/var/lib/agito/repos
    restart: unless-stopped
```

## Client Configuration

### Environment Variables

```bash
# Set your Agito server
export AGITO_SERVER=git.example.com:2222
export AGITO_USER=git

# Use agito commands
agito create myrepo
agito clone ssh://git@git.example.com:2222/myrepo.git
```

### SSH Config

Add to `~/.ssh/config`:

```
Host agito
    HostName git.example.com
    Port 2222
    User git
    IdentityFile ~/.ssh/id_rsa
```

Then use:

```bash
git clone ssh://agito/myrepo.git
```

## Security Configuration

### SSH Key Setup

1. Generate SSH key:
```bash
ssh-keygen -t ed25519 -C "your_email@example.com"
```

2. Add public key to server:
```bash
cat ~/.ssh/id_ed25519.pub | ssh root@server 'cat >> /var/lib/agito/ssh/authorized_keys'
```

### Firewall Configuration

```bash
# Allow HTTP
sudo ufw allow 3000/tcp

# Allow SSH (for git)
sudo ufw allow 2222/tcp
```

## Reverse Proxy with Nginx

```nginx
server {
    listen 80;
    server_name git.example.com;

    location / {
        proxy_pass http://localhost:3000;
        proxy_set_header Host $host;
        proxy_set_header X-Real-IP $remote_addr;
        proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
        proxy_set_header X-Forwarded-Proto $scheme;
    }
}

# SSH passthrough (port 2222)
stream {
    upstream ssh {
        server localhost:2222;
    }
    
    server {
        listen 22;
        proxy_pass ssh;
    }
}
```

## HTTPS with Let's Encrypt

```bash
# Install certbot
sudo apt install certbot python3-certbot-nginx

# Get certificate
sudo certbot --nginx -d git.example.com

# Nginx will be configured automatically
```

## Systemd Service

Create `/etc/systemd/system/agito.service`:

```ini
[Unit]
Description=Agito Git Server
After=network.target

[Service]
Type=simple
User=git
Group=git
WorkingDirectory=/opt/agito
ExecStart=/usr/local/bin/agito-server \
    --repos /var/lib/agito/repos \
    --http-port 3000 \
    --ssh-port 2222 \
    --ssh-key /var/lib/agito/ssh/host_key \
    --authorized-keys /var/lib/agito/ssh/authorized_keys
Restart=always
RestartSec=5

[Install]
WantedBy=multi-user.target
```

Enable and start:

```bash
sudo systemctl enable agito
sudo systemctl start agito
sudo systemctl status agito
```

## Production Recommendations

1. **Use a dedicated user**: Create a `git` user for running the server
2. **Set proper permissions**: Restrict access to repos and SSH keys
3. **Enable firewall**: Only allow necessary ports
4. **Use HTTPS**: Set up SSL/TLS for the web interface
5. **Regular backups**: Back up the `/var/lib/agito/repos` directory
6. **Monitor logs**: Set up log rotation and monitoring
7. **Update regularly**: Keep Agito and dependencies updated
