# consenpico-bench

`consenpico-bench` is the benchmark of Ethereum consensus' state transition functions by using [ream](https://github.com/ReamLabs/ream) within [Pico zkVM](https://github.com/brevis-network/pico).

## Requirements

- [Rust](https://rustup.rs/)
- [Pico](https://pico-docs.brevis.network/getting-started/installation)

## Running the Project

### Generate benchmarks

```sh
cd host
make download
make run-<OPERATION_NAME>
```

```sh
OPERATIONS = attestation attester_slashing block_header bls_to_execution_change deposit execution_payload proposer_slashing sync_aggregate voluntary_exit withdrawals
```

This will execute the program and generate benchmarks (especially for cycles) in `./host/summaries` directory.
