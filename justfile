IMAGE_NAME := "procon:dev"

# Show available commands
default:
  just --list

# Build the Docker image
build:
  docker build -t {{IMAGE_NAME}} .

# Open a shell directly in the container
run:
  docker run -it {{IMAGE_NAME}} nix-shell -p rustup toybox
