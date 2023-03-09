// RGB smart contracts for Bitcoin & Lightning
//
// SPDX-License-Identifier: Apache-2.0
//
// Written in 2019-2023 by
//     Dr Maxim Orlovsky <orlovsky@lnp-bp.org>
//
// Copyright (C) 2019-2023 LNP/BP Standards Association. All rights reserved.
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

use std::path::PathBuf;

use rgbstd::contract::ContractId;

#[derive(Subcommand, Clone, PartialEq, Eq, Debug, Display, Default)]
#[display(lowercase)]
pub enum Command {
    /// Prints out detailed information about RGB stash.
    #[default]
    #[clap(alias = "stash")]
    Info,

    /// Imports RGB data into the stash: contracts, schema, interfaces etc.
    #[display("import")]
    Import {
        /// Use BASE64 ASCII armoring for binary data.
        #[clap(short)]
        armored: bool,

        /// File with RGB data. If not provided, assumes `-a` and prints out
        /// data to STDOUT.
        file: Option<PathBuf>,
    },

    /// Exports existing RGB data from the stash.
    #[display("export")]
    Export {
        /// Use BASE64 ASCII armoring for binary data.
        #[clap(short)]
        armored: bool,

        /// File with RGB data. If not provided, assumes `-a` and reads the data
        /// from STDIN.
        file: Option<PathBuf>,
    },

    /// Reports information about state of a contact.
    State { contract_id: ContractId },

    /// Issues new contract.
    #[display("issue")]
    Issue {
        /// Schema name to use for the contract.
        schema: String,

        /// Interface name to use for the contract.
        iface: String,

        /// File containing contract genesis description in YAML format.
        contract: PathBuf,
    },
}

impl Command {
    pub fn exec(self) {
        match self {
            Self::Info => {
                println!("RGB stash");
            }
            Command::Import { .. } => {}
            Command::Export { .. } => {}
            Command::State { .. } => {}
            Command::Issue { .. } => {}
        }
    }
}
