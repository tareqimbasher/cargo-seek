![seekr][logo]

[logo]: img/logo.png?raw=true "seekr"
[preview]: img/preview.gif?raw=true "preview"

[![CI](https://github.com/tareqimbasher/cargo-seek/actions/workflows/ci.yml/badge.svg)](https://github.com/tareqimbasher/cargo-seek/actions/workflows/ci.yml)

**cargo-seek** is a fast search and management tool for rust crates.

It's meant to be a quick way to search for crates on crates.io, add crates to your projects and install cargo binaries.

![preview][preview]

## Features

- [x] Search
  - [x] Sorting
  - [ ] Category
  - [x] Tag added
  - [ ] Tag installed
  - [ ] Add
  - [ ] Install
  - [ ] Open ReadMe
  - [x] Open Docs
- [ ] Project Management Tab
  - [ ] Start by listing added
- [ ] bin tab
  - [ ] Start by listing globally installed binaries

## Install
    cargo install cargo-seek

## Usage
    cargo-seek
    
**or**

    cargo-seek [SEARCHTERM]
      -t, --tps <FLOAT>  Tick rate, i.e. number of ticks per second [default: 4]
      -f, --fps <FLOAT>  Frame rate, i.e. number of frames per second [default: 30]
          --counter      Show TPS/FPS counter
      -h, --help         Print help
      -V, --version      Print version**



|Key|Action|
|-|-|
|`Enter`|Search crates|
|`Shift + Enter`|Sort by|
|`Esc`|Go back to Search, and if there, reset search|
|`Tab`|Move between different panels|
