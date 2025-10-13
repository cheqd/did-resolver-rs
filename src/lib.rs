//! This crate contains a resolver for DIDs of the [did:cheqd](https://docs.cheqd.io/product/architecture/adr-list/adr-001-cheqd-did-method) method.
//! The implementation resolves DIDs via gRPC network requests to the configured nodes. Default nodes for cheqd's `mainnet` & `testnet` can be used,
//! or custom nodes can be opt-in by supplying a different gRPC URL configuration.
//!
//! This crate uses gRPC types and clients generated using [tonic](https://github.com/hyperium/tonic).
//! The generated rust code is checked-in to this repository for monitoring, [see here](./src/proto/mod.rs).
//! These generated rust files are checked-in alongside the V2 cheqd proto files & dependencies, [here](./cheqd_proto_gen/proto/),
//! which are sourced from [cheqd's Buf registry](https://buf.build/cheqd/proto/docs).
//!
//! Since the generated code & proto files are not relatively large nor overwhelming in content, they are checked-in rather than pulled and/or generated at build time. The benefit is that the contents of the files can be monitored with each update, making supply-chain attacks obvious. It also reduces the build time complexity for consumers - such as reducing requirements for any 3rd party build tools to be installed (`protobuf`). The drawback is that it introduces some more manual maintainence.
//! The crate exports the `DIDCheqd` type which implements the
//! [`ssi_dids_core::DIDMethod`] and
//! [`ssi_dids_core::resolution::DIDMethodResolver`] traits. This crate is
//! uses cheqd network's GRPC
//!
//! Quick example
//! -------------
//! The example below is intentionally minimal and self-contained so it can be
//! executed as a doc-test (no network calls, no async runtime). It verifies the
//! public associated constant and basic construction of the type. This keeps
//! `cargo test --doc` and tools like `cargo-rdme` reliable.
//!
//! ```
//! use did_resolver_rs::DIDCheqd;
//!
//! // Confirm the API constant and that we can construct the value
//! assert_eq!(DIDCheqd::DID_METHOD_NAME, "cheqd");
//! let _ = DIDCheqd::new();
//! let _ = DIDCheqd::default();
//! ```
//!
//! Library features
//! ----------------
//! - Implements a `DIDMethodResolver` for the `did:cheqd` DID method.
//! - Exposes `resolution`, `proto` and `error` modules for integration.

use crate::resolution::resolver::{DidCheqdResolver, DidCheqdResolverConfiguration};
use crate::resolution::transformer::cheqd_diddoc_to_json;
use serde_json::to_vec;
use ssi_dids_core::resolution::{
    DIDMethodResolver, Error, Metadata as ResolutionMetadata, Options, Output,
};
use ssi_dids_core::{DIDMethod, document, document::representation::MediaType};

pub mod error;
pub mod proto;
pub mod resolution;

pub struct DIDCheqd;

impl DIDCheqd {
    pub fn new() -> Self {
        DIDCheqd
    }
}

impl Default for DIDCheqd {
    fn default() -> Self {
        Self::new()
    }
}

impl DIDMethod for DIDCheqd {
    const DID_METHOD_NAME: &'static str = "cheqd";
}

impl DIDMethodResolver for DIDCheqd {
    async fn resolve_method_representation<'a>(
        &'a self,
        method_specific_id: &'a str,
        options: Options,
    ) -> Result<Output<Vec<u8>>, Error> {
        // Try parse as a DID URL (resource) first, otherwise as a DID
        // We will use the internal cheqd resolver to fetch a DidDocument or a resource and
        // then convert it into bytes (JSON-LD) to match the did:key style Output.
        let cfg = DidCheqdResolverConfiguration::default();
        let resolver = DidCheqdResolver::new(cfg);

        // decide if it's a DidUrl (resource) or a plain DID. We interpret a
        // DID resource when the input contains `/` or `?` characters, otherwise
        // treat it as a method-specific id to be combined with the did:cheqd prefix.
        if method_specific_id.contains('/') || method_specific_id.contains('?') {
            // treat as a full did URL
            match resolver.query_resource_by_str(method_specific_id).await {
                Ok((content_bytes, media_type)) => {
                    return Ok(Output::new(
                        content_bytes,
                        document::Metadata::default(),
                        ResolutionMetadata::from_content_type(media_type),
                    ));
                }
                Err(e) => return Err(Error::internal(format!("cheqd resolver error: {e:?}"))),
            }
        }

        // treat as a did (method specific id)
        let did_str = format!("did:cheqd:{}", method_specific_id);
        match resolver.query_did_doc_by_str(&did_str).await {
            Ok((proto_doc, metadata)) => {
                // convert proto DIDDoc to a JSON representation and serialize
                let json_value = cheqd_diddoc_to_json(proto_doc)
                    .map_err(|e| Error::internal(format!("cheqd transform error: {e:?}")))?;
                let json = to_vec(&json_value).map_err(|e| {
                    Error::internal(format!("failed to serialize DID document: {e}"))
                })?;

                let content_type = options.accept.unwrap_or(MediaType::JsonLd);

                Ok(Output::new(
                    json,
                    match metadata {
                        Some(meta) => document::Metadata {
                            deactivated: Some(meta.deactivated),
                        },
                        None => document::Metadata { deactivated: None },
                    },
                    ResolutionMetadata::from_content_type(Some(content_type.to_string())),
                ))
            }
            Err(e) => Err(Error::internal(format!("cheqd resolver error: {e:?}"))),
        }
    }
}
