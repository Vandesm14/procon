## User Stories

- I want to provide a GitHub link and have it clone my repo for me
  - Pull on update
  - Target a branch or commit
- I want to tell it how to build and run my project and have it run it
  - Install system deps, build, run
- I want to configure my project for systemctl in the config and run it
- I want to give it a Nginx site file and have it integrate it
  - Build or run-time modules/scripts
- I want to have it manage a remote server
  - SSH

## Lifecycle

- [ ] Commands
  - [ ] Plan (captures config changes)
  - [ ] Apply (applies config changes)
  - [ ] Update (updates source)
- [ ] Config
  - [ ] Added
    - [ ] Setup, Install, Build
  - [ ] Removed
    - [ ] Teardown
  - [ ] Changed
    - [ ] Teardown
    - [ ] Setup, Install, Build
- [ ] Phase
  - [ ] Setup (hook)
  - [ ] Install
    - [ ] Installs source
  - [ ] Update
    - [ ] Updates source
  - [ ] Build
    - [ ] Builds source
    - [ ] Generates daemon (service)
    - [ ] Generates Nginx config
  - [ ] Start
    - [ ] Applies daemon
    - [ ] Applies Nginx
    - [ ] Starts daemon
  - [ ] Stop
    - [ ] Stops daemon
  - [ ] Teardown
    - [ ] Stops daemon
    - [ ] Removes daemon
    - [ ] Removes Nginx
    - [ ] Removes artifacts
