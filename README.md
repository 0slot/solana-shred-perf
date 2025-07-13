# solana-shred-perf

Sample code to test and compare performance between any two Shreds.

# How to use?

## Build
```bash
cargo build
```

## Run
```bash
export RUST_LOG=info && cargo run -- --name-0 <Shred1's name> --port-0 <Port to receive Shred1> --name-1 <Shred2's name> --port-1 <Port to receive Shred2>
```

Where:
- `<Shred1's name>` is an arbitrary name for Shred1, used to distinguish it in printed output.
- `<Port to receive Shred1>` is the port to receive Shred1.
- `<Shred2's name>` is an arbitrary name for Shred2, used to distinguish it in printed output.
- `<Port to receive Shred2>` is the port to receive Shred2.

For example:
```bash
export RUST_LOG=info && cargo run -- --name-0 uk --port-0 20001 --name-1 de --port-1 20002
```

This compares a shred named `uk` with a data receiving port of `20001` to a shred named `de` with a data receiving port of `20002`. Normal execution outputs data as follows:

<img width="656" height="111" alt="image" src="https://github.com/user-attachments/assets/501e8ccf-caef-4a3d-af6d-ecf766d08a0a" />
