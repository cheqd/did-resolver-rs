use std::{cmp::Ordering, collections::HashMap};

use chrono::{DateTime, Utc};
use tokio::sync::Mutex;
use tonic::transport::{Channel, ClientTlsConfig, Endpoint};

// transformer helpers produce JSON values; no direct types imported here.
use crate::{
    error::{DidCheqdError, DidCheqdResult},
    proto::cheqd::{
        did::v2::{
            QueryDidDocRequest, QueryDidDocVersionRequest,
            query_client::QueryClient as DidQueryClient,
        },
        resource::v2::{
            Metadata as CheqdResourceMetadata, QueryCollectionResourcesRequest,
            QueryResourceRequest, query_client::QueryClient as ResourceQueryClient,
        },
    },
    resolution::parser::DidCheqdParsed,
};

/// default namespace for the cheqd "mainnet". as it would appear in a DID.
pub const MAINNET_NAMESPACE: &str = "mainnet";
/// default gRPC URL for the cheqd "mainnet".
pub const MAINNET_DEFAULT_GRPC: &str = "https://grpc.cheqd.net:443";
/// default namespace for the cheqd "testnet". as it would appear in a DID.
pub const TESTNET_NAMESPACE: &str = "testnet";
/// default gRPC URL for the cheqd "testnet".
pub const TESTNET_DEFAULT_GRPC: &str = "https://grpc.cheqd.network:443";

/// Configuration for the [DidCheqdResolver] resolver
pub struct DidCheqdResolverConfiguration {
    /// Configuration for which networks are resolvable
    pub networks: Vec<NetworkConfiguration>,
}

impl Default for DidCheqdResolverConfiguration {
    fn default() -> Self {
        Self {
            networks: vec![
                NetworkConfiguration::mainnet(),
                NetworkConfiguration::testnet(),
            ],
        }
    }
}

/// Configuration for a cheqd network. Defining details such as where to resolve DIDs from.
pub struct NetworkConfiguration {
    /// the cheqd nodes gRPC URL
    pub grpc_url: String,
    /// the namespace of the network - as it would appear in a DID (did:cheqd:namespace:123)
    pub namespace: String,
}

impl Clone for NetworkConfiguration {
    fn clone(&self) -> Self {
        Self {
            grpc_url: self.grpc_url.clone(),
            namespace: self.namespace.clone(),
        }
    }
}

impl Clone for DidCheqdResolverConfiguration {
    fn clone(&self) -> Self {
        Self {
            networks: self.networks.clone(),
        }
    }
}

impl NetworkConfiguration {
    /// default configuration for cheqd mainnet
    pub fn mainnet() -> Self {
        Self {
            grpc_url: String::from(MAINNET_DEFAULT_GRPC),
            namespace: String::from(MAINNET_NAMESPACE),
        }
    }

    /// default configuration for cheqd testnet
    pub fn testnet() -> Self {
        Self {
            grpc_url: String::from(TESTNET_DEFAULT_GRPC),
            namespace: String::from(TESTNET_NAMESPACE),
        }
    }
}

#[derive(Clone)]
struct CheqdGrpcClient {
    did: DidQueryClient<Channel>,
    resources: ResourceQueryClient<Channel>,
}

pub struct DidCheqdResolver {
    networks: Vec<NetworkConfiguration>,
    network_clients: Mutex<HashMap<String, CheqdGrpcClient>>,
}

// Note: we intentionally avoid depending on external `did_resolver` types here.
// This module exposes string-based resolution helpers that return proto results
// or raw bytes + media type so callers can transform them into the desired
// in-repo types without importing the external did_resolver crate.

impl DidCheqdResolver {
    /// Assemble a new resolver with the given config.
    ///
    /// [DidCheqdResolverConfiguration::default] can be used if default mainnet & testnet
    /// configurations are suitable.
    pub fn new(configuration: DidCheqdResolverConfiguration) -> Self {
        Self {
            networks: configuration.networks,
            network_clients: Default::default(),
        }
    }

    /// lazily get the client, initializing if not already
    async fn client_for_network(&self, network: &str) -> DidCheqdResult<CheqdGrpcClient> {
        let mut lock = self.network_clients.lock().await;
        if let Some(client) = lock.get(network) {
            return Ok(client.clone());
        }

        let network_config = self
            .networks
            .iter()
            .find(|n| n.namespace == network)
            .ok_or(DidCheqdError::NetworkNotSupported(network.to_owned()))?;

        let endpoint = Endpoint::new(network_config.grpc_url.to_string())
            .map_err(|_e| DidCheqdError::BadConfiguration("Failed to parse GRPC url".to_string()))?
            .tls_config(ClientTlsConfig::new().with_webpki_roots())
            .map_err(|e| DidCheqdError::TransportError(Box::new(e)))?;

        // Connect to the channel
        let channel = endpoint
            .connect()
            .await
            .map_err(|e| DidCheqdError::TransportError(Box::new(e)))?;

        let did_client = DidQueryClient::new(channel.clone());
        let resource_client = ResourceQueryClient::new(channel);

        let client = CheqdGrpcClient {
            did: did_client,
            resources: resource_client,
        };

        lock.insert(network.to_owned(), client.clone());

        Ok(client)
    }

    /// Query a DID Doc by a DID string (e.g. "did:cheqd:mainnet:zF7...").
    /// Returns the raw proto DIDDoc and an optional proto metadata object.
    pub async fn query_did_doc_by_str(
        &self,
        _did_str: &str,
        parsed_did: DidCheqdParsed,
    ) -> DidCheqdResult<(
        crate::proto::cheqd::did::v2::DidDoc,
        Option<crate::proto::cheqd::did::v2::Metadata>,
    )> {
        // parsed.namespace is an owned String; borrow as &str for client lookup
        let network = parsed_did.namespace.as_str();
        let mut client = self.client_for_network(network).await?;

        if parsed_did.version.is_some() {
            let request = tonic::Request::new(QueryDidDocVersionRequest {
                id: parsed_did.did.to_string(),
                version: parsed_did.version.unwrap(),
            });
            let response = client
                .did
                .did_doc_version(request)
                .await
                .map_err(|e| DidCheqdError::NonSuccessResponse(Box::new(e)))?;
            let query_response = response.into_inner();
            let query_doc_res = query_response.value.ok_or(DidCheqdError::InvalidResponse(
                "DIDDoc query did version not return a value".into(),
            ))?;
            let query_doc = query_doc_res.did_doc.ok_or(DidCheqdError::InvalidResponse(
                "DIDDoc query did version not return a DIDDoc".into(),
            ))?;

            Ok((query_doc, query_doc_res.metadata))
        } else {
            let request = tonic::Request::new(QueryDidDocRequest {
                id: parsed_did.did.to_string(),
            });
            let response = client
                .did
                .did_doc(request)
                .await
                .map_err(|e| DidCheqdError::NonSuccessResponse(Box::new(e)))?;
            let query_response = response.into_inner();
            let query_doc_res = query_response.value.ok_or(DidCheqdError::InvalidResponse(
                "DIDDoc query did not return a value".into(),
            ))?;
            let query_doc = query_doc_res.did_doc.ok_or(DidCheqdError::InvalidResponse(
                "DIDDoc query did not return a DIDDoc".into(),
            ))?;

            Ok((query_doc, query_doc_res.metadata))
        }
    }

    /// Query a DID resource by a DID URL string and return raw bytes and optional
    /// media type. Supported forms mirror the earlier functionality:
    /// * `did:cheqd:<namespace>:<did>/resources/<resource_id>`
    /// * `did:cheqd:<namespace>:<did>?resourceName=...&resourceType=...&resourceVersionTime=...`
    pub async fn query_resource_by_str(
        &self,
        did_url: &str,
        parsed_did: DidCheqdParsed,
    ) -> DidCheqdResult<(Vec<u8>, Option<String>)> {
        // borrow the owned Strings for local use
        let network = parsed_did.namespace.as_str();
        let did_id = parsed_did.id.as_str();

        // If parser injected a resourceId (from a path like /resources/<id>), resolve by id.
        if let Some(ref qmap) = parsed_did.query {
            if let Some(resource_id) = qmap.get("resourceId") {
                return self
                    .resolve_resource_by_id(did_id, resource_id.as_str(), network)
                    .await;
            }
        }

        // Otherwise, if query parameters indicate name+type lookup, perform that
        if let Some(qmap) = parsed_did.query {
            let resource_name = qmap.get("resourceName");
            let resource_type = qmap.get("resourceType");
            let version_time = qmap.get("resourceVersionTime");

            let (Some(resource_name), Some(resource_type)) = (resource_name, resource_type) else {
                return Err(DidCheqdError::InvalidDidUrl(format!(
                    "Resolver can only resolve by exact resource ID or name+type combination {did_url}"
                )));
            };

            let version_time = match version_time {
                Some(v) => DateTime::parse_from_rfc3339(v)
                    .map_err(|e| DidCheqdError::InvalidDidUrl(e.to_string()))?
                    .to_utc(),
                None => Utc::now(),
            };

            return self
                .resolve_resource_by_name_type_and_time(
                    did_id,
                    resource_name.as_str(),
                    resource_type.as_str(),
                    version_time,
                    network,
                )
                .await;
        }

        Err(DidCheqdError::InvalidDidUrl(format!(
            "No resource path or query present: {did_url}"
        )))
    }

    /// Resolve a resource from a collection (did_id) and network by an exact id.
    async fn resolve_resource_by_id(
        &self,
        did_id: &str,
        resource_id: &str,
        network: &str,
    ) -> DidCheqdResult<(Vec<u8>, Option<String>)> {
        let mut client = self.client_for_network(network).await?;
        let request = QueryResourceRequest {
            collection_id: did_id.to_owned(),
            id: resource_id.to_owned(),
        };
        let response = client
            .resources
            .resource(request)
            .await
            .map_err(|e| DidCheqdError::NonSuccessResponse(Box::new(e)))?;

        let query_response = response.into_inner();
        let query_response = query_response
            .resource
            .ok_or(DidCheqdError::InvalidResponse(
                "Resource query did not return a value".into(),
            ))?;
        let query_resource = query_response
            .resource
            .ok_or(DidCheqdError::InvalidResponse(
                "Resource query did not return a resource".into(),
            ))?;
        let query_metadata = query_response
            .metadata
            .ok_or(DidCheqdError::InvalidResponse(
                "Resource query did not return metadata".into(),
            ))?;

        let media_type =
            (!query_metadata.media_type.trim().is_empty()).then_some(query_metadata.media_type);

        Ok((query_resource.data, media_type))
    }

    /// Resolve a resource from a given collection (did_id) & network, that has a given name & type,
    /// as of a given time.
    async fn resolve_resource_by_name_type_and_time(
        &self,
        did_id: &str,
        name: &str,
        rtyp: &str,
        time: DateTime<Utc>,
        network: &str,
    ) -> DidCheqdResult<(Vec<u8>, Option<String>)> {
        let mut client = self.client_for_network(network).await?;

        let response = client
            .resources
            .collection_resources(QueryCollectionResourcesRequest {
                collection_id: did_id.to_owned(),
                // FUTURE - pagination
                pagination: None,
            })
            .await
            .map_err(|e| DidCheqdError::NonSuccessResponse(Box::new(e)))?;

        let query_response = response.into_inner();
        let resources = query_response.resources;
        let mut filtered: Vec<_> =
            filter_resources_by_name_and_type(resources.iter(), name, rtyp).collect();
        filtered.sort_by(|a, b| desc_chronological_sort_resources(a, b));

        let resource_meta = find_resource_just_before_time(filtered.into_iter(), time);

        let Some(meta) = resource_meta else {
            return Err(DidCheqdError::ResourceNotFound(format!(
                "network: {network}, collection: {did_id}, name: {name}, type: {rtyp}, time: \
                 {time}"
            )));
        };

        let (data, media) = self
            .resolve_resource_by_id(did_id, &meta.id, network)
            .await?;
        Ok((data, media))
    }
}

/// Filter for resources which have a matching name and type
fn filter_resources_by_name_and_type<'a>(
    resources: impl Iterator<Item = &'a CheqdResourceMetadata> + 'a,
    name: &'a str,
    rtyp: &'a str,
) -> impl Iterator<Item = &'a CheqdResourceMetadata> + 'a {
    resources.filter(move |r| r.name == name && r.resource_type == rtyp)
}

/// Sort resources chronologically by their created timestamps
fn desc_chronological_sort_resources(
    b: &CheqdResourceMetadata,
    a: &CheqdResourceMetadata,
) -> Ordering {
    let (a_secs, a_ns) = a
        .created
        .map(|v| {
            let v = v.normalized();
            (v.seconds, v.nanos)
        })
        .unwrap_or((0, 0));
    let (b_secs, b_ns) = b
        .created
        .map(|v| {
            let v = v.normalized();
            (v.seconds, v.nanos)
        })
        .unwrap_or((0, 0));

    match a_secs.cmp(&b_secs) {
        Ordering::Equal => a_ns.cmp(&b_ns),
        res => res,
    }
}

/// assuming `resources` is sorted by `.created` time in descending order, find
/// the resource which is closest to `before_time`, but NOT after.
///
/// Returns a reference to this resource if it exists.
///
/// e.g.:
/// resources: [{created: 20}, {created: 15}, {created: 10}, {created: 5}]
/// before_time: 14
/// returns: {created: 10}
///
/// resources: [{created: 20}, {created: 15}, {created: 10}, {created: 5}]
/// before_time: 4
/// returns: None
fn find_resource_just_before_time<'a>(
    resources: impl Iterator<Item = &'a CheqdResourceMetadata>,
    before_time: DateTime<Utc>,
) -> Option<&'a CheqdResourceMetadata> {
    let before_epoch = before_time.timestamp();

    for r in resources {
        let Some(created) = r.created else {
            continue;
        };

        let created_epoch = created.normalized().seconds;
        if created_epoch < before_epoch {
            return Some(r);
        }
    }

    None
}

#[cfg(test)]
mod unit_tests {
    use crate::resolution::parser::DidCheqdParser;

    use super::*;

    #[tokio::test]
    async fn test_resolve_fails_if_no_network_config() {
        let did = "did:cheqd:devnet:Ps1ysXP2Ae6GBfxNhNQNKN";
        let resolver = DidCheqdResolver::new(Default::default());
        let e = resolver
            .query_did_doc_by_str(did, DidCheqdParser::parse(did).unwrap())
            .await
            .unwrap_err();
        assert!(matches!(e, DidCheqdError::NetworkNotSupported(_)));
    }

    #[tokio::test]
    async fn test_resolve_fails_if_bad_network_uri() {
        let did = "did:cheqd:devnet:Ps1ysXP2Ae6GBfxNhNQNKN";
        let config = DidCheqdResolverConfiguration {
            networks: vec![NetworkConfiguration {
                grpc_url: "@baduri://.".into(),
                namespace: "devnet".into(),
            }],
        };

        let resolver = DidCheqdResolver::new(config);
        let e = resolver
            .query_did_doc_by_str(did, DidCheqdParser::parse(did).unwrap())
            .await
            .unwrap_err();
        assert!(matches!(e, DidCheqdError::BadConfiguration(_)));
    }

    #[tokio::test]
    async fn test_resolve_resource_fails_if_no_query() {
        let url = "did:cheqd:mainnet:zF7rhDBfUt9d1gJPjx7s1J";
        let resolver = DidCheqdResolver::new(Default::default());
        let e = resolver
            .query_resource_by_str(url, DidCheqdParser::parse(url).unwrap())
            .await
            .unwrap_err();
        assert!(matches!(e, DidCheqdError::InvalidDidUrl(_)));
    }

    #[tokio::test]
    async fn test_resolve_resource_fails_if_incomplete_query() {
        let url = "did:cheqd:mainnet:zF7rhDBfUt9d1gJPjx7s1j?resourceName=asdf";
        let resolver = DidCheqdResolver::new(Default::default());
        let e = resolver
            .query_resource_by_str(url, DidCheqdParser::parse(url).unwrap())
            .await
            .unwrap_err();
        assert!(matches!(e, DidCheqdError::InvalidDidUrl(_)));
    }

    #[tokio::test]
    async fn test_resolve_resource_fails_if_invalid_resource_time() {
        // use epoch instead of XML DateTime
        let url = "did:cheqd:mainnet:zF7rhDBfUt9d1gJPjx7s1J?resourceName=asdf&resourceType=fdsa&resourceVersionTime=12341234";
        let resolver = DidCheqdResolver::new(Default::default());
        let e = resolver
            .query_resource_by_str(url, DidCheqdParser::parse(url).unwrap())
            .await
            .unwrap_err();
        assert!(matches!(e, DidCheqdError::InvalidDidUrl(_)));
    }

    #[tokio::test]
    async fn test_resolve_did_success() {
        // use epoch instead of XML DateTime
        let did = "did:cheqd:testnet:f5101dd8-447f-40a7-a9b8-700abeba389a".to_string();
        let resolver = DidCheqdResolver::new(Default::default());
        let res = resolver
            .query_did_doc_by_str(&did, DidCheqdParser::parse(&did).unwrap())
            .await;
        println!("res: {res:?}");
        assert!(res.is_ok());
    }

    #[tokio::test]
    async fn test_resolve_resource_id_success() {
        // use epoch instead of XML DateTime
        let did_url = "did:cheqd:testnet:f5101dd8-447f-40a7-a9b8-700abeba389a/resources/6155f8bc-d9c9-4e83-a1bb-453744fe5438".to_string();
        let resolver = DidCheqdResolver::new(Default::default());
        let res = resolver
            .query_resource_by_str(&did_url, DidCheqdParser::parse(&did_url).unwrap())
            .await;
        println!("res: {res:?}");
        assert!(res.is_ok());
    }

    #[tokio::test]
    async fn test_resolve_resource_query_success() {
        // use epoch instead of XML DateTime
        let did_url = "did:cheqd:testnet:f5101dd8-447f-40a7-a9b8-700abeba389a?resourceName=Patient ID 85905-Schema&resourceType=anonCredsSchema".to_string();
        let resolver = DidCheqdResolver::new(Default::default());
        let res = resolver
            .query_resource_by_str(&did_url, DidCheqdParser::parse(&did_url).unwrap())
            .await;
        println!("res: {res:?}");
        assert!(res.is_ok());
    }

    #[tokio::test]
    async fn test_resolve_did_version_id() {
        // use epoch instead of XML DateTime
        let did = "did:cheqd:testnet:ac2b9027-ec1a-4ee2-aad1-1e316e7d6f59/versions/ff82cc93-25fd-493a-8896-9303a9c8383d".to_string();
        let resolver = DidCheqdResolver::new(Default::default());
        let res = resolver
            .query_did_doc_by_str(&did, DidCheqdParser::parse(&did).unwrap())
            .await;
        println!("res: {res:?}");
        assert!(res.is_ok());
    }
}
