use csv;
use rust_decimal::prelude::*;
use serde::{Deserialize, Serialize};

/// Set of possible transaction types.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum TransactionType {
    /// Removal of some amount from a Client
    Withdraw(Decimal),
    /// Addition of some amount to a Client
    Deposit(Decimal),
    /// Freeze a transaction
    Dispute,
    /// Unfreeze a disputed transaction
    Resolve,
    /// User-initiated action that results in an account lock
    Chargeback,
}

impl TransactionType {
    /// Generate a TransactionType with any optional abount data from a CSV record
    ///
    /// # Arguments
    ///
    /// * `headers` - Header data from the CSV file. This is used to dynamically access the record
    /// data regardless of column order. It only depends on the column names being known and
    /// consistent.
    /// * `record` - A StringRecord data row containing transaction data to be parsed into a
    /// TransactionType.
    pub fn from_record(
        headers: &csv::StringRecord,
        record: &csv::StringRecord,
    ) -> Result<TransactionType, String> {
        let type_indicator = match headers.iter().position(|x| x == "type") {
            Some(index) => record.get(index),
            None => None,
        };
        match type_indicator {
            Some(indicator) => match indicator {
                "deposit" => {
                    if let Some(decimal) = TransactionType::amount_from_record(headers, record) {
                        Ok(TransactionType::Deposit(decimal))
                    } else {
                        Err(format!("Failed to parse deposit transaction amount."))
                    }
                }
                "withdraw" => {
                    if let Some(decimal) = TransactionType::amount_from_record(headers, record) {
                        Ok(TransactionType::Withdraw(decimal))
                    } else {
                        Err(format!("Failed to parse withdraw transaction amount."))
                    }
                }
                "dispute" => Ok(TransactionType::Dispute),
                "resolve" => Ok(TransactionType::Resolve),
                "chargeback" => Ok(TransactionType::Chargeback),
                _ => Err(format!("Unknown transaction type.")),
            },
            None => Err(format!("Failed to parse transaction from provided data.")),
        }
    }

    /// Parse the transaction amount from the prvided record, if present.
    ///
    /// # Arguments
    ///
    /// * `headers` - Header data from the CSV file. This is used to dynamically access the record
    /// data regardless of column order. It only depends on the column names being known and
    /// consistent.
    /// * `record` - A StringRecord data row optionally containing an amount
    fn amount_from_record(
        headers: &csv::StringRecord,
        record: &csv::StringRecord,
    ) -> Option<Decimal> {
        let amount_opt = match headers.iter().position(|x| x == "amount") {
            Some(index) => record.get(index),
            None => None,
        };
        match amount_opt {
            Some(value) => match Decimal::from_str(value) {
                Ok(val) => Some(val),
                Err(_) => None,
            },
            None => None,
        }
    }
}
