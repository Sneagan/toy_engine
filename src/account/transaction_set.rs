use super::{Transaction, TransactionType};
use anyhow::{Error, Result};
use csv;
use itertools::Itertools;
use serde::{Deserialize, Serialize};

/// A series of chronologically sequential Transations for a given Account.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransactionSet {
    /// Group of transactions with the same client
    pub transactions: Vec<Transaction>,
    /// Client identifier
    pub client: u16,
}

impl TransactionSet {
    /// Converts CSV data into TransactionSets that can be turned into Account definitions
    /// # Arguments
    ///
    /// * `data` - Reference to a Vec<u8> of CSV data. The argument type makes this method ready
    /// to process CSV file data from any source.
    pub fn transaction_sets_from_csv_data(data: &[u8]) -> Result<Vec<TransactionSet>, Error> {
        let mut csv_results = csv::ReaderBuilder::new()
            .trim(csv::Trim::All)
            .from_reader(data);
        let headers = csv_results.headers()?.clone();
        let grouped_transactions = csv_results
            .records()
            .filter_map(|result| match result {
                Ok(record) => {
                    let transaction_type = TransactionType::from_record(&headers, &record);
                    let tx_opt = match headers.iter().position(|x| x == "tx") {
                        Some(index) => record.get(index),
                        None => None,
                    };
                    let client_opt = match headers.iter().position(|x| x == "client") {
                        Some(index) => record.get(index),
                        None => None,
                    };
                    // These unwraps could be eliminated with nested `if let` or `match` blocks. If
                    // the assumptions regarding source data change to allow more loose constraints
                    // this could be necessary.
                    if transaction_type.is_ok() && tx_opt.is_some() && client_opt.is_some() {
                        Some(Transaction {
                            transaction_type: transaction_type.unwrap(),
                            tx: tx_opt.unwrap().parse::<u32>().unwrap(),
                            client: client_opt.unwrap().parse::<u16>().unwrap(),
                        })
                    } else {
                        None
                    }
                }
                Err(_) => None,
            })
            .sorted_by_key(|transaction| transaction.client)
            .group_by(|transaction| transaction.client);

        let mut transaction_sets = Vec::new();
        for (key, group) in &grouped_transactions {
            transaction_sets.push(TransactionSet {
                transactions: group.collect(),
                client: key,
            });
        }
        Ok(transaction_sets)
    }
}
