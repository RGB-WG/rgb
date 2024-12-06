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

The **state extension** allows the public to participate in specific logical parts of the contract, such as declaring a
Burn. State extension operations allow anyone to create state extensions without on-chain commitments, similar to
Bitcoin transactions not yet packaged into a block.

### Interface

In RGB, contract interfaces are similar to Ethereum’s ERC standards. Generic interfaces are called “RGBxx” and are
defined as independent LNP/BP standards.

**Interface Definition**: Defines global states (like Ticker and Name) and ownable states (like Inflation and Asset),
along with operations (like Issue and Transfer).

**Interface Implementation**: When implementing an interface, states and operations of a specific schema are bound to
the interface. For example, the FungibleToken interface implements global and ownable state bindings for the
DecentralizedIdentity schema.

## Install

from source

```
$ git clone <https://github.com/RGB-WG/rgb>
$ cd rgb/cli
$ cargo install --path --all-features .
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
  list              List known wallets
  default           Get or set default wallet
  create            Create a wallet
  address           Generate a new wallet address(es)
  taprets
  schemata          Prints out list of known RGB schemata
  interfaces        Prints out list of known RGB interfaces
  contracts         Prints out list of known RGB contracts
  import            Imports RGB data into the stash: contracts, schema, interfaces, etc
  export            Exports existing RGB contract
  armor             Convert binary RGB file into a text armored version
  state             Reports information about state of a contract
  history-fungible  Print operation history for a default fungible token under a given interface
  utxos             Display all known UTXOs belonging to this wallet
  issue             Issues new contract
  invoice           Create new invoice
  prepare           Prepare PSBT file for transferring RGB assets. In the most of cases you need to use `transfer` command instead of `prepare` and `consign`
  consign           Prepare consignment for transferring RGB assets. In the most of cases you need to use `transfer` command instead of `prepare` and `consign`
  transfer          Transfer RGB assets
  inspect           Inspects any RGB data file
  dump              Debug-dump all stash and inventory data
  validate          Validate transfer consignment
  accept            Validate transfer consignment & accept to the stash
  help              Print this message or the help of the given subcommand(s)

Options:
  -v, --verbose...
          Set verbosity level
  -w, --wallet <NAME>

  -W, --wallet-path <WALLET_PATH>
          Path to wallet directory
      --tapret-key-only <TAPRET_KEY_ONLY>
          Use tapret(KEY) descriptor as wallet
      --wpkh <WPKH>
          Use wpkh(KEY) descriptor as wallet
  -e, --esplora <URL>
          Esplora server to use [env: ESPLORA_SERVER=] [default: <https://blockstream.info/testnet/api>]
      --sync

  -d, --data-dir <DATA_DIR>
          Data directory path [env: LNPBP_DATA_DIR=] [default: ~/.lnp-bp]
  -n, --network <NETWORK>
          Network to use [env: LNPBP_NETWORK=] [default: testnet]
  -h, --help
          Print help (see more with '--help')
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
$ rgb create my_wallet --wpkh "[1f09c6b9/86h/1h/0h]tpubDCrfSMscBA93FWm8qounj6kcBjnw6LxmVeKSi6VoYS327VCpoLHARWjdqeVtDt2ujDRznB9m1uXpHkDpDXyXM5gsvg2bMMmFcSHrtWUA4Py/<0;1;9;10>/*"
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
my_wallet
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
urn:lnp-bp:sc:9ZKGvK-tGs6nJvr-HQVRDDyV-zPnJYE5U-J2mb6yDi-PgBrby#frog-order-costume
```

### Import interface

Now we need to import the interface definition and interface implementation, otherwise you may encounter an error:

```shell
Error: no known interface implementation for XXX
```

Execute:

```shell
$ rgb import ../rgb-schemata/interfaces/RGB20.rgb
$ rgb import ../rgb-schemata/schemata/NonInflatableAssets-RGB20.rgb
```

### List interface

```shell
$ rgb interfaces
```

```shell
RGB21 urn:lnp-bp:if:KtMq1E-bFRhMzn5-sc9NezhQ-kn2JzeJn-VxjDCqru-sieYa#portal-ecology-hostel
RGB25 urn:lnp-bp:if:75swax-yN5mDaKB-B3peGeLu-tLctU3Ef-rAjFySp7-RMLTVF#cable-kayak-david
RGB20 urn:lnp-bp:if:9UMsvx-HkLVK5VT-GkSy7yNU-ihAUBo7a-hxQvLCFq-U4aouK#object-spring-silk
```

### Issue a contract

Usage:

```
$ rgb issue [OPTIONS] <SCHEMA_ID> <CONTRACT_PATH>

```

Tutorial:

Write a contract declaration. (YAML in this example)

```yaml
interface: RGB20

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
  created: 1687969158

assignments:
  assetOwner:
    seal: tapret1st:fb9ae7ae4b70a27e7fdfdefac91b37967b549d65007dbf25470b0817a2ae810a:1
    amount: 100000000 # this is 1 million (we have two digits for cents)

```

Here, we observe a seal value in the form of `closing_method:txid:vout` and here closing method is `tapret1st` (can also
be `opret1st`). This hash, in reality, represents the txid of the previously created PSBT. And `txid:vout` is the
outpoint of a valid UTXO.

Compile the contract:

```
$ rgb issue urn:lnp-bp:sc:9ZKGvK-tGs6nJvr-HQVRDDyV-zPnJYE5U-J2mb6yDi-PgBrby#frog-order-costume ./examples/rgb20-demo.yaml

```

A contract (which also serves as a consignment) will be generated and imported into the current runtime's stock.

Output:

```shell
A new contract rgb:DF4vyV9-i85ZzUqbq-QLxvKtgtp-AJk9NvpL3-k4AHmcRrf-vyHksB is issued and added to the stash.
```

### Export contract

Next, we export the contract that was just created.

```shell
$ rgb export rgb:DF4vyV9-i85ZzUqbq-QLxvKtgtp-AJk9NvpL3-k4AHmcRrf-vyHksB
RGB: command-line wallet for RGB smart contracts
     by LNP/BP Standards Association

Loading descriptor from wallet my_wallet ... success
Loading stock ... success
-----BEGIN RGB CONSIGNMENT-----
Id: urn:lnp-bp:consignment:Ctc1wq-Xrqm78uM-nNaDsoHj-TJESKydn-4GLgtYmr-G9AdQE#smoke-oxford-burger
Version: v2
Type: contract
Contract-Id: rgb:DF4vyV9-i85ZzUqbq-QLxvKtgtp-AJk9NvpL3-k4AHmcRrf-vyHksB
Checksum-SHA256: 50468d33da7aab15c8c2b467126b721c4c3c6cf31d00c8964fb12e23fbc64777

0ssM^4-D2iQYiE=(kr<ho`PqD7ID7TPL?t(cy6J>o^uy=TL1t60DmODi%$$wo#Ma
...

-----END RGB CONSIGNMENT-----
```

The consignment encoded in base64 format will be output to the `stdout`.

Alternatively, you can specify a file name to obtain the binary consignment:

```shell
$ rgb export rgb:DF4vyV9-i85ZzUqbq-QLxvKtgtp-AJk9NvpL3-k4AHmcRrf-vyHksB demo.rgb  

Contract rgb:DF4vyV9-i85ZzUqbq-QLxvKtgtp-AJk9NvpL3-k4AHmcRrf-vyHksB exported to 'demo.rgb'
```

### Import contract (or other kind of consignment)

Consignments can be imported using the import subcommand, but the RGB CLI already automatically imports the contract, so
there is no need to execute it.

```shell
$ rgb import demo.rgb
```

### Read the contract state

```shell
$ rgb state rgb:DF4vyV9-i85ZzUqbq-QLxvKtgtp-AJk9NvpL3-k4AHmcRrf-vyHksB RGB20

Global:
  spec := (naming=(ticker=("DBG"), name=("Debug asset"), details=1(("Pay attention: the asset has no value"))), precision=2)
  data := (terms=("..."), media=~)
  issuedSupply := (100000000)
  created := (1687969158)

Owned:
  assetOwner:

```

### List contract

Execute:

```shell
$ rgb contracts
```

Example output:

```shell
rgb:DF4vyV9-i85ZzUqbq-QLxvKtgtp-AJk9NvpL3-k4AHmcRrf-vyHksB
```

### Take an address

```shell
$ rgb address
Term.   Address
&0/0    tb1qeyu926l47099vtp7wewvhwt03vc5sn5c6t604p
```

Run multiple times to generate more addresses at different indexes. To view an address at given index, for example `0`,
execute:

```shell
$ rgb address --index 0
```

### Create an address based invoice

```shell
$ rgb invoice --address-based rgb:DF4vyV9-i85ZzUqbq-QLxvKtgtp-AJk9NvpL3-k4AHmcRrf-vyHksB RGB20 100

```

Created invoice:

```shell
rgb:DF4vyV9-i85ZzUqbq-QLxvKtgtp-AJk9NvpL3-k4AHmcRrf-vyHksB/RGB20/100+tb:q0q6u0urtzn59cg9qacm7c5aq7ud3wmgms7stew
```

Here's a breakdown of the different parts of the invoice string:

1. `rgb:DF4vyV9-i85ZzUqbq-QLxvKtgtp-AJk9NvpL3-k4AHmcRrf-vyHksB`: This is the contract ID, which is a unique identifier
   for the contract associated with this invoice.
2. `RGB20`: This is the interface (or protocol) used for the transaction.
3. `100`: This is the amount of the transaction, which is 100 units.
4. `tb:q0q6u0urtzn59cg9qacm7c5aq7ud3wmgms7stew`: This is the beneficiary of the transaction

The invoice string could also includes some additional parameters that are encoded as query parameters, which are
separated by the `?` character. These parameters are used to provide additional information about the transaction, such
as the operation being performed or the assignment associated with the transaction.

### Validate the consignment

```shell
$ rgb validate demo.rgb
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
$ rgb transfer <INVOICE> <CONSIGNMENT_FILE> [PSBT]
$ rgb transfer \ 
    rgb:2bLwMXo-deVgzKq97-GUVy6wXea-G1nE84nxw-v5CX3WSJN-mbhsMn7/RGB20/1000+bcrt:p9yjaffzhuh9p7d9gnwfunxssngesk25tz7rudu4v69dl6e7w7qhq5x43k5 \
    transfer.consignment \ 
    alice.psbt
```

Now you can use bdk-cli or any other wallet to sign and broadcast the transaction.

### Wait for confirmation and accept transfer

As receiver:

```shell
$ rgb accept -f CONSIGNMENT_FILE
```
