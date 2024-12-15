// Standard Library for RGB smart contracts
//
// SPDX-License-Identifier: Apache-2.0
//
// Designed in 2019-2025 by Dr Maxim Orlovsky <orlovsky@lnp-bp.org>
// Written in 2024-2025 by Dr Maxim Orlovsky <orlovsky@lnp-bp.org>
//
// Copyright (C) 2019-2024 LNP/BP Standards Association, Switzerland.
// Copyright (C) 2024-2025 LNP/BP Laboratories,
//                         Institute for Distributed and Cognitive Systems (InDCS), Switzerland.
// Copyright (C) 2019-2025 Dr Maxim Orlovsky.
// All rights under the above copyrights are reserved.
//
// Licensed under the Apache License, Version 2.0 (the "License"); you may not use this file except
// in compliance with the License. You may obtain a copy of the License at
//
//        http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software distributed under the License
// is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express
// or implied. See the License for the specific language governing permissions and limitations under
// the License.

#[macro_use]
extern crate amplify;

use hypersonic::Schema;

fn main() {
    /*
    let mut alice = OmniBarrow::load("");

    let seal1 = alice.next_seal_pub();

    let schema = Schema::load("").expect("unable to load schema");
    // Contract is articles object
    let articles = alice.issue();
        .start_issue("firstIssue")
        .append("ticker", svnum!(0u64), Some(ston!(ticker "TICK", name "Token")))
        .assign("tokenOwners", seal1, svnum!(100_0000), None)
        .finish::<Bp>("TokenContract", 1732529307);

    let mut stockpile = Stockpile::new(articles, "examples/token/data");

    let bob = Wallet::load("");
    let seal2 = bob.next_seal_pub();
    let seal_change = alice.next_seal_priv();
    // The seal can be also vout-based, use `next_seal_vout` for that purpose

    // Instead of keeping the whole contract ops in the memory, this can be actually done
    // using state combined with APIs!
    let op = stockpile
        .start_deed("transfer")
        .using(seal1, ston!())
        .append("tokenOwners", seal2, svnum!(10_0000), None)
        .append("tokenOwners", seal_change, svnum!(90_0000), None)
        .commit();

    // PSBT constructor analyses both inputs and outputs of the operation, detecting and checking
    // relevant seals (for instance, vout-based).
    let psbt = alice.construct_psbt(op);
    //let alice_balances = stockpile.select_unspent(alice.utxos(), svnum!(10_0000));

    for contract_id in alice.affected(psbt) {
        // TODO: We need to do blank transitions as well
    }

    let anchor = psbt.extract_anchor(stockpile.contract_id());
    // TODO: We need to extract an anchor per each contract
    // This adds information about UTXOs to the wallet
    let tx = alice.finalize_psbt();

    // Ensuring Alice's contract is updated
    stockpile.append_witness(tx, anchor);

    stockpile.consign_to_file([seal2], "examples/token/transfer.rgb");
    tx.broadcast();
    alice.save("");

    // Bob's site:
    // let diff = Deeds::diff("", transfer);
    // let transitions = state.accept(diff); // This also does the verification
    // Trace::extend("", transitions);
    // Deeds::extend("", diff);

    // Locker, token (stash) and trace are append-only logs
    // Only state, wallet (alice) must persist in memory
     */
    todo!()
}
