IMAGE_NAME := "procon"

# Show available commands
default:
  just --list

# Build the Docker image
build:
  docker build -t {{IMAGE_NAME}} .
  docker container rm -f {{IMAGE_NAME}}

# Open a shell directly in the container
shell: build
  clear
  docker run --name {{IMAGE_NAME}} -it {{IMAGE_NAME}} nix-shell -p toybox yazi

serve:
  zip out.zip -r src test Cargo.lock Cargo.toml
  miniserve --index out.zip
