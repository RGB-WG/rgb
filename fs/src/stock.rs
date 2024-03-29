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

use std::path::Path;

use amplify::confinement::U32;
use rgbstd::persistence::Stock;
use strict_encoding::{DeserializeError, SerializeError, StrictDeserialize, StrictSerialize};

pub trait StockFs: Sized {
    fn load(path: impl AsRef<Path>) -> Result<Self, DeserializeError>;
    fn store(&self, path: impl AsRef<Path>) -> Result<(), SerializeError>;
}

impl StockFs for Stock {
    fn load(file: impl AsRef<Path>) -> Result<Self, DeserializeError> {
        Stock::strict_deserialize_from_file::<U32>(file)
    }

    fn store(&self, file: impl AsRef<Path>) -> Result<(), SerializeError> {
        self.strict_serialize_to_file::<U32>(file)
    }
}
