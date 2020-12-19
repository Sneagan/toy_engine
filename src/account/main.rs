use super::{Transaction, TransactionSet, TransactionType};
use anyhow::{Context, Result};
use csv::Writer;
use itertools::Itertools;
use rust_decimal::prelude::*;
use serde::{Deserialize, Serialize};

/// A representation of known state for a given client identifier.
#[derive(Debug, Serialize, Deserialize)]
pub struct Account {
    client: u16,
    /// The total funds that are available. Equivalent to `total - held`.
    available: Decimal,
    /// The total funds held for dispute. Equivalent to `total - available`.
    held: Decimal,
    /// The total funds in all states. Equivalent to `total + held`
    total: Decimal,
    /// Whether the account is locked as a result of a charge back.
    locked: bool,
    /// The set of transactions that compute the state of the account.
    #[serde(skip_serializing)]
    transactions: TransactionSet,
}

impl Account {
    /// Generates Accounts with fully rendered states from provided CSV data and serializes them
    /// into a provided target that implements the `Write` trait.
    ///
    /// # Arguments
    ///
    /// * `data` - Reference to a Vec<u8> buffer containing CSV data
    /// * `writer` - Anything that implements the Write trait.
    pub fn accounts_state_from_csv_data(
        data: &[u8],
        mut writer: impl std::io::Write,
    ) -> Result<()> {
        let transaction_sets = TransactionSet::transaction_sets_from_csv_data(data)
            .with_context(|| format!("TransactionSet failed generation from the provided data"))?;
        let mut csv_writer = Writer::from_writer(vec![]);
        for transaction_set in transaction_sets.into_iter() {
            let account = Account::from_transaction_set(transaction_set);
            csv_writer
                .serialize(account)
                .with_context(|| format!("Failed to serialize account data to CSV writer."))?;
        }
        let wrtr = csv_writer
            .into_inner()
            .with_context(|| format!("CSV writer data failed to flush internal buffer."))?;
        let data = String::from_utf8(wrtr)
            .with_context(|| format!("Failed to generate UTF-8 from writer buffer."))?;
        writeln!(writer, "{}", data).with_context(|| format!("Writer failed to write results."))
    }

    /// Generates an Account with a fully rendered state from a TransactionSet.
    ///
    /// # Arguments
    ///
    /// * `transaction_set` - A series of transactions with a shared client identifier in
    /// chronological order.
    pub fn from_transaction_set(transaction_set: TransactionSet) -> Account {
        let mut account = Account {
            client: transaction_set.client,
            available: Decimal::new(00, 1),
            held: Decimal::new(00, 1),
            total: Decimal::new(00, 1),
            locked: false,
            transactions: TransactionSet {
                transactions: Vec::new(),
                client: transaction_set.client,
            },
        };

        for transaction in transaction_set.transactions.into_iter() {
            account.resolve_new_transaction(transaction);
        }
        account
    }

    /// Allows the addition of any new transaction to the history of an account. The transaction is
    /// applied to the Account state and appended to the TransactionSet for the Account. Locked
    /// accounts cannot process transactions.
    ///
    /// # Arguments
    ///
    /// * `transaction` - A transaction of any TransactionType
    pub fn resolve_new_transaction(&mut self, transaction: Transaction) {
        // If the provided transaction is not for this client, ignore it.
        if self.client != transaction.client || self.locked {
            ()
        }
        match transaction.transaction_type {
            TransactionType::Deposit(_) => self.deposit(transaction),
            TransactionType::Withdraw(_) => self.withdraw(transaction),
            TransactionType::Dispute => self.dispute(transaction),
            TransactionType::Resolve => self.resolve(transaction),
            TransactionType::Chargeback => self.chargeback(transaction),
        }
    }

    /// Execute a deposit transaction on the Account state. This increases the available amount,
    /// recalculates the total, and pushes the transaction to the Account's TransactionSet.
    ///
    /// # Arguments
    ///
    /// * `transaction` - A transaction of any TransactionType
    fn deposit(&mut self, transaction: Transaction) {
        match transaction.transaction_type {
            TransactionType::Deposit(amount) => {
                self.available = self.available + amount;
                self.total = self.held + self.available;
                self.transactions.transactions.push(transaction);
            }
            _ => (),
        }
    }

    /// Execute a withdraw transaction on the Account state. This decreases the available amount,
    /// recalculates the total, and pushes the transaction to the Account's TransactionSet.
    ///
    /// # Arguments
    ///
    /// * `transaction` - A transaction of any TransactionType
    fn withdraw(&mut self, transaction: Transaction) {
        match transaction.transaction_type {
            TransactionType::Withdraw(amount) => {
                if amount <= self.available {
                    self.available = self.available - amount;
                    self.total = self.held + self.available;
                    self.transactions.transactions.push(transaction);
                }
            }
            _ => (),
        }
    }

    /// Execute a dispute transaction on the Account state. This moves the amount from a withdraw
    /// or deposit transaction into the `held` amount on the Account, changing the available amount,
    /// but not the total.
    ///
    /// # Arguments
    ///
    /// * `transaction` - A transaction of any TransactionType
    fn dispute(&mut self, transaction: Transaction) {
        match transaction.transaction_type {
            TransactionType::Dispute => {
                let disputed_transaction = self.get_transaction(transaction.tx);
                if let Some(txn) = disputed_transaction {
                    match txn.transaction_type {
                        TransactionType::Deposit(amount) => {
                            self.available = self.available - amount;
                            self.held = self.held + amount;
                        }
                        TransactionType::Withdraw(amount) => {
                            self.available = self.available + amount;
                            self.held = self.held - amount;
                        }
                        _ => (),
                    };
                    self.total = self.held + self.available;
                    self.transactions.transactions.push(transaction);
                }
            }
            _ => (),
        }
    }

    /// Execute a resolve transaction on the Account state. This moves the amount from held that
    /// that was palced there during a dispute transaction. This changes the available amount,
    /// but not the total. If there is no dispute in the TransactionSet for the specified resolve
    /// there is no effect.
    ///
    /// # Arguments
    ///
    /// * `transaction` - A transaction of any TransactionType
    fn resolve(&mut self, transaction: Transaction) {
        match transaction.transaction_type {
            TransactionType::Resolve => {
                if let Some(txn) = self.get_transaction(transaction.tx) {
                    if self.transaction_disputed(txn) {
                        match txn.transaction_type {
                            TransactionType::Deposit(amount) => {
                                self.available = self.available + amount;
                                self.held = self.held - amount;
                            }
                            TransactionType::Withdraw(amount) => {
                                self.available = self.available - amount;
                                self.held = self.held + amount;
                            }
                            _ => (),
                        };
                        self.total = self.held + self.available;
                        self.transactions.transactions.push(transaction);
                    }
                }
            }
            _ => (),
        }
    }

    /// Execute a chargeback transaction on the Account state. This finalizes a dispute rather than
    /// resolving it and results in an account lock.
    ///
    /// # Arguments
    ///
    /// * `transaction` - A transaction of any TransactionType
    fn chargeback(&mut self, transaction: Transaction) {
        // This solution uses clone and a strange code structure to avoid having to use
        // any unsafe code despite needing what is otherwise a simultaneous mutable and
        // immutable borrow for get_transaction and resolve.

        // If the account has no unresolved disputes, there is nothing to chargeback.
        if !self.has_unresolved_disputes() {
            ()
        }
        let mut txn_for_resolution: Option<Transaction> = None;
        match transaction.transaction_type {
            TransactionType::Chargeback => {
                if let Some(txn) = self.get_transaction(transaction.tx) {
                    let break_reference = txn.clone();
                    if self.transaction_disputed(&break_reference) {
                        txn_for_resolution = Some(break_reference.clone());
                    }
                }
            }
            _ => (),
        };
        if let Some(txn) = txn_for_resolution {
            self.resolve(txn);
        }
        self.transactions.transactions.push(transaction);
        self.locked = true;
    }

    /// Indicated whether a given transaction is disputed.
    ///
    /// # Arguments
    ///
    /// * `transaction` - A transaction of any TransactionType
    fn transaction_disputed(&self, transaction: &Transaction) -> bool {
        if let TransactionType::Dispute = transaction.transaction_type {
            false
        } else {
            let related_transactions = self
                .transactions
                .transactions
                .iter()
                .filter(|txn| txn.tx == transaction.tx)
                .sorted_by_key(|txn| txn.tx)
                .group_by(|txn| txn.transaction_type);
            let mut disputes = 0;
            let mut resolutions = 0;
            for (key, group) in &related_transactions {
                let count = group.count();
                if key == TransactionType::Dispute {
                    disputes = count;
                }
                if key == TransactionType::Resolve {
                    resolutions = count;
                }
            }
            disputes > resolutions
        }
    }

    /// Indicates whether the account has unresolved disputes.
    ///
    /// # Arguments
    ///
    /// * `transaction` - A transaction of any TransactionType
    fn has_unresolved_disputes(&self) -> bool {
        let disputes = self
            .transactions
            .transactions
            .iter()
            .filter(|txn| match txn.transaction_type {
                TransactionType::Dispute => true,
                _ => false,
            })
            .count();
        let resolutions = self
            .transactions
            .transactions
            .iter()
            .filter(|txn| match txn.transaction_type {
                TransactionType::Resolve => true,
                _ => false,
            })
            .count();
        disputes > resolutions
    }

    /// Returns a transaction by its transaction identifier.
    ///
    /// # Arguments
    ///
    /// * `transaction` - A transaction of any TransactionType
    fn get_transaction(&self, identifier: u32) -> Option<&Transaction> {
        let mut target_transaction = self
            .transactions
            .transactions
            .iter()
            .filter(|txn| txn.tx == identifier);
        target_transaction.next()
    }
}

// Tests

#[cfg(test)]
mod tests {
    use super::*;
    use std::error::Error;
    use std::str;

    #[test]
    fn test_from_csv_data() -> Result<(), Box<dyn Error>> {
        let mut result = Vec::new();
        let sample_input = std::fs::read("test_data/sample_input.csv").unwrap();
        let sample_output = std::fs::read_to_string("test_data/sample_output.csv").unwrap();

        Account::accounts_state_from_csv_data(&sample_input, &mut result)?;

        assert_eq!(str::from_utf8(&result).unwrap(), sample_output);
        Ok(())
    }

    #[test]
    fn test_dispute() {
        let mut account = Account::from_transaction_set(TransactionSet {
            transactions: vec![Transaction {
                transaction_type: TransactionType::Deposit(Decimal::new(5, 0)),
                tx: 1,
                client: 4,
            }],
            client: 4,
        });
        account.dispute(Transaction {
            transaction_type: TransactionType::Dispute,
            tx: 1,
            client: 4,
        });

        assert_eq!(account.total, Decimal::new(50, 1));
        assert_eq!(account.held, Decimal::new(50, 1));
        assert_eq!(account.available, Decimal::new(00, 1));
    }

    #[test]
    fn test_resolve() {
        let mut account = Account::from_transaction_set(TransactionSet {
            transactions: vec![Transaction {
                transaction_type: TransactionType::Deposit(Decimal::new(5, 0)),
                tx: 1,
                client: 4,
            }],
            client: 4,
        });
        account.dispute(Transaction {
            transaction_type: TransactionType::Dispute,
            tx: 1,
            client: 4,
        });
        account.resolve(Transaction {
            transaction_type: TransactionType::Resolve,
            tx: 1,
            client: 4,
        });

        assert_eq!(account.total, Decimal::new(50, 1));
        assert_eq!(account.held, Decimal::new(00, 1));
        assert_eq!(account.available, Decimal::new(50, 1));
    }

    #[test]
    fn test_transaction_disputed() {
        let account = Account::from_transaction_set(TransactionSet {
            transactions: vec![
                Transaction {
                    transaction_type: TransactionType::Deposit(Decimal::new(5, 0)),
                    tx: 1,
                    client: 4,
                },
                Transaction {
                    transaction_type: TransactionType::Dispute,
                    tx: 1,
                    client: 4,
                },
                Transaction {
                    transaction_type: TransactionType::Deposit(Decimal::new(3, 0)),
                    tx: 2,
                    client: 4,
                },
            ],
            client: 4,
        });

        assert_eq!(
            account.transaction_disputed(&account.transactions.transactions[0]),
            true
        );
        assert_eq!(
            account.transaction_disputed(&account.transactions.transactions[1]),
            false
        );
        assert_eq!(
            account.transaction_disputed(&account.transactions.transactions[2]),
            false
        );
    }

    #[test]
    fn test_account_has_disputes() {
        let disputed_account = Account::from_transaction_set(TransactionSet {
            transactions: vec![
                Transaction {
                    transaction_type: TransactionType::Deposit(Decimal::new(5, 0)),
                    tx: 1,
                    client: 4,
                },
                Transaction {
                    transaction_type: TransactionType::Dispute,
                    tx: 1,
                    client: 4,
                },
                Transaction {
                    transaction_type: TransactionType::Deposit(Decimal::new(3, 0)),
                    tx: 2,
                    client: 4,
                },
            ],
            client: 4,
        });

        let undisputed_account = Account::from_transaction_set(TransactionSet {
            transactions: vec![Transaction {
                transaction_type: TransactionType::Deposit(Decimal::new(5, 0)),
                tx: 1,
                client: 4,
            }],
            client: 4,
        });

        assert_eq!(disputed_account.has_unresolved_disputes(), true);
        assert_eq!(undisputed_account.has_unresolved_disputes(), false);
    }
}
