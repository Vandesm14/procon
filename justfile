IMAGE_NAME := "procon"

# Show available commands
default:
  just --list

# Build the Docker image
build:
  docker-compose build
  docker-compose down

# Open a shell directly in the container
shell: build
  clear
  docker-compose run --rm {{IMAGE_NAME}} nix-shell -p toybox yazi

serve:
  rm -rf out.zip
  zip out.zip -r src test Cargo.lock Cargo.toml
  miniserve --index out.zip
