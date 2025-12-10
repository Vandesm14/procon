CONTAINER_NAME := "procon-dev"

# Show available commands
default:
  just --list

# Build the base Docker image (only needed once or when Dockerfile changes)
build-image:
  docker-compose build

# Start the development container
start:
  docker-compose up -d
  @echo "Container started. Use 'just sync' to copy files and 'just build' to compile."

# Stop the development container
stop:
  docker-compose down

# Restart the container
restart:
  docker-compose restart

# Sync source files to the container
sync:
  @echo "Syncing source files..."
  docker cp src {{CONTAINER_NAME}}:/build/
  docker cp Cargo.toml {{CONTAINER_NAME}}:/build/
  docker cp Cargo.lock {{CONTAINER_NAME}}:/build/
  @echo "Syncing test files..."
  docker cp test {{CONTAINER_NAME}}:/app/
  @echo "Files synced!"

# Build the Rust project inside the container
build:
  @echo "Building procon..."
  docker exec {{CONTAINER_NAME}} nix-shell -p rustup --run "cd /build && cargo build"
  docker exec {{CONTAINER_NAME}} cp /build/target/debug/procon /usr/local/bin/procon
  @echo "Build complete!"

# Build in release mode
build-release:
  @echo "Building procon (release)..."
  docker exec {{CONTAINER_NAME}} nix-shell -p rustup --run "cd /build && cargo build --release"
  docker exec {{CONTAINER_NAME}} cp /build/target/release/procon /usr/local/bin/procon
  @echo "Release build complete!"

# Sync and build in one step
update: sync build

# Open a shell in the container
shell: update
  docker exec -it {{CONTAINER_NAME}} nix-shell -p toybox yazi

# Run procon in the container
run *ARGS:
  docker exec -it {{CONTAINER_NAME}} procon {{ARGS}}

# Check container status
status:
  @docker ps -a --filter name={{CONTAINER_NAME}} --format "table {{{{.Names}}}}\t{{{{.Status}}}}\t{{{{.Ports}}}}"

# View container logs
logs:
  docker logs {{CONTAINER_NAME}}

# Clean the build cache (removes target directory)
clean:
  docker exec {{CONTAINER_NAME}} rm -rf /build/target/*
  @echo "Build cache cleaned!"

# Full cleanup (removes container and volumes)
clean-all:
  docker-compose down -v
  @echo "Container and volumes removed!"

# Complete workflow: start, sync, and build
dev: start sync build
  @echo "Development environment ready! Use 'just shell' to enter the container."

# Legacy serve command for ZIP-based workflow
serve:
  rm -rf out.zip
  zip out.zip -r src test Cargo.lock Cargo.toml
  miniserve --index out.zip
