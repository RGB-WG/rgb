# RGB smart contracts

![Build](https://github.com/RGB-WG/rgb/workflows/Build/badge.svg)
![Tests](https://github.com/RGB-WG/rgb/workflows/Tests/badge.svg)
![Lints](https://github.com/RGB-WG/rgb/workflows/Lints/badge.svg)
[![codecov](https://codecov.io/gh/RGB-WG/rgb/branch/master/graph/badge.svg)](https://codecov.io/gh/RGB-WG/rgb)

[![crates.io](https://img.shields.io/crates/v/rgb-wallet)](https://crates.io/crates/rgb-wallet)
[![Docs](https://docs.rs/rgb-wallet/badge.svg)](https://docs.rs/rgb-wallet)
[![unsafe forbidden](https://img.shields.io/badge/unsafe-forbidden-success.svg)](https://github.com/rust-secure-code/safety-dance/)
[![Apache-2 licensed](https://img.shields.io/crates/l/rgb-wallet)](./LICENSE)

RGB is confidential & scalable client-validated smart contracts for Bitcoin &
Lightning. It embraces the concepts of private & mutual ownership, abstraction
and separation of concerns and represents "post-blockchain", Turing-complete
form of trustless distributed computing which does not require introduction of
"tokens". To learn more about RGB please check [RGB black paper][Blackpaper]
and [RGB Tech] websites.

This repository provides client-facing library which can be used by desktop
apps and mobile wallets for integrating RGB support. It also provides binary
`rgb` which runs in the command-line and exposes all RGB functionality locally,
requiring only Electrum server.

The development of the project is supported and managed by [LNP/BP Standards
Association][Association]. The design of RGB smart contract system and
implementation of this and underlying consensus libraries was done in 2019-2023
by [Dr Maxim Orlovsky][Max] basing or earlier ideas of client-side-validation
and RGB as "assets for bitcoin and LN" by [Peter Todd][Todd] and
[Giacomo Zucco][Zucco].

## Installing

First, you need to install [cargo](https://doc.rust-lang.org/cargo/).
Minimum supported rust compiler version (MSRV) is shown in `rust-version` of `Cargo.toml`.

Next, you need to install developer components, which are OS-specific:

* Linux
  ```
  sudo apt update
  sudo apt install -y build-essential cmake pkg-config
  ```

* MacOS
  ```
  brew install cmake pkg-config
  ```

* Windows: download and install the
  latest [Build Tools for Visual Studio](https://aka.ms/vs/17/release/vs_BuildTools.exe), including
  the 'Desktop
  development with C++' workflow and recommended optional features

Finally, install RGB command-line utility shipped with this repo by running

```
cargo install rgb-wallet
```

To use the library from other rust code add dependency to the `Cargo.toml` file:

```toml
[dependencies]
rgb-std = "0.11.0-beta.7" # use the latest version
rgb-runtime = "0.11.0-beta.7" # use the latest version
```

## Using the demo script

Run `examples/demo.sh`.

## Using command-line

1. Initialize the data directory by running `rgb init` command.
2. Import issuing schemata by some developer. For instance, you may take those
   from [the repository of Pandora Prime](https://github.com/pandora-prime/rgb-issuers/tree/master/compiled)
   or download from [rgbex.io]. Once downloaded, run `rgb import <ISSUERS>` command, replacing
   `<ISSUERS>` with a path to the downloaded files (you can use wildmarks, like `*.issuers`).
3. Check that the import has completed successfully by running `rgb contracts --issuers` and
   checking that all imported issuers are present.

## Contributing

Altcoins and "blockchains" other than Bitcoin blockchain/Bitcoin protocols are
not supported and not planned to be supported; pull requests targeting them will
be declined.

## License

See [LICENCE](LICENSE) file.


[Association]: https://lnp-bp.org

[Blackpaper]: https://blackpaper.rgb.tech

[RGB Tech]: https://rgb.tech

[FAQ]: https://rgbfaq.com

[Max]: https://github.com/dr-orlovsky

[Todd]: https://petertodd.org/

[Zucco]: https://giacomozucco.com/

[VS]: https://learn.microsoft.com/en-us/cpp/windows/latest-supported-vc-redist?view=msvc-170

[rgbex.io]: https://rgbex.io
