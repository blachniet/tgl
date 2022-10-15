# tgl

A simple Toggl command line client.

## Installation

The package name is [`tgl-cli` on crates.io][1].

```sh
cargo install tgl-cli
```

## Usage

The binary name is `tgl`. It requires that the `TOGGL_API_TOKEN` environment variable is set. You can retrieve your Toggl API token from <https://track.toggl.com/profile>.

Bash/Zsh:

```sh
read -s TOGGL_API_TOKEN
export TOGGL_API_TOKEN
tgl
```

Fish:

```sh
read -sx TOGGL_API_TOKEN
tgl
```

[1]: https://crates.io/crates/tgl-cli
