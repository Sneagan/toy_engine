# Toy Engine

This toy takes transaction data and renders it into Account states. At the moment it only handles CSV data, but the structure was designed to allow any data source with the correct values to be turned into a rendered `Account`.

## Accounts and Transactions

In this toy, Accounts are nothing more than the sum of their ordered transactions. An account can be created with an empty transaction history and then extended as transactions arrive, or it can be initialized with a full transaction history that is rendered into the Account's current state.

Transactions are collected into an ordered set and persisted on the account itself to permit extension of the transaction history after initial generation even if the account was brought up with a fully up-to-date transaction history.

Note: Naming was ambiguous in the input data, so the less error-prone `withdraw` column was used for input data indicating account withdrawals.

## Transaction Behaviors

It's not clear if `dispute` transaction types can apply to both `deposit` and `withdrawal` transaction types, but this functionality is supported. The result is the possibility of an overdrawn available account value in some cases. In the event of a chargeback on an overdrawn account, it is possible that an account could be frozen with a negative balance. Further requirements would be needed to handle this case.

## Tests and Failure Modes

Account behaviors like submitting deposit and withdrawal transactions contain business logic that can't be checked by the compiler. While we rely on the type system to keep data correct during the conversion from source to structs, tests are needed on the calculations. These have been created to detect failures, but they can and should be extended if more time is applied to this code base.

The CLI itself also has a basic integration test to ensure that its errors are captured. Additional integration tests would be an improvement as well. With only a single code path this is less critical, but it will become important as the interface grows.

Errors are captured and handled to avoid panics. The sole use of `unwrap` is in `transaction_sets_from_csv_data` and exists for readability of the code block. This usage is wrapped in `is_some` and `is_ok` to prevent panics, but if there is a case unaccounted for the `unwrap`s could be removed at a small readability cost.

## Possible Improvements

Currently, the Account state is tracked through the TransactionSet. For deposits and withdrawals this poses little issue. However, disputes, resolutions, and chargebacks walk the transaction history to determine whether there are unresolved disputes and to determine if a given transaction is disputed. The additional of values on the account to indicate whether it has unresolved disputes and on the transaction to indicate whether that specific transaction has an open dispute would speed up validation in longer TransactionSets.

Additionally, the source file is loaded into memory in its entirety at the moment. Streaming data through is possible with this configuration and would improve the memory footprint of this toy when handling large sources. This improvement will be easier to accomplish with an external data store like a cache or database because it will avoid trying to read an open and mutating file on disk or constantly closing and reopening the file on disk.
