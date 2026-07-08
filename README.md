<p align="center">
  <img src="docs/cargo-seek-128.png?raw=true">
</p>

<h1 align="center">cargo-seek</h1>
<div align="center">
 <strong>
   A terminal user interface (TUI) for searching, adding and installing cargo crates.
 </strong>
</div>

<br />

<div align="center">
  <a href="https://crates.io/crates/cargo-seek">
    <img src="https://img.shields.io/crates/v/cargo-seek.svg?style=flat-square"
      alt="Crates.io version" />
  </a>
  <a href="https://crates.io/crates/cargo-seek">
    <img src="https://img.shields.io/crates/d/cargo-seek.svg?style=flat-square"
      alt="Downloads" />
  </a>
  <a href="https://github.com/tareqimbasher/cargo-seek/actions/workflows/ci.yml">
    <img src="https://img.shields.io/github/actions/workflow/status/tareqimbasher/cargo-seek/ci.yml?branch=main&style=flat-square"
      alt="CI status" />
  </a>
  <a href="https://crates.io/crates/cargo-seek">
    <img src="https://img.shields.io/crates/l/cargo-seek.svg?style=flat-square"
      alt="License" />
  </a>
</div>
<br/>


[preview]: docs/preview.gif?raw=true "preview"
![preview][preview]

# Features 🚀

- Search [crates.io](https://crates.io) with sorting and scoping
    - Sort by: Relevance, Name, Downloads, Recent Downloads, Recently Updated, Newly Added
    - Search in: Online, Project, Installed, or All
    - Project dependencies and installed binaries are visually labelled
    - Paged results
- Add and remove crates in the current cargo project
- Install and uninstall cargo binaries
- Pick features when adding or installing — your selection builds the exact `cargo add` /
  `cargo install` command (passing `--no-default-features` when a default feature is unchecked)
- Rich crate details pane: description, stable/latest version, release count and recent
  versions (with yank markers), license, MSRV, crate size, features, downloads, publisher,
  and links
- Confirmation prompt before removing or uninstalling a crate
- Quick links: open a crate's docs, repository, [crates.io](https://crates.io), or [lib.rs](https://lib.rs) page

# Roadmap 🚧

- Flag outdated crates with the ability to update them
- Show crate dependencies in the details pane
- In-app settings screen for behavior and appearance
- Open a crate's README in the terminal using `glow` or `mdcat`

# Install

### From crates.io

    cargo install --locked cargo-seek

### Arch Linux (AUR)

Community-maintained package on the AUR: [`cargo-seek`](https://aur.archlinux.org/packages/cargo-seek).
Install it with an AUR helper:

    yay -S cargo-seek   # or: paru -S cargo-seek

### Prebuilt binaries

Download a prebuilt binary for your platform from the
[latest release](https://github.com/tareqimbasher/cargo-seek/releases/latest). Builds are provided for
Linux, macOS, and Windows on both `x86_64` and `arm64`. The Linux binaries are statically linked, so
they run on any distribution regardless of glibc version.

# Usage

    cargo-seek

or as a cargo sub-command:

    cargo seek


**Options**

```
cargo-seek [OPTIONS] [PROJECT_DIR]

Arguments:
  [PROJECT_DIR]  Path to a directory containing (or one of its parents) a Cargo.toml file
                 Default: <current directory>
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

If a cargo project (`Cargo.toml`) is found in the current directory or one of its parents, you can use `cargo-seek` to
add and remove crates in your cargo project. You can also direct `cargo-seek` to target a specific cargo project
directory:

    # dir, or one of its parents, should contain a cargo.toml file
    cargo seek /path/to/dir

# Key Bindings

## Search

| Key        | Action       |
|------------|--------------|
| `Enter`    | Run search   |
| `Ctrl + a` | Search scope |
| `Ctrl + s` | Sort         |

## Navigation

| Key                 | Action                                                 |
|---------------------|--------------------------------------------------------|
| `Tab` / `Shift + Tab` | Move focus between panes                              |
| `/`                 | Jump to the search box                                 |
| `ESC`               | Go back to search; if already there will clear results |
| `Ctrl + Left/Right` | Change column width                                    |
| `PgUp / PgDn`       | Scroll the crate details pane                          |
| `Ctrl + h`          | Toggle help screen                                     |
| `Ctrl + c`          | Quit                                                   |

## Results

| Key               | Action                                     |
|-------------------|--------------------------------------------|
| `a`               | Add crate to current project (pick features) |
| `r`               | Remove crate from current project          |
| `i`               | Install binary (pick features)             |
| `u`               | Uninstall binary                           |
| `Ctrl + d`        | Open docs                                   |
| `Up, Down`        | Select crate in list                       |
| `Left, Right`     | Go previous/next page                       |
| `Home, End`       | Go to first/last crate in page             |
| `Ctrl + Home/End` | Go to first/last page                      |

# Credits

- The UX was inspired by [`pacseek`](https://github.com/moson-mo/pacseek)
- [ratatui](https://ratatui.rs/)
- [crates-io-api](https://crates.io/crates/crates_io_api)