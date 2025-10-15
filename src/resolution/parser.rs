use crate::error::{DidCheqdError, DidCheqdResult};
use std::collections::HashMap;

/// Parsed representation of a did:cheqd DID or DID URL
#[derive(Debug, PartialEq, Eq)]
pub struct DidCheqdParsed {
    /// The canonical DID string (e.g. `did:cheqd:mainnet:abcd123`)
    pub did: String,
    /// Namespace (e.g. `mainnet`, `testnet`), or default `mainnet` when omitted
    pub namespace: String,
    /// Identifier part (collection / DID id)
    pub id: String,
    /// Optional parsed query parameters
    pub query: Option<HashMap<String, String>>,
    /// Optional version identifier (from `versionId` query param or `/versions/<id>` path)
    pub version: Option<String>,
}

pub const DEFAULT_NAMESPACE: &str = "mainnet";

pub struct DidCheqdParser;

impl DidCheqdParser {
    /// Parse a DID or DID URL of the forms described in the issue.
    ///
    /// Examples accepted:
    /// - `did:cheqd:<namespace>:<identifier>`
    /// - `did:cheqd:<namespace>:<identifier>?resourceName=...&resourceType=...`
    /// - `did:cheqd:<namespace>:<identifier>/resources/<resource_id>`
    /// - namespace may be omitted (defaults to `mainnet`)
    pub fn parse(input: &str) -> DidCheqdResult<DidCheqdParsed> {
        if !input.starts_with("did:cheqd:") {
            return Err(DidCheqdError::MethodNotSupported(format!(
                "not a did:cheqd string: {input}"
            )));
        }

        // split off query
        let (base, query_opt) = match input.split_once('?') {
            Some((b, q)) => (b, Some(q)),
            None => (input, None),
        };

        // strip prefix
        let rest = &base["did:cheqd:".len()..];

        // look for a path after the id (preserves leading slash)
        let (id_part, path_opt) = match rest.split_once('/') {
            Some((p, _suf)) => (p, Some(&rest[p.len()..])),
            None => (rest, None),
        };

        // id_part may contain an optional namespace separated by ':'
        let (namespace, id) = if let Some((ns, id)) = id_part.split_once(':') {
            (ns.to_string(), id.to_string())
        } else {
            (DEFAULT_NAMESPACE.to_string(), id_part.to_string())
        };

        // parse query string into an owned map so we can inject `resourceId` from the path
        let mut query = query_opt.map(parse_query_string);

        // version may come from the path or the query param `versionId` (query takes precedence)
        let mut version: Option<String> = None;
        if let Some(p) = path_opt {
            let parts: Vec<&str> = p.trim_start_matches('/').split('/').collect();
            if parts.len() != 2 {
                return Err(DidCheqdError::InvalidDidUrl(
                    "unsupported path format; expected /resources/<id> or /versions/<id>"
                        .to_string(),
                ));
            }

            match parts[0] {
                "resources" => {
                    let resource_id = parts[1];
                    match &mut query {
                        Some(map) => {
                            map.insert("resourceId".to_string(), resource_id.to_string());
                        }
                        None => {
                            let mut m = HashMap::new();
                            m.insert("resourceId".to_string(), resource_id.to_string());
                            query = Some(m);
                        }
                    }
                }
                "versions" => {
                    version = Some(parts[1].to_string());
                }
                _ => {
                    return Err(DidCheqdError::InvalidDidUrl(
                        "unsupported path segment; only `resources` and `versions` are accepted"
                            .to_string(),
                    ));
                }
            }
        }

        // If the query contains an explicit `versionId`, it takes precedence.
        if let Some(ref qmap) = query {
            if let Some(v) = qmap.get("versionId") {
                version = Some(v.clone());
            }
        }

        let did = format!("did:cheqd:{}:{}", namespace, id);

        Ok(DidCheqdParsed {
            did,
            namespace,
            id,
            query,
            version,
        })
    }
}

fn parse_query_string(q: &str) -> HashMap<String, String> {
    q.split('&')
        .filter_map(|kv| kv.split_once('='))
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_basic_did_with_namespace() {
        let s = "did:cheqd:mainnet:abcd123";
        let p = DidCheqdParser::parse(s).unwrap();
        assert_eq!(p.did, "did:cheqd:mainnet:abcd123".to_string());
        assert_eq!(p.namespace, "mainnet".to_string());
        assert_eq!(p.id, "abcd123".to_string());
        assert!(p.version.is_none());
        assert!(p.query.is_none());
    }

    #[test]
    fn parse_basic_did_without_namespace() {
        let s = "did:cheqd:abcd123";
        let p = DidCheqdParser::parse(s).unwrap();
        assert_eq!(p.did, "did:cheqd:mainnet:abcd123".to_string());
        assert_eq!(p.namespace, "mainnet".to_string());
        assert_eq!(p.id, "abcd123".to_string());
    }

    #[test]
    fn parse_resource_path() {
        let s = "did:cheqd:mainnet:abcd123/resources/r1";
        let p = DidCheqdParser::parse(s).unwrap();
        assert_eq!(p.did, "did:cheqd:mainnet:abcd123".to_string());
        assert_eq!(p.namespace, "mainnet".to_string());
        assert_eq!(p.id, "abcd123".to_string());
        // resource id should be injected into the query map as `resourceId`
        let q = p.query.unwrap();
        assert_eq!(q.get("resourceId").map(String::as_str), Some("r1"));
        assert!(p.version.is_none());
    }

    #[test]
    fn parse_versions_path() {
        let s = "did:cheqd:mainnet:abcd123/versions/v1";
        let p = DidCheqdParser::parse(s).unwrap();
        assert_eq!(p.did, "did:cheqd:mainnet:abcd123".to_string());
        assert_eq!(p.namespace, "mainnet".to_string());
        assert_eq!(p.id, "abcd123".to_string());
        assert_eq!(p.version, Some("v1".to_string()));
    }

    #[test]
    fn parse_query_params() {
        let s = "did:cheqd:mainnet:abcd123?resourceName=foo&resourceType=bar&foo=bar";
        let p = DidCheqdParser::parse(s).unwrap();
        assert_eq!(p.namespace, "mainnet".to_string());
        assert_eq!(p.id, "abcd123".to_string());
        let q = p.query.unwrap();
        assert_eq!(q.get("resourceName").map(String::as_str), Some("foo"));
        assert_eq!(q.get("resourceType").map(String::as_str), Some("bar"));
        assert_eq!(q.get("foo").map(String::as_str), Some("bar"));
    }

    #[test]
    fn parse_without_namespace_but_with_query() {
        let s = "did:cheqd:abcd123?resourceName=foo";
        let p = DidCheqdParser::parse(s).unwrap();
        assert_eq!(p.namespace, "mainnet".to_string());
        assert_eq!(p.id, "abcd123".to_string());
        let q = p.query.unwrap();
        assert_eq!(q.get("resourceName").map(String::as_str), Some("foo"));
    }

    #[test]
    fn parse_malformed_not_cheqd() {
        let s = "did:xyz:abc";
        let e = DidCheqdParser::parse(s).unwrap_err();
        let es = e.to_string();
        assert!(es.contains("not a did:cheqd"));
    }

    #[test]
    fn parse_version_from_query() {
        let s = "did:cheqd:mainnet:abcd123?resourceName=foo&versionId=v42";
        let p = DidCheqdParser::parse(s).unwrap();
        assert_eq!(p.version, Some("v42".to_string()));
        let q = p.query.unwrap();
        // versionId remains present in the query map
        assert_eq!(q.get("versionId").map(String::as_str), Some("v42"));
    }

    #[test]
    fn parse_invalid_path_param() {
        let s = "did:cheqd:mainnet:f5a28137-5cfa-486f-bf88-3fbe6507eac5/invalid/r1";
        let e = DidCheqdParser::parse(s).unwrap_err();
        let es = e.to_string();
        assert!(es.contains("unsupported path segment"));
    }
}
