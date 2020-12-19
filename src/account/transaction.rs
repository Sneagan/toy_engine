use super::TransactionType;
use serde::{Deserialize, Serialize};

/// A single transaction. Generally, part of a series of transactions used to
/// determine the state of the associated Account.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Transaction {
    /// Type of transaction. Used to determine how this transaction impacts the
    /// associated account.
    pub transaction_type: TransactionType,
    /// Transaction identifier.
    pub tx: u32,
    /// Client identifier
    pub client: u16,
}
