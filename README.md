# awsconnect

`awsconnect` is a command line utility to simplify using [aws-vault](aws-vault) for complex workflows, currently predominantly execing into ECS containers.

## Installation

The easiest way to install `awsconnect` is via cargo.

To do this you'll need [Rust 1.5.8 or later installed](https://rustup.rs/), after which you can run `cargo install awsconnect`.

You can also install it from the [Github releases](https://github.com/arranf/awsconnect/releases) page.

## Usage

```sh
awsconnect help
awsconnect login
awsconnect execute
```
