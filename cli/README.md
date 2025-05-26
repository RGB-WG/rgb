## Basic concepts

In order to understand the following content, you need to grasp these prerequisite concepts.

### Schema

The RGB protocol uses *Schemas*, akin to classes in OOP, **defines the contract business logic**, i.e. how the contract
works. Each RGB contract is an instance of a schema created via the schema's genesis operation, separating roles for
contract developers and issuers for easier operation.

### State

The **global state** applies to the entire contract (for example the name of token, is not belong to any party of the
contract), while the **ownable state** is associated with specific single-use seals. Contracts also use special syntax
structures like braces, brackets, and question marks to denote sets or arrays of data types involved in state operations
and their optionality.

## Install

from source

```
$ git clone <https://github.com/RGB-WG/rgb>
$ cd rgb/cli
$ cargo install --all-features --path .
```

## Data Directory

The RGB wallet stores its data in a directory specified by the `DATA_DIR` constant.

The `DATA_DIR_ENV`  environment variable be used to override the default data directory location. If not set, the
default data directory locations are:

- Linux and BSD-based systems: `~/.lnp-bp`
- macOS: `~/Library/Application Support/LNP-BP Suite`
- Windows: `%LOCALAPPDATA%\\LNP-BP Suite`
- iOS: `~/Documents`
- Android: `.` (the current working directory)

The wallet will create the data directory if it does not already exist. The data directory is used to store the wallet's
configuration, transaction history, and other persistent data.

The base directory of the wallet will be `$data_dir/$network`.

## Configuration File

The default configuration file is `rgb.toml`.

Currently, the only supported configuration key is `default_wallet`, and the default value is `default`.

## Overview

Here is the command line help for rgb-cli.

```
Command-line wallet for RGB smart contracts on Bitcoin

Usage: rgb [OPTIONS] <COMMAND>

Commands:
  list       List known named wallets
  default    Get or set default wallet
  create     Create a named wallet
  address    Generate a new wallet address(es)
  finalize   Finalize a PSBT, optionally extracting and publishing the signed transaction
  extract    Extract a signed transaction from PSBT. The PSBT file itself is not modified
  taprets    List known tapret tweaks for a wallet
  schemata   Prints out list of known RGB schemata
  contracts  Prints out list of known RGB contracts
  import     Imports RGB data into the stash: contracts, schema, etc
  export     Exports existing RGB contract
  armor      Convert binary RGB file into a text armored version
  state      Reports information about state of a contract
  history    Print operation history for a contract
  utxos      Display all known UTXOs belonging to this wallet
  issue      Issues new contract
  invoice    Create new invoice
  prepare    Prepare PSBT file for transferring RGB assets
  consign    Prepare consignment for transferring RGB assets
  transfer   Transfer RGB assets
  inspect    Inspects any RGB data file
  dump       Debug-dump all stash and inventory data
  validate   Validate transfer consignment
  accept     Validate transfer consignment & accept to the stash
  help       Print this message or the help of the given subcommand(s)

Options:
  -v, --verbose...
          Set verbosity level.

          Can be used multiple times to increase verbosity.

  -w, --wallet <NAME>
          Use specific named wallet

  -W, --wallet-path <WALLET_PATH>
          Use wallet from a given path

      --tapret-key-only <TAPRET_KEY_ONLY>
          Use tapret(KEY) descriptor as wallet

      --wpkh <WPKH>
          Use wpkh(KEY) descriptor as wallet

      --electrum[=<URL>]
          Electrum server to use

          [env: ELECRTUM_SERVER=]

      --esplora[=<URL>]
          Esplora server to use

          [env: ESPLORA_SERVER=]

      --mempool[=<URL>]
          Mempool server to use

          [env: MEMPOOL_SERVER=]

      --sync
          Force-sync wallet data with the indexer before performing the operation

  -d, --data-dir <DATA_DIR>
          Data directory path

          Path to the directory that contains RGB stored data.

          [env: LNPBP_DATA_DIR=]
          [default: ~/.lnp-bp]

  -n, --network <NETWORK>
          Network to use

          [env: LNPBP_NETWORK=]
          [default: testnet3]

      --no-network-prefix
          Do not add network prefix to the `--data-dir`

  -H, --from-height <FROM_HEIGHT>
          Specify blockchain height starting from which witness transactions should be checked for re-orgs

  -h, --help
          Print help (see a summary with '-h')

  -V, --version
          Print version
```

## Preparation

### Create a wallet

To create a wallet, you need to prepare a wallet descriptor. You can create a wallet via `bdk-cli` or other similar
utilities.

Here is an example descriptor:

```shell
[1f09c6b9/86h/1h/0h]tpubDCrfSMscBA93FWm8qounj6kcBjnw6LxmVeKSi6VoYS327VCpoLHARWjdqeVtDt2ujDRznB9m1uXpHkDpDXyXM5gsvg2bMMmFcSHrtWUA4Py/<0;1;9;10>/*
```

```
$ rgb --esplora=https://blockstream.info/testnet/api/ create my_wallet --wpkh "[1f09c6b9/86h/1h/0h]tpubDCrfSMscBA93FWm8qounj6kcBjnw6LxmVeKSi6VoYS327VCpoLHARWjdqeVtDt2ujDRznB9m1uXpHkDpDXyXM5gsvg2bMMmFcSHrtWUA4Py/<0;1;9;10>/*"
```

Now we can find the related files created in the wallet runtime directory:

```shell
$ ls ~/.lnp-bp/testnet3/my_wallet

cache.yaml  data.toml  descriptor.toml
```

### List wallets

Usage:

```shell
$ rgb list
```

Example output:

```shell
Known wallets:
my_wallet                       wpkh([1f09c6b9/86h/1h/0h]tpubDCrfSMscBA93FWm8qounj6kcBjnw6LxmVeKSi6VoYS327VCpoLHARWjdqeVtDt2ujDRznB9m1uXpHkDpDXyXM5gsvg2bMMmFcSHrtWUA4Py/<0;1;9;10>/*)
```

### Set default wallet

Now let’s set our default wallet to `my_wallet`

```shell
$ rgb default my_wallet
```

## Assets

### Import schemata

The schemata file’s name ends with `.rgba`, and the standard schemata can be found
in [`https://github.com/RGB-WG/rgb-schemata`](https://github.com/RGB-WG/rgb-schemata) repository.

You can take a look
at [https://github.com/RGB-WG/rgb-schemata/blob/master/schemata/NonInflatableAssets.rgba](https://github.com/RGB-WG/rgb-schemata/blob/master/schemata/NonInflatableAssets.rgba)
which is the NIA schema.

Example:

```shell
$ rgb import rgb-schemata/schemata/NonInflatableAssets.rgb
```

### List schemata

```shell
$ rgb schemata
```

Example Output:

```shell
NonInflatableAsset              rgb:sch:tq4jbmu9hL6kJ5galPSMBH37K1g6MqPlxTa8$!0jhZs#marble-simon-avalon                2024-04-17      ssi:LZS1ux-gjD9nXPF-OcetUUkW-6r3uSCS6-aQhs9W5f-8JE7w
```

### Issue a contract

Usage:

```
$ rgb issue <ISSUER> <CONTRACT_PATH>
```

Tutorial:

Write a contract declaration. (YAML in this example)

```yaml
schema: tq4jbmu9hL6kJ5galPSMBH37K1g6MqPlxTa8$!0jhZs#marble-simon-avalon

globals:
  spec:
    naming:
      ticker: DBG
      name: Debug asset
      details: "Pay attention: the asset has no value"
      precision: 2
  data:
    terms: >
      SUBJECT TO, AND WITHOUT IN ANY WAY LIMITING, THE REPRESENTATIONS AND WARRANTIES OF ANY SELLER 
      EXPRESSLY SET FORTH IN THIS AGREEMENT OR ANY OTHER EXPRESS OBLIGATION OF SELLERS PURSUANT TO THE
      TERMS HEREOF, AND ACKNOWLEDGING THE PRIOR USE OF THE PROPERTY AND PURCHASER’S OPPORTUNITY 
      TO INSPECT THE PROPERTY, PURCHASER AGREES TO PURCHASE THE PROPERTY “AS IS”, “WHERE IS”, 
      WITH ALL FAULTS AND CONDITIONS THEREON. ANY WRITTEN OR ORAL INFORMATION, REPORTS, STATEMENTS, 
      DOCUMENTS OR RECORDS CONCERNING THE PROPERTY PROVIDED OR MADE AVAILABLE TO PURCHASER, ITS AGENTS
      OR CONSTITUENTS BY ANY SELLER, ANY SELLER’S AGENTS, EMPLOYEES OR THIRD PARTIES REPRESENTING OR
      PURPORTING TO REPRESENT ANY SELLER, SHALL NOT BE REPRESENTATIONS OR WARRANTIES, UNLESS
      SPECIFICALLY SET FORTH HEREIN. IN PURCHASING THE PROPERTY OR TAKING OTHER ACTION HEREUNDER,
      PURCHASER HAS NOT AND SHALL NOT RELY ON ANY SUCH DISCLOSURES, BUT RATHER, PURCHASER SHALL RELY
      ONLY ON PURCHASER’S OWN INSPECTION OF THE PROPERTY AND THE REPRESENTATIONS AND WARRANTIES 
      HEREIN. PURCHASER ACKNOWLEDGES THAT THE PURCHASE PRICE REFLECTS AND TAKES INTO ACCOUNT THAT THE
      PROPERTY IS BEING SOLD “AS IS”.
    media: ~
  issuedSupply: 100000000

assignments:
  assetOwner:
    seal: fb9ae7ae4b70a27e7fdfdefac91b37967b549d65007dbf25470b0817a2ae810a:1
    amount: 100000000 # this is 1 million (we have two digits for cents)

```

Here, we observe a seal value in the form of `txid:vout`. This hash, in
reality, represents the TXID of the previously created PSBT. And `txid:vout` is
the outpoint of a valid UTXO.

Compile the contract:

```
$ rgb issue issuerName ./examples/nia-demo.yaml
```

A contract (which also serves as a consignment) will be generated and imported into the current runtime's stock.

Output:

```shell
A new contract rgb:hcRzR8wK-zh$jdpc-Rhsg!uH-WQ!zuV9-h7x877N-BQNcwNM is issued and added to the stash.
```

### Export contract

Next, we export the contract that was just created.

```shell
$ rgb export 'rgb:hcRzR8wK-zh$jdpc-Rhsg!uH-WQ!zuV9-h7x877N-BQNcwNM'
-----BEGIN RGB CONSIGNMENT-----
Id: urn:lnp-bp:consignment:Ctc1wq-Xrqm78uM-nNaDsoHj-TJESKydn-4GLgtYmr-G9AdQE#smoke-oxford-burger
Version: v2
Type: contract
Contract-Id: rgb:hcRzR8wK-zh$jdpc-Rhsg!uH-WQ!zuV9-h7x877N-BQNcwNM
Checksum-SHA256: 50468d33da7aab15c8c2b467126b721c4c3c6cf31d00c8964fb12e23fbc64777

0ssM^4-D2iQYiE=(kr<ho`PqD7ID7TPL?t(cy6J>o^uy=TL1t60DmODi%$$wo#Ma
...

-----END RGB CONSIGNMENT-----
```

The consignment encoded in base64 format will be output to the `stdout`.

Alternatively, you can specify a file name to obtain the binary consignment:

```shell
$ rgb export 'rgb:hcRzR8wK-zh$jdpc-Rhsg!uH-WQ!zuV9-h7x877N-BQNcwNM' demo.rgb

Contract rgb:hcRzR8wK-zh$jdpc-Rhsg!uH-WQ!zuV9-h7x877N-BQNcwNM exported to 'demo.rgb'
```

### Import contract (or other kind of consignment)

Consignments can be imported using the import subcommand, but the RGB CLI already automatically imports the contract, so
there is no need to execute it.

```shell
$ rgb --esplora=https://blockstream.info/testnet/api/ import demo.rgb
```

### Read the contract state

```shell
$ rgb state 'rgb:hcRzR8wK-zh$jdpc-Rhsg!uH-WQ!zuV9-h7x877N-BQNcwNM'

Global:
  spec := ticker "DEMO", name "Demo asset", details "Pay attention: the asset has no value".some, precision centi
  terms := text "SUBJECT TO, AND WITHOUT IN ANY WAY LIMITING, THE REPRESENTATIONS AND WARRANTIES OF ANY SELLER  EXPRESSLY SET FORTH IN THIS AGREEMENT OR ANY OTHER EXPRESS OBLIGATION OF SELLERS PURSUANT TO THE TERMS HEREOF, AND ACKNOWLEDGING THE PRIOR USE OF THE PROPERTY AND PURCHASER’S OPPORTUNITY  TO INSPECT THE PROPERTY, PURCHASER AGREES TO PURCHASE THE PROPERTY “AS IS”, “WHERE IS”,  WITH ALL FAULTS AND CONDITIONS THEREON. ANY WRITTEN OR ORAL INFORMATION, REPORTS, STATEMENTS,  DOCUMENTS OR RECORDS CONCERNING THE PROPERTY PROVIDED OR MADE AVAILABLE TO PURCHASER, ITS AGENTS OR CONSTITUENTS BY ANY SELLER, ANY SELLER’S AGENTS, EMPLOYEES OR THIRD PARTIES REPRESENTING OR PURPORTING TO REPRESENT ANY SELLER, SHALL NOT BE REPRESENTATIONS OR WARRANTIES, UNLESS SPECIFICALLY SET FORTH HEREIN. IN PURCHASING THE PROPERTY OR TAKING OTHER ACTION HEREUNDER, PURCHASER HAS NOT AND SHALL NOT RELY ON ANY SUCH DISCLOSURES, BUT RATHER, PURCHASER SHALL RELY ONLY ON PURCHASER’S OWN INSPECTION OF THE PROPERTY AND THE REPRESENTATIONS AND WARRANTIES  HEREIN. PURCHASER ACKNOWLEDGES THAT THE PURCHASE PRICE REFLECTS AND TAKES INTO ACCOUNT THAT THE PROPERTY IS BEING SOLD “AS IS”.
", media ~
  issuedSupply := 100000000

Owned:
  State         Seal                                                                            Witness
  assetOwner:
```

### List contract

Execute:

```shell
$ rgb contracts
```

Example output:

```shell
rgb:hcRzR8wK-zh$jdpc-Rhsg!uH-WQ!zuV9-h7x877N-BQNcwNM    BitcoinTestnet3 2025-03-08      rgb:sch:tq4jbmu9hL6kJ5galPSMBH37K1g6MqPlxTa8$!0jhZs#marble-simon-avalon
  Developer: issuerName
```

### Take an address

```shell
$ rgb address
Term.   Address
&0/1    tb1qeyu926l47099vtp7wewvhwt03vc5sn5c6t604p
```

Run multiple times to generate more addresses at different indexes. To view an address at given index, for example `0`,
execute:

```shell
$ rgb address --index 0
Term.   Address
&0/0    tb1qeyu926l47099vtp7wewvhwt03vc5sn5c6t604p
```

### Create an address based invoice

```shell
$ rgb invoice --address-based 'rgb:hcRzR8wK-zh$jdpc-Rhsg!uH-WQ!zuV9-h7x877N-BQNcwNM' --amount 100
```

Created invoice:

```shell
rgb:hcRzR8wK-zh$jdpc-Rhsg!uH-WQ!zuV9-h7x877N-BQNcwNM/~/BF+tb3:wvout:A3g1x$Br-FOhcIKD-uN!xToP-cbF20bA-AAAAAAA-AAAAAAA-AIi0trA
```

The invoice string could also includes some additional parameters that are encoded as query parameters, which are
separated by the `?` character. These parameters are used to provide additional information about the transaction, such
as the operation being performed or the assignment associated with the transaction.

### Validate the consignment

```shell
$ rgb --esplora=https://blockstream.info/testnet/api/ validate demo.rgb
```

Example output:

```shell
Consignment has non-mined terminal(s)
Non-mined terminals:
- f17d544c0ac161f758d379c4366e6ede8f394da9633671908738b415ae5c8fb4
Validation warnings:
- terminal witness transaction f17d544c0ac161f758d379c4366e6ede8f394da9633671908738b415ae5c8fb4 is not yet mined.
```

### Sign and broadcast the transaction

Create transfer:

```shell
$ rgb transfer <INVOICE> <CONSIGNMENT> [PSBT]
$ rgb transfer \
    rgb:hcRzR8wK-zh$jdpc-Rhsg!uH-WQ!zuV9-h7x877N-BQNcwNM/~/BF+tb3:wvout:A3g1x$Br-FOhcIKD-uN!xToP-cbF20bA-AAAAAAA-AAAAAAA-AIi0trA \
    transfer.consignment \
    alice.psbt
```

Now you can use bdk-cli or any other wallet to sign and broadcast the transaction.

### Wait for confirmation and accept transfer

As receiver:

```shell
$ rgb accept -f <FILE>
```
