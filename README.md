# Quote - DEX Pool quoting CLI and Library

Simple tool inspired by UNIX philosophy

Inputs:

- pool_id (address or hash)
- protocol
- token_in: address
- token_out: address
- amount_in: uint256

Output:

- amount_out: uint256

## CLI Output

Returns simple unformatted value of amount_out, formatting is the task of other tools

## Quoting

To quote pools we use Ephemeral Contracts - smart contracts that are not deployable, those contracts contain data retrieval logic in constructor and on finish of data gathering they revert immediately all the data, which should be decoded by out service

In current quoting library we don't use off-chain maths and try to do everything on-chain using Node and smart contracts

Quotes using math and via getting data
