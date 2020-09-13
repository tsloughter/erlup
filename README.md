ERLUP
=====

Manage multiple Erlang installs with per directory configuration.

![screenshot](image.png)

## Build

```
$ cargo build --release
```

## Setup

If you download a binary from the github releases you must rename it to `erlup` for it to work.

Because `erlup` creates symlinks from commands like `erl` to the `erlup` binary you must be sure the directory the symlinks are created, `~/.cache/erlup/bin`, is in your `PATH`:

```
$ mkdir -p ~/.cache/erlup/bin
$ export PATH=~/.cache/erlup/bin:$PATH
```

## Build Erlang

`erlup` will create a default config under `~/.config/erlup/config` if you don't create it yourself and it'll contain:

```
[erlup]
dir=<your home>/.cache/erlup

[repos]
default=https://github.com/erlang/otp
```

To list tags available to build one:

```
$ erlup tags
...
$ erlup build OTP-21.2
```

## Add a Repo

To add an alternative Erlang/OTP repo use `erlup repo add <name> <url>`. For
example to add Lukas' repo to build the JIT branch:

``` shell
$ erlup repo add garazdawi https://github.com/garazdawi/otp
$ erlup fetch -r garazdawi
$ erlup build -r garazdawi origin/beamasm
```

## Configuring Erlang Compilation

To pass options to `./configure` (like for setting where SSL ) you can add them in the config file:

``` ini
[erlup]
default_configure_options=--enable-lock-counter
```

Or pass through the env variable `ERLUP_CONFIGURE_OPTIONS`:

``` shellsession
$ ERLUP_CONFIGURE_OPTIONS=--enable-lock-counter erlup build OTP-21.2
```

## Acknowledgements

Inspiration for `erlup` is [erln8](https://github.com/metadave/erln8) by Dave Parfitt. He no longer maintains it and I figured I could use writing my own as a way to learn Rust.
