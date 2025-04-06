![cargo-seek][logo]

[logo]: img/logo.png?raw=true "cargo-seek"

[preview]: img/preview.gif?raw=true "preview"

[![CI](https://github.com/tareqimbasher/cargo-seek/actions/workflows/ci.yml/badge.svg)](https://github.com/tareqimbasher/cargo-seek/actions/workflows/ci.yml)

**cargo-seek** is a terminal user interface (TUI) for searching, adding and installing cargo crates.

![preview][preview]

## Features

- [x] Search
    - [x] Sorting
    - [x] Search in: Online, Project, Installed or All
    - [ ] Filter By Category
- [x] Flag added & installed crates
- [x] Add, remove
- [x] Install, uninstall
- [ ] Show more details (dependencies, version history...)
- [x] Open ReadMe
- [x] Open Docs
- [x] Open crates.io
- [x] Open lib.rs

## Install

    cargo install cargo-seek

## Usage

    cargo-seek

**Options**

```
cargo-seek.exe [OPTIONS] [PROJ_DIR]

Arguments:
  [PROJ_DIR]  Path to a directory containing (or one of its parents) a cargo.toml file

Options:
  -s, --search <TERM>  Start a search on start
  -h, --help           Print help
  -V, --version        Print version
  
UI Options:
  -f, --fps <FLOAT>    Frame rate, i.e. number of frames per second [default: 30]
  -t, --tps <FLOAT>    Tick rate, i.e. number of ticks per second [default: 4]
      --counter        Show TPS/FPS counter
```

**Cargo Projects**

If a cargo project (`cargo.toml`) is found in the current directory or one of its parents, you can use `cargo-seek` to
add and remove crates to your cargo project. You can also direct `crate-seek` to target a specific cargo project
directory:

    # dir, or one of its parents, should contain a cargo.toml file
    cargo-seek /path/to/dir


## Keyboard shortcuts

### Search

| Key        | Action       |
|------------|--------------|
| `Enter`    | Run search   |
| `Ctrl + s` | Sort         |
| `Ctrl + a` | Search scope |

### Navigation

| Key                 | Action                                    |
|---------------------|-------------------------------------------|
| `Tab`               | Switch between boxes in the UI            |
| `ESC`               | Go back to search; again to clear results |
| `Ctrl + Left/Right` | Change column width                       |
| `Ctrl + h`          | Toggle usage/help screen                  |
| `Ctrl + c`          | Quit                                      |

### Results

| Key               | Action                            |
|-------------------|-----------------------------------|
| `a`               | Add crate to current project      |
| `r`               | Remove crate from current project |
| `i`               | Install binary                    |
| `u`               | Uninstall binary                  |
| `Ctrl + d`        | Open docs                         |
| `Left, Right`     | Go previous/next page             |
| `Home, End`       | Go to first/last crate in page    |
| `Ctrl + Home/End` | Go to first/last page             |
