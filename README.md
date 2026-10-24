# Ghost-cms-publisher

Validate and preview Markdown collections offline, then publish them to Ghost.

## Installation

```sh
git clone https://github.com/P4suta/ghost-cms-publisher.git
cd ghost-cms-publisher
mise install
mise x -- cargo install --locked --path crates/ghost-cms-cli
```

## Usage

```sh
# Read docs/usage.md for authentication and publication options.
ghost-cms check blog/posts/
ghost-cms preview blog/posts/ --output preview
ghost-cms publish blog/posts/ --dry-run
```

## License

Licensed under either license:

- [Apache License, Version 2.0](LICENSE-APACHE)
- [Massachusetts Institute of Technology (MIT) license](LICENSE-MIT)

Contributions use both licenses unless you state other terms.
