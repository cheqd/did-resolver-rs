use chrono::{DateTime, Utc};
use serde_json::{Value, json};

use crate::{
    error::{DidCheqdError, DidCheqdResult},
    proto::cheqd::{
        did::v2::{DidDoc as CheqdDidDoc, Metadata as CheqdDidDocMetadata},
        resource::v2::Metadata as CheqdResourceMetadata,
    },
};

/// Convert a CheqdDidDoc proto message into a serde_json::Value representing a W3C DID Document.
/// This avoids depending on external DID Document types and produces a JSON structure that can be
/// serialized into bytes for the ssi_dids_core `Output<Vec<u8>>` path.
pub fn cheqd_diddoc_to_json(value: CheqdDidDoc) -> Result<Value, DidCheqdError> {
    let mut context = value.context;

    // ensure default contexts present
    if !context
        .iter()
        .any(|c| c == "https://www.w3.org/ns/did/v1" || c == "https://w3id.org/did/v1")
    {
        context.push("https://www.w3.org/ns/did/v1".to_string());
    }

    let mut doc = json!({
        "id": value.id,
        "@context": context,
    });

    // controller
    if !value.controller.is_empty() {
        let controllers: Vec<Value> = value.controller.into_iter().map(Value::String).collect();
        doc["controller"] = Value::Array(controllers);
    }

    // verificationMethod
    if !value.verification_method.is_empty() {
        let vms: Vec<Value> = value
            .verification_method
            .into_iter()
            .map(|vm| {
                let mut o = serde_json::Map::new();
                o.insert("id".to_string(), Value::String(vm.id));
                o.insert(
                    "type".to_string(),
                    Value::String(vm.verification_method_type),
                );
                o.insert("controller".to_string(), Value::String(vm.controller));
                // try to parse verification material as JSON, otherwise keep as string
                let material = match serde_json::from_str::<Value>(&vm.verification_material) {
                    Ok(v) => v,
                    Err(_) => Value::String(vm.verification_material),
                };
                o.insert("publicKey".to_string(), material);
                Value::Object(o)
            })
            .collect();
        doc["verificationMethod"] = Value::Array(vms);
    }

    // simple arrays: authentication, assertionMethod, capabilityInvocation, capabilityDelegation, keyAgreement
    if !value.authentication.is_empty() {
        doc["authentication"] = Value::Array(
            value
                .authentication
                .into_iter()
                .map(Value::String)
                .collect(),
        );
    }
    if !value.assertion_method.is_empty() {
        // assertionMethod may contain JSON objects or strings; try to parse
        let arr: Vec<Value> = value
            .assertion_method
            .into_iter()
            .map(|s| match serde_json::from_str::<Value>(&s) {
                Ok(v) => v,
                Err(_) => Value::String(s),
            })
            .collect();
        doc["assertionMethod"] = Value::Array(arr);
    }
    if !value.capability_invocation.is_empty() {
        doc["capabilityInvocation"] = Value::Array(
            value
                .capability_invocation
                .into_iter()
                .map(Value::String)
                .collect(),
        );
    }
    if !value.capability_delegation.is_empty() {
        doc["capabilityDelegation"] = Value::Array(
            value
                .capability_delegation
                .into_iter()
                .map(Value::String)
                .collect(),
        );
    }
    if !value.key_agreement.is_empty() {
        doc["keyAgreement"] =
            Value::Array(value.key_agreement.into_iter().map(Value::String).collect());
    }

    if !value.service.is_empty() {
        let services: Vec<Value> = value
            .service
            .into_iter()
            .map(|svc| {
                let mut o = serde_json::Map::new();

                // required fields
                o.insert("id".to_string(), Value::String(svc.id));
                o.insert(
                    "type".to_string(),
                    serde_json::from_value(json!(svc.service_type))
                        .unwrap_or(Value::String(svc.service_type)),
                );

                // serviceEndpoint (single or multiple)
                if !svc.service_endpoint.is_empty() {
                    if svc.service_endpoint.len() == 1 {
                        o.insert(
                            "serviceEndpoint".to_string(),
                            Value::String(svc.service_endpoint[0].clone()),
                        );
                    } else {
                        o.insert(
                            "serviceEndpoint".to_string(),
                            Value::Array(
                                svc.service_endpoint
                                    .into_iter()
                                    .map(Value::String)
                                    .collect(),
                            ),
                        );
                    }
                }

                // recipientKeys
                if !svc.recipient_keys.is_empty() {
                    o.insert(
                        "recipientKeys".to_string(),
                        Value::Array(svc.recipient_keys.into_iter().map(Value::String).collect()),
                    );
                }

                // routingKeys
                if !svc.routing_keys.is_empty() {
                    o.insert(
                        "routingKeys".to_string(),
                        Value::Array(svc.routing_keys.into_iter().map(Value::String).collect()),
                    );
                }

                // accept
                if !svc.accept.is_empty() {
                    o.insert(
                        "accept".to_string(),
                        Value::Array(svc.accept.into_iter().map(Value::String).collect()),
                    );
                }

                // priority
                if svc.priority != 0 {
                    o.insert("priority".to_string(), Value::Number(svc.priority.into()));
                }

                Value::Object(o)
            })
            .collect();

        doc["service"] = Value::Array(services);
    }

    // alsoKnownAs
    if !value.also_known_as.is_empty() {
        doc["alsoKnownAs"] =
            Value::Array(value.also_known_as.into_iter().map(Value::String).collect());
    }

    Ok(doc)
}

// Note: We no longer map verification methods into external VerificationMethod types.
// Instead, verification methods are incorporated into the JSON DID Document produced by
// `cheqd_diddoc_to_json` above. The previous, more detailed mapping is intentionally omitted
// to avoid depending on the external did_resolver crate.

// Service mapping removed; services are represented directly in the JSON produced earlier.

/// Convert CheqdDidDocMetadata into a JSON object with common metadata fields.
pub fn cheqd_diddoc_metadata_to_json(value: CheqdDidDocMetadata) -> Result<Value, DidCheqdError> {
    let mut obj = serde_json::Map::new();
    if let Some(timestamp) = value.created {
        obj.insert(
            "created".to_string(),
            Value::String(prost_timestamp_to_dt(timestamp)?.to_rfc3339()),
        );
    }
    if let Some(timestamp) = value.updated {
        obj.insert(
            "updated".to_string(),
            Value::String(prost_timestamp_to_dt(timestamp)?.to_rfc3339()),
        );
    }
    obj.insert("deactivated".to_string(), Value::Bool(value.deactivated));
    if !value.version_id.is_empty() {
        obj.insert("versionId".to_string(), Value::String(value.version_id));
    }
    if !value.next_version_id.is_empty() {
        obj.insert(
            "nextVersionId".to_string(),
            Value::String(value.next_version_id),
        );
    }
    Ok(Value::Object(obj))
}

pub struct CheqdResourceMetadataWithUri {
    pub uri: String,
    pub meta: CheqdResourceMetadata,
}

pub fn cheqd_resource_metadata_with_uri_to_json(
    value: CheqdResourceMetadataWithUri,
) -> Result<Value, DidCheqdError> {
    let uri = value.uri;
    let value = value.meta;

    let created = value
        .created
        .ok_or(DidCheqdError::InvalidDidDocument(format!(
            "created field missing from resource: {value:?}"
        )))?;

    let mut obj = serde_json::Map::new();
    obj.insert("uri".to_string(), Value::String(uri));
    obj.insert(
        "collectionId".to_string(),
        Value::String(value.collection_id),
    );
    obj.insert("id".to_string(), Value::String(value.id));
    obj.insert("name".to_string(), Value::String(value.name));
    obj.insert("type".to_string(), Value::String(value.resource_type));
    if !value.version.is_empty() {
        obj.insert("version".to_string(), Value::String(value.version));
    }
    obj.insert("mediaType".to_string(), Value::String(value.media_type));
    obj.insert(
        "created".to_string(),
        Value::String(prost_timestamp_to_dt(created)?.to_rfc3339()),
    );
    if !value.checksum.is_empty() {
        obj.insert("checksum".to_string(), Value::String(value.checksum));
    }

    Ok(Value::Object(obj))
}

fn prost_timestamp_to_dt(mut timestamp: prost_types::Timestamp) -> DidCheqdResult<DateTime<Utc>> {
    timestamp.normalize();
    DateTime::from_timestamp(timestamp.seconds, timestamp.nanos.try_into()?).ok_or(
        DidCheqdError::Other(format!("Unknown error, bad timestamp: {timestamp:?}").into()),
    )
}
