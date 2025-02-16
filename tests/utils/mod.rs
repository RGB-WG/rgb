pub mod runtime;
pub mod report;
pub mod chain;

pub const DEFAULT_FEE_ABS: u64 = 400;

use amplify::Display;

#[derive(Debug, Copy, Clone, PartialEq, Eq, Display)]
#[display(lowercase)]
pub enum TransferType {
    Blinded,
    Witness,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Display)]
#[display(lowercase)]
pub enum DescriptorType {
    Wpkh,
    Tr,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Display)]
#[display(lowercase)]
pub enum AssetSchema {
    Nia,
    Uda,
    Cfa,
}
