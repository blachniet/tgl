# tgl

A simple Toggl command line client.

## Installation

The package name is [`tgl-cli` on crates.io][1].

```sh
cargo install tgl-cli
```

## Usage

The binary name is `tgl`. It will request your Toggle API token the first time you run it. It will store the token in your system's keyring so that you don't need to provide it in the future.

```sh
tgl
```

Alternatively, you can set the `TOGGL_API_TOKEN` environment variable. You can retrieve your Toggl API token from <https://track.toggl.com/profile>.

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

## Contributing

### Release checklist

Use [cargo-release][2] to deploy new releases.

```sh
cargo release minor [--execute]
gh release create --generate-notes
```

[1]: https://crates.io/crates/tgl-cli
[2]: https://github.com/crate-ci/cargo-release
