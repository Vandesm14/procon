FROM nixos/nix

# Update nix channels and install base tools
RUN nix-channel --update
RUN nix-env -iA nixpkgs.yazi nixpkgs.toybox nixpkgs.rsync
RUN nix-shell -p rustup --run "rustup default stable"

# Create working directories
RUN mkdir -p /build /app/test /usr/local/bin
ENV PATH="$PATH:/usr/local/bin"

# Copy shims (no-op commands for testing) to /usr/bin so they're always in PATH
COPY docker/shims/* /usr/local/bin/
RUN chmod +x /usr/local/bin/systemctl /usr/local/bin/nginx /usr/local/bin/sudo

# Set up build directory
WORKDIR /build

# Keep container running
CMD ["sleep", "infinity"]
