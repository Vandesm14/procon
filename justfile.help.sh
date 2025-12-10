# Don't actually run this file
exit

# First time setup (only needed once)
just build-image  # Build the base Docker image
just dev          # Start container, sync files, and build

# Daily development cycle (super fast!)
# 1. Make changes to your code
# 2. Sync and rebuild:
just update       # Syncs files + rebuilds (or use 'just sync' then 'just build')

# 3. Test in the container:
just shell        # Opens interactive shell
just run <args>   # Run procon directly

# Container management:
just start        # Start the container
just stop         # Stop the container  
just status       # Check if container is running
