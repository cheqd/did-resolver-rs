# cheqd DID Resolver Rust

[![GitHub release (latest by date)](https://img.shields.io/github/v/release/cheqd/did-resolver-rs?color=green&label=stable%20release&style=flat-square)](https://github.com/cheqd/did-resolver-rs/releases/latest) ![GitHub Release Date](https://img.shields.io/github/release-date/cheqd/did-resolver-rs?color=green&style=flat-square) [![GitHub license](https://img.shields.io/github/license/cheqd/did-resolver-rs?color=blue&style=flat-square)](https://github.com/cheqd/did-resolver-rs/blob/main/LICENSE)

[![GitHub release (latest by date including pre-releases)](https://img.shields.io/github/v/release/cheqd/did-resolver-rs?include_prereleases&label=dev%20release&style=flat-square)](https://github.com/cheqd/did-resolver-rs/releases/) ![GitHub commits since latest release (by date)](https://img.shields.io/github/commits-since/cheqd/did-resolver-rs/latest?style=flat-square) [![GitHub contributors](https://img.shields.io/github/contributors/cheqd/did-resolver-rs?label=contributors%20%E2%9D%A4%EF%B8%8F&style=flat-square)](https://github.com/cheqd/did-resolver-rs/graphs/contributors)

[![GitHub Workflow Status](https://img.shields.io/github/actions/workflow/status/cheqd/did-resolver-rs/dispatch.yml?label=workflows&style=flat-square)](https://github.com/cheqd/did-resolver-rs/actions/workflows/dispatch.yml) [![GitHub Workflow Status](https://img.shields.io/github/actions/workflow/status/cheqd/did-resolver-rs/codeql.yml?label=CodeQL&style=flat-square)](https://github.com/cheqd/did-resolver-rs/actions/workflows/codeql.yml) ![GitHub repo size](https://img.shields.io/github/repo-size/cheqd/did-resolver-rs?style=flat-square)

## ℹ️ Overview

DID methods are expected to provide [standards-compliant methods of DID and DID Document ("DIDDoc") production](https://w3c.github.io/did-resolution/#resolver-architectures). The **cheqd DID Resolver** is designed to implement the [W3C DID _Resolution_ specification](https://w3c.github.io/did-resolution/) for [`did:cheqd`](https://docs.cheqd.io/identity/architecture/adr-list/adr-001-cheqd-did-method) method in Rust.

### 📝 Architecture

The [Architecture Decision Record for the cheqd DID Resolver](https://docs.cheqd.io/identity/architecture/adr-list/adr-003-did-resolver) describes the architecture & design decisions for this software package.

#### gRPC Endpoints used by DID Resolver

Our DID Resolver uses the [Cosmos gRPC endpoint](https://docs.cosmos.network/main/core/grpc_rest) from `cheqd-node` to fetch data. Typically, this would be running on port `9090` on a `cheqd-node` instance.

You can either use [public gRPC endpoints for the cheqd network](https://cosmos.directory/cheqd/nodes) (such as the default ones mentioned above), or point it to your own `cheqd-node` instance by enabling gRPC in the `app.toml` configuration file on a node:

```toml
[grpc]

# Enable defines if the gRPC server should be enabled.
enable = true

# Address defines the gRPC server address to bind to.
address = "0.0.0.0:9090"
```

**Note**: If you're pointing a DID Resolver to your own node instance, by default `cheqd-node` instance gRPC endpoints are _not_ served up with a TLS certificate. This means the `useTls` property would need to be set to `false`, unless you're otherwise using a load balancer that provides TLS connections to the gRPC port.

<!-- cargo-rdme start -->

This crate contains a resolver for DIDs of the [did:cheqd](https://docs.cheqd.io/product/architecture/adr-list/adr-001-cheqd-did-method) method.
The implementation resolves DIDs via gRPC network requests to the configured nodes. Default nodes for cheqd's `mainnet` & `testnet` can be used,
or custom nodes can be opt-in by supplying a different gRPC URL configuration.

This crate uses gRPC types and clients generated using [tonic](https://github.com/hyperium/tonic).
The generated rust code is checked-in to this repository for monitoring, [see here](./src/proto/mod.rs).
These generated rust files are checked-in alongside the V2 cheqd proto files & dependencies.
which are sourced from [cheqd's Buf registry](https://buf.build/cheqd/proto/docs).

Since the generated code & proto files are not relatively large nor overwhelming in content, they are checked-in rather than pulled and/or generated at build time. The benefit is that the contents of the files can be monitored with each update, making supply-chain attacks obvious. It also reduces the build time complexity for consumers - such as reducing requirements for any 3rd party build tools to be installed (`protobuf`). The drawback is that it introduces some more manual maintainence.
The crate exports the `DIDCheqd` type which implements the
[`ssi_dids_core::DIDMethod`] and
[`ssi_dids_core::resolution::DIDMethodResolver`] traits. This crate is
uses cheqd network's GRPC

##### Example

The example below is intentionally minimal and self-contained so it can be
executed as a doc-test (no network calls, no async runtime). It verifies the
public associated constant and basic construction of the type. This keeps
`cargo test --doc` and tools like `cargo-rdme` reliable.

```rust
use did_resolver_cheqd::DIDCheqd;
use ssi_dids_core::DIDMethod;
// Confirm the API constant and that we can construct the value
assert_eq!(DIDCheqd::DID_METHOD_NAME, "cheqd");
let _ = DIDCheqd::default();
let _ = DIDCheqd::new(None);
let _ = DIDCheqd::new(Some(DidCheqdResolverConfiguration {
    networks: vec![
        NetworkConfiguration {
            grpc_url: "https://grpc.cheqd.net:443".to_string(),
            namespace: "mainnet".to_string(),
        },
    ],
}));
```

##### Library features

- Implements a `DIDMethodResolver` for the `did:cheqd` DID method.
- Exposes `resolution`, `proto` and `error` modules for integration.

<!-- cargo-rdme end -->

## 🐞 Bug reports & 🤔 feature requests

If you notice anything not behaving how you expected, or would like to make a suggestion / request for a new feature, please create a [**new issue**](https://github.com/cheqd/did-resolver-rs/issues/new/choose) and let us k

## 💬 Community

Our [**Discord server**](http://cheqd.link/discord-github) is the primary chat channel for the open-source community, software developers, and node operators.

Please reach out to us there for discussions, help, and feedback on the project.

## 🙋 Find us elsewhere

[![Telegram](https://img.shields.io/badge/Telegram-2CA5E0?style=for-the-badge&logo=telegram&logoColor=white)](https://t.me/cheqd) [![Discord](https://img.shields.io/badge/Discord-7289DA?style=for-the-badge&logo=discord&logoColor=white)](http://cheqd.link/discord-github) [![Twitter](https://img.shields.io/badge/Twitter-1DA1F2?style=for-the-badge&logo=twitter&logoColor=white)](https://twitter.com/intent/follow?screen_name=cheqd_io) [![LinkedIn](https://img.shields.io/badge/LinkedIn-0077B5?style=for-the-badge&logo=linkedin&logoColor=white)](http://cheqd.link/linkedin) [![Medium](https://img.shields.io/badge/Medium-12100E?style=for-the-badge&logo=medium&logoColor=white)](https://blog.cheqd.io) [![YouTube](https://img.shields.io/badge/YouTube-FF0000?style=for-the-badge&logo=youtube&logoColor=white)](https://www.youtube.com/channel/UCBUGvvH6t3BAYo5u41hJPzw/)
