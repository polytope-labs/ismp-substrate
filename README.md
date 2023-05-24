# pallet-ismp ![Unit Tests](https://github.com/polytope-labs/substrate-ismp/actions/workflows/ci.yml/badge.svg)

Implementation of the Interoperable State Machine Protocol for substrate runtimes. This project is [funded by the web3 foundation](https://github.com/w3f/Grants-Program/blob/master/applications/ismp.md).

## Overview

This repo holds all the required components substrate runtimes need to interoperate together using [ISMP](https://github.com/polytope-labs/ismp)  

* [pallet-ismp](./)  
* [ismp-runtime-api](./pallet-ismp/runtime-api)  
* [ismp-rpc](./pallet-ismp/rpc)

### Parachain Support

* [ismp-parachain](./parachain)
* [ismp-parachain-inherent](./parachain/inherent)
* [ismp-parachain-runtime-api](./parachain/runtime-api)

## Documentation

Installation and integration guides can be found in the [book](https://substrate-ismp.polytope.technology).

## License

This library is licensed under the Apache 2.0 License, Copyright (c) 2023 Polytope Labs.