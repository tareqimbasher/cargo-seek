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

<!--<div align="center">
  <a href="https://crates.io/crates/cargo-seek">
    <img src="https://img.shields.io/crates/v/cargo-seek.svg?style=flat-square"
    alt="Crates.io version" />
  </a>
  <a href="https://crates.io/crates/cargo-seek">
    <img src="https://img.shields.io/crates/d/cargo-seek.svg?style=flat-square"
      alt="Download" />
  </a>
</div>
<br/>-->


[preview]: docs/preview.gif?raw=true "preview"
![preview][preview]

# Features 🚀

- Search
    - Sort by: Relevance, Name, Downloads, Recent Downloads, Recently Updated, Newly Added.
    - Search in: Online, Project, Installed or All
    - Visually label project dependencies & installed binaries
    - Paging
- Add, remove crates to projects
- Install, uninstall a cargo binary
- Open docs
- Open repository
- Open crate on [crates.io](https://crates.io)
- Open crate on [lib.rs](https://lib.rs)

# Roadmap 🚧

- Flag outdated with ability to update
- Settings to configure behavior and appearance
- Show more crate details: dependencies, version history...etc
- Open repository README in terminal using `glow` or `mdcat`

# Install

    cargo install --locked cargo-seek

# Usage

    cargo-seek

or as a cargo sub-command:

    cargo seek


**Options**

```
cargo-seek.exe [OPTIONS] [PROJECT_DIR]

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
add and remove crates to your cargo project. You can also direct `crate-seek` to target a specific cargo project
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
| `Tab`               | Switch between boxes in the UI                         |
| `ESC`               | Go back to search; if already there will clear results |
| `Ctrl + Left/Right` | Change column width                                    |
| `Ctrl + h`          | Toggle usage/help screen                               |
| `Ctrl + c`          | Quit                                                   |

## Results

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

# Credits

- The UX was inspired by [`pacseek`](https://github.com/moson-mo/pacseek)
- [ratatui](https://ratatui.rs/)
- [crates-io-api](https://crates.io/crates/crates_io_api)