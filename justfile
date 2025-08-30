IMAGE_NAME := "procon"

# Show available commands
default:
  just --list

# Build the Docker image
build:
  docker build -t {{IMAGE_NAME}} .

# Open a shell directly in the container
shell:
  docker run -it {{IMAGE_NAME}} nix-shell -p rustup toybox
