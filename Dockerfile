FROM nixos/nix

# Update nix channels and install base tools
RUN nix-channel --update
RUN nix-env -iA nixpkgs.yazi nixpkgs.toybox nixpkgs.rsync
RUN nix-shell -p rustup --run "rustup default stable"

# Create working directories
RUN mkdir -p /build /app/test /usr/local/bin
ENV PATH="$PATH:/usr/local/bin"

# Set up build directory
WORKDIR /build

# Keep container running
CMD ["sleep", "infinity"]
