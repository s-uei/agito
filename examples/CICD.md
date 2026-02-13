# Example CI/CD Pipeline Configuration for Agito

# This file shows how to configure CI/CD pipelines using Agito's git hooks

## Simple Test and Build Pipeline

Create this file as `agito-ci.sh` in your repository root:

```bash
#!/bin/bash
# Simple CI/CD Pipeline for Agito

set -e

BRANCH=$1
OLD_REV=$2
NEW_REV=$3

echo "=== Starting CI/CD Pipeline ==="
echo "Branch: $BRANCH"
echo "Old Rev: $OLD_REV"
echo "New Rev: $NEW_REV"

# Run tests
echo "=== Running Tests ==="
if [ -f "package.json" ]; then
    npm test
elif [ -f "go.mod" ]; then
    go test ./...
elif [ -f "requirements.txt" ]; then
    python -m pytest
fi

# Run build
echo "=== Building ==="
if [ -f "package.json" ]; then
    npm run build
elif [ -f "Makefile" ]; then
    make build
fi

# Deploy if on main branch
if [ "$BRANCH" = "main" ] || [ "$BRANCH" = "master" ]; then
    echo "=== Deploying to Production ==="
    # Add your deployment commands here
    # ./deploy.sh
fi

echo "=== Pipeline Completed Successfully ==="
```

## Docker Build Pipeline

For projects with Docker:

```bash
#!/bin/bash
# Docker Build Pipeline

set -e

BRANCH=$1
IMAGE_NAME="myapp"
TAG="${BRANCH}-${NEW_REV:0:8}"

echo "Building Docker image: $IMAGE_NAME:$TAG"

# Build the image
docker build -t "$IMAGE_NAME:$TAG" .

# Run tests in container
docker run --rm "$IMAGE_NAME:$TAG" npm test

# Push to registry if on main
if [ "$BRANCH" = "main" ]; then
    docker tag "$IMAGE_NAME:$TAG" "$IMAGE_NAME:latest"
    docker push "$IMAGE_NAME:$TAG"
    docker push "$IMAGE_NAME:latest"
fi
```

## Node.js Example

```bash
#!/bin/bash
set -e

echo "Installing dependencies..."
npm ci

echo "Running linter..."
npm run lint

echo "Running tests..."
npm test

echo "Building application..."
npm run build

echo "Success!"
```

## Go Example

```bash
#!/bin/bash
set -e

echo "Running go mod tidy..."
go mod tidy

echo "Running tests..."
go test -v ./...

echo "Running linter..."
golangci-lint run

echo "Building..."
go build -o app ./cmd/app

echo "Success!"
```

## Python Example

```bash
#!/bin/bash
set -e

echo "Installing dependencies..."
pip install -r requirements.txt

echo "Running flake8..."
flake8 .

echo "Running tests..."
pytest --cov

echo "Success!"
```

## Setting Up CI/CD

1. Create `agito-ci.sh` in your repository root
2. Make it executable: `chmod +x agito-ci.sh`
3. Commit and push to Agito server
4. The post-receive hook will automatically execute it

## Customizing Git Hooks

You can customize the hooks in your repository:

```bash
# In the bare repository on the server
cd /var/lib/agito/repos/myrepo.git/hooks

# Edit post-receive hook
vim post-receive
```

## Environment Variables

Available in hook scripts:
- `GIT_DIR`: The git repository directory
- `BRANCH`: Current branch being pushed
- `OLD_REV`: Previous commit SHA
- `NEW_REV`: New commit SHA

## Security Notes

- Hooks run with the permissions of the git user
- Be careful with secrets - use environment variables
- Validate inputs in custom hooks
- Consider using dedicated CI/CD runners for isolation
