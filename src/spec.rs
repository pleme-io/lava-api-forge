//! Typed input specs lava-api-forge consumes.
//!
//! Two upstream shapes:
//!   - **OpenAPI 3.x** — the modern public-API standard (Azure,
//!     Cloudflare API v4, Stripe, Kubernetes, GitHub, etc.).
//!   - **AWS botocore service-2.json** — the per-service spec
//!     `botocore` ships (every AWS service has one); the same spec
//!     the official AWS SDKs are generated from.
//!
//! Both are parsed into a common [`Service`] shape — one
//! representation, two adapters.

use indexmap::IndexMap;
use serde::{Deserialize, Serialize};

/// The canonical typed representation lava-api-forge emits from.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Service {
    pub name: String,
    pub version: String,
    pub doc: Option<String>,
    /// Ordered by source-file position for deterministic emission.
    pub operations: IndexMap<String, Operation>,
    pub shapes: IndexMap<String, Shape>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Operation {
    pub name: String,
    pub method: Method,
    pub path: String,
    pub doc: Option<String>,
    pub input_shape: Option<String>,
    pub output_shape: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum Method {
    Get,
    Post,
    Put,
    Patch,
    Delete,
    Head,
    Options,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Shape {
    pub kind: ShapeKind,
    pub doc: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ShapeKind {
    /// Primitive: `string`, `integer`, `boolean`, `float`, `timestamp`.
    Primitive(String),
    /// Object / structure / record. Members keyed by name.
    Object {
        members: IndexMap<String, ShapeMember>,
        required: Vec<String>,
    },
    /// List of one shape.
    List { item: String },
    /// Map of string keys to one shape.
    Map { value: String },
    /// String enum.
    Enum { values: Vec<String> },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShapeMember {
    pub shape: String,
    pub doc: Option<String>,
    pub sensitive: bool,
}

// ── Botocore adapter ──────────────────────────────────────────────────

/// Parse an AWS botocore service-2.json into the canonical Service.
pub fn from_botocore(json: &str) -> Result<Service, ParseError> {
    let v: serde_json::Value = serde_json::from_str(json)?;
    let metadata = v.get("metadata").ok_or(ParseError::ShapeMissing("metadata"))?;
    let name = metadata
        .get("serviceId")
        .or_else(|| metadata.get("serviceFullName"))
        .or_else(|| metadata.get("endpointPrefix"))
        .and_then(|x| x.as_str())
        .unwrap_or("unknown")
        .to_string();
    let version = metadata
        .get("apiVersion")
        .and_then(|x| x.as_str())
        .unwrap_or("unknown")
        .to_string();
    let mut svc = Service {
        name,
        version,
        doc: v.get("documentation").and_then(|x| x.as_str()).map(String::from),
        ..Default::default()
    };

    // Shapes
    if let Some(shapes) = v.get("shapes").and_then(|x| x.as_object()) {
        for (sname, sval) in shapes {
            let doc = sval.get("documentation").and_then(|x| x.as_str()).map(String::from);
            let kind = match sval.get("type").and_then(|x| x.as_str()).unwrap_or("structure") {
                "structure" => {
                    let members = sval.get("members").and_then(|x| x.as_object()).cloned().unwrap_or_default();
                    let mut map = IndexMap::new();
                    for (mname, mval) in members.iter() {
                        let shape = mval.get("shape").and_then(|x| x.as_str()).unwrap_or("String").to_string();
                        let mdoc = mval.get("documentation").and_then(|x| x.as_str()).map(String::from);
                        let sensitive = mval.get("sensitive").and_then(|x| x.as_bool()).unwrap_or(false);
                        map.insert(mname.clone(), ShapeMember { shape, doc: mdoc, sensitive });
                    }
                    let required = sval
                        .get("required")
                        .and_then(|x| x.as_array())
                        .map(|a| a.iter().filter_map(|v| v.as_str().map(String::from)).collect())
                        .unwrap_or_default();
                    ShapeKind::Object { members: map, required }
                }
                "list" => {
                    let item = sval
                        .get("member")
                        .and_then(|m| m.get("shape"))
                        .and_then(|x| x.as_str())
                        .unwrap_or("String")
                        .to_string();
                    ShapeKind::List { item }
                }
                "map" => {
                    let value = sval
                        .get("value")
                        .and_then(|m| m.get("shape"))
                        .and_then(|x| x.as_str())
                        .unwrap_or("String")
                        .to_string();
                    ShapeKind::Map { value }
                }
                "enum" | "string" if sval.get("enum").is_some() => {
                    let values = sval
                        .get("enum")
                        .and_then(|x| x.as_array())
                        .map(|a| a.iter().filter_map(|v| v.as_str().map(String::from)).collect())
                        .unwrap_or_default();
                    ShapeKind::Enum { values }
                }
                other => ShapeKind::Primitive(other.to_string()),
            };
            svc.shapes.insert(sname.clone(), Shape { kind, doc });
        }
    }

    // Operations
    if let Some(ops) = v.get("operations").and_then(|x| x.as_object()) {
        for (oname, oval) in ops {
            let http = oval.get("http").cloned().unwrap_or(serde_json::Value::Null);
            let method_str = http.get("method").and_then(|x| x.as_str()).unwrap_or("POST");
            let method = method_from_str(method_str);
            let path = http
                .get("requestUri")
                .and_then(|x| x.as_str())
                .unwrap_or("/")
                .to_string();
            let input_shape = oval
                .get("input")
                .and_then(|i| i.get("shape"))
                .and_then(|x| x.as_str())
                .map(String::from);
            let output_shape = oval
                .get("output")
                .and_then(|o| o.get("shape"))
                .and_then(|x| x.as_str())
                .map(String::from);
            let doc = oval.get("documentation").and_then(|x| x.as_str()).map(String::from);
            svc.operations.insert(
                oname.clone(),
                Operation {
                    name: oname.clone(),
                    method,
                    path,
                    doc,
                    input_shape,
                    output_shape,
                },
            );
        }
    }

    Ok(svc)
}

// ── OpenAPI adapter ───────────────────────────────────────────────────

/// Parse an OpenAPI 3.x document (YAML or JSON) into the canonical
/// Service. Operations keyed by `operationId` (falls back to
/// `${method}_${path}` when absent).
pub fn from_openapi(text: &str) -> Result<Service, ParseError> {
    let v: serde_yaml::Value = serde_yaml::from_str(text)?;
    let info = &v["info"];
    let name = info["title"].as_str().unwrap_or("unknown").to_string();
    let version = info["version"].as_str().unwrap_or("unknown").to_string();
    let mut svc = Service {
        name,
        version,
        doc: info["description"].as_str().map(String::from),
        ..Default::default()
    };

    // Components/schemas → Shapes (best-effort).
    if let Some(schemas) = v["components"]["schemas"].as_mapping() {
        for (sname, sval) in schemas {
            let sname = sname.as_str().unwrap_or("").to_string();
            if sname.is_empty() {
                continue;
            }
            let kind = openapi_shape_kind(sval);
            svc.shapes.insert(
                sname,
                Shape {
                    kind,
                    doc: sval["description"].as_str().map(String::from),
                },
            );
        }
    }

    // paths.<path>.<method> → Operations.
    if let Some(paths) = v["paths"].as_mapping() {
        for (path_val, item) in paths {
            let path = path_val.as_str().unwrap_or("").to_string();
            if path.is_empty() {
                continue;
            }
            let item_map = match item.as_mapping() {
                Some(m) => m,
                None => continue,
            };
            for (method_val, op_val) in item_map {
                let method_str = match method_val.as_str() {
                    Some(s) => s.to_uppercase(),
                    None => continue,
                };
                if !matches!(
                    method_str.as_str(),
                    "GET" | "POST" | "PUT" | "PATCH" | "DELETE" | "HEAD" | "OPTIONS"
                ) {
                    continue;
                }
                let method = method_from_str(&method_str);
                let operation_id = op_val["operationId"]
                    .as_str()
                    .map(String::from)
                    .unwrap_or_else(|| format!("{}_{}", method_str.to_lowercase(), path.replace('/', "_")));
                svc.operations.insert(
                    operation_id.clone(),
                    Operation {
                        name: operation_id,
                        method,
                        path: path.clone(),
                        doc: op_val["description"]
                            .as_str()
                            .or_else(|| op_val["summary"].as_str())
                            .map(String::from),
                        input_shape: None,
                        output_shape: None,
                    },
                );
            }
        }
    }

    Ok(svc)
}

fn openapi_shape_kind(v: &serde_yaml::Value) -> ShapeKind {
    let t = v["type"].as_str().unwrap_or("string");
    match t {
        "object" => {
            let mut members = IndexMap::new();
            if let Some(props) = v["properties"].as_mapping() {
                for (k, vv) in props {
                    let k = k.as_str().unwrap_or("").to_string();
                    let shape = vv["$ref"]
                        .as_str()
                        .and_then(|r| r.rsplit('/').next())
                        .or_else(|| vv["type"].as_str())
                        .unwrap_or("string")
                        .to_string();
                    members.insert(
                        k,
                        ShapeMember {
                            shape,
                            doc: vv["description"].as_str().map(String::from),
                            sensitive: false,
                        },
                    );
                }
            }
            let required = v["required"]
                .as_sequence()
                .map(|a| a.iter().filter_map(|x| x.as_str().map(String::from)).collect())
                .unwrap_or_default();
            ShapeKind::Object { members, required }
        }
        "array" => {
            let item = v["items"]["$ref"]
                .as_str()
                .and_then(|r| r.rsplit('/').next())
                .or_else(|| v["items"]["type"].as_str())
                .unwrap_or("string")
                .to_string();
            ShapeKind::List { item }
        }
        "string" if v["enum"].as_sequence().is_some() => {
            let values = v["enum"]
                .as_sequence()
                .unwrap()
                .iter()
                .filter_map(|x| x.as_str().map(String::from))
                .collect();
            ShapeKind::Enum { values }
        }
        other => ShapeKind::Primitive(other.to_string()),
    }
}

fn method_from_str(s: &str) -> Method {
    match s.to_uppercase().as_str() {
        "GET" => Method::Get,
        "POST" => Method::Post,
        "PUT" => Method::Put,
        "PATCH" => Method::Patch,
        "DELETE" => Method::Delete,
        "HEAD" => Method::Head,
        "OPTIONS" => Method::Options,
        _ => Method::Post,
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ParseError {
    #[error("json: {0}")]
    Json(#[from] serde_json::Error),
    #[error("yaml: {0}")]
    Yaml(#[from] serde_yaml::Error),
    #[error("missing required field: {0}")]
    ShapeMissing(&'static str),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_minimal_botocore() {
        let json = r#"{
          "metadata": { "serviceId": "EC2", "apiVersion": "2016-11-15" },
          "shapes": {
            "CreateVpcRequest": {
              "type": "structure",
              "documentation": "Input for CreateVpc.",
              "required": ["CidrBlock"],
              "members": {
                "CidrBlock": { "shape": "String", "documentation": "CIDR." },
                "InstanceTenancy": { "shape": "Tenancy" }
              }
            },
            "String": { "type": "string" },
            "Tenancy": { "type": "string", "enum": ["default", "dedicated", "host"] }
          },
          "operations": {
            "CreateVpc": {
              "http": { "method": "POST", "requestUri": "/" },
              "input": { "shape": "CreateVpcRequest" },
              "output": { "shape": "CreateVpcResult" },
              "documentation": "Creates a VPC with the specified IPv4 CIDR block."
            }
          }
        }"#;
        let svc = from_botocore(json).unwrap();
        assert_eq!(svc.name, "EC2");
        assert_eq!(svc.version, "2016-11-15");
        let op = svc.operations.get("CreateVpc").unwrap();
        assert_eq!(op.method, Method::Post);
        assert_eq!(op.input_shape.as_deref(), Some("CreateVpcRequest"));
        let req = svc.shapes.get("CreateVpcRequest").unwrap();
        match &req.kind {
            ShapeKind::Object { members, required } => {
                assert!(members.contains_key("CidrBlock"));
                assert_eq!(required, &vec!["CidrBlock".to_string()]);
            }
            _ => panic!("expected object shape"),
        }
        let tenancy = svc.shapes.get("Tenancy").unwrap();
        assert!(matches!(&tenancy.kind, ShapeKind::Enum { values } if values.len() == 3));
    }
}
