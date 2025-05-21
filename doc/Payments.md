# Payments Workflows

There are two main ways to do a payment (also called "state transfer") with `rgb` command-line (
coming with `rgb-wallet` crate) and `Runtime` API (coming from [`rgb-runtime`] crate). The first
approach is for simple payments, allowing to pay for an invoice issued by some beneficiary; the
second approach allows arbitrary payment customization and is suited for advanced scenarios.

The user of both payment workflows will work with the following entities:

- **RGB invoice**: a simple payment specification structured as a URI, using `contract:` scheme;
- **PSBT**: partially-signed bitcoin transaction file;
- **RGB consignment**: a stream (which can be serialized to a file) transferring all client-side
  information required for the payment between peers.

Invoice is created by the beneficiery of the payment; which in return receives a consignment from
the payer. PSBT file is used by the payer internally, which must sign it and publish upon receiving
confirmation from the beneficiary. PSBT file can be transferred between parties on the side of the
payer, for instance different signers in multisig wallet, or payjoin peers.

## Paying an invoice

Paying invoice takes just one command-line call - `rgb pay` - which takes an invoice and produces
both ready-to-be-signed PSBT and a consignment file, which should be sent to the beneficiary before
publishing signed PSBT.

With RGB runtime API, you will have to call two methods in a sequence:

1. `Runtime::pay_invoice`, which will return a redy-to-sing PSBT and a **terminal**, containing
   information about the final state which should be exported on the second step to the consignment;
2. `Runtime::consign` (or `Runtime::consign_to_file`), which takes the terminal received on the
   step 1 and produces the consignment for the beneficiary.

## Custom payment

Custom payments are done in multiple steps, providing rich customization options on each of them.

The user of custom payment workflows will be interacting with additional RGB data types and files:

- **Payment script** (`PaymentScript` data type, which can be written and read from `*.yaml`
  files): a scenario which describes multiple payments under potentially multiple contracts;
- **Prefabricated operation bundle** (`PrefabBundle` data type, which is stored in binary form as a
  `*.pfab` file): packed information about multiple operations under multiple contracts.

The custom payment process consists of the following steps, some of which are optional and are not
required in some scenarios:

1. Converting invoice to a payment script (optional): `rgb script` command and `Runtime::script`
   API.
2. Execute payment script, creating PSBT and prefabricated bundle: `rgb exec` command and
   `Runtime::exec` API. The PSBT file, returned by this operation, is not ready to be signed! It
   has to be _completed_ first, as described in step 4 below.
3. Customize PSBT (optional): a PSBT file from the previous step may be further modified in multiple
   ways, like re-ordering inputs or outputs; adding more inputs; merging with other PSBT files
   (which may also contain other RGB payments from other peers). All of these operations may happen
   as a part of independent workflows for transaction aggregation, payjoin or coinjoin etc.
4. Complete PSBT, using `rgb complete` command or `Runtime::complete` API. This creates all
   necessary deterministic bitcoin commitments, after which transaction can't be anymore modified
   and becomes ready to be signed.
5. Share PSBT and prefabricated bundle with other signers (optional). It is important to share both
   of the files, since without prefabricated bundle other signers will have no idea which RGB
   operations the transaction commits to, and won't be able to properly sign it.
6. Produce and send consignment to the beneficiary using `rgb consign` and `Runtime::consign` (or
   `Runtime::consign_to_file`) APIs. The consignment creation will require providing a
   **terminal** (or multiple terminals, if required), which are **authentication tokens** present
   in the payment script in outputs sent to the beneficiary.
7. Sign PSBT; finalize and extract transaction. It is important to note that unlike in non-RGB PSBT
   workflows a partially-signed PSBT transaction can't be modified, merged etc., since this may
   invalidate deterministic bitcoin commitments. If necessary such operations must be performed on
   the step 3. PSBT signing is not managed by RGB runtime API or command-line tool and can be
   performed using existing bitcoin wallets and hardware signers.
8. Receive confirmation from the beneficiary that he is fine with the consignment. Publish the
   signed transaction to the network.

Custom payments are useful in the following cases:

- multisig wallets (makes step 5 required);
- custom coin selection (makes step 3 required);
- payment aggregation (may skip step 1, or use it to create separate scripts, which can be merged
  together; also makes step 3 required);
- payjoin/coinjoin (makes step 3 required);
- lightning.

## Changes from v0.11

### RGB data not kept in PSBT

Previously, information about RGB operations were directly included into PSBT (in form of
proprietary RGB key types in global map). In v0.12 these data are separated into an independent
binary file (called _prefabricated operation bundle_). This have the main reason of enhanced privacy
for payjoin and coinjoin protocols.

### Payment scripts

Previously, creating custom payments required manual construction of all operations, which can be
done only when a PSBT was created; significantly complicating custom coin selection algorithm.

Payment scripts, introduced in v0.12, simplify this, since they can reference witness-output based
seals before a PSBT is actually created. This also allows creating a multi-step payments with
scripts, and then operating on multiple transactions in the same time, which is useful in case of
off-chain transaction graph protocols like Lightning network. Finally, payment scripts allow simpler
editing and visualisation of the payment thanks for used YAML format.

### Consignments as streams

Version 0.12 introduces ability to stream consignment into a networking interface (or to a hardware
verification device), also supporting verification of the consignment from the stream.
