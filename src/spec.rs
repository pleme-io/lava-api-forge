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
//!
//! ## Smithy vocabulary
//!
//! botocore `service-2.json` is the *older serialization of AWS's Smithy
//! models* — the same models `smithy-rs` generates `aws-sdk-rust` from.
//! Every wire-metadata type below therefore takes **Smithy's** names for
//! its variants and its emitted tokens (`awsQuery`, `restJson1`,
//! `awsJson1_1`, `restXml`, `ec2Query`, `sigv4`) rather than botocore's
//! (`query`, `rest-json`, `json`, `rest-xml`, `ec2`, `v4`). It costs
//! nothing today and is free compatibility if a Smithy front end ever
//! lands. The botocore token each value was parsed from is preserved
//! alongside it, so nothing is lost in the translation.
//!
//! This is *not* a Smithy ingester and does not pretend to be one.

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
    /// AWS wire + auth metadata. `Some` for botocore inputs, `None` for
    /// OpenAPI inputs (which carry no AWS protocol).
    pub aws: Option<AwsMetadata>,
}

/// The service-level wire and auth facts a client needs to build and
/// sign a request. Everything here was previously read and discarded.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AwsMetadata {
    /// `metadata.protocol` (+ `metadata.jsonVersion`).
    pub protocol: AwsProtocol,
    /// `metadata.signatureVersion`.
    pub auth: AwsAuth,
    /// `metadata.endpointPrefix` — the host label, e.g. `sts` in
    /// `sts.us-east-1.amazonaws.com`.
    pub endpoint_prefix: String,
    /// `metadata.signingName` — the service name in the SigV4 credential
    /// scope, when it differs from `endpoint_prefix`. Present on 221 of
    /// the 349 models surveyed 2026-08-02.
    pub signing_name: Option<String>,
    /// `metadata.globalEndpoint` — a fixed, non-regional host.
    /// 13 of 349.
    pub global_endpoint: Option<String>,
    /// `metadata.targetPrefix` — the `X-Amz-Target` header prefix that
    /// selects the operation under the `awsJson*` protocols. 129 of 349,
    /// exactly the `awsJson*` services.
    pub target_prefix: Option<String>,
    /// `metadata.xmlNamespace` — the request/response XML namespace under
    /// `awsQuery` / `restXml` / `ec2Query`. 25 of 349.
    pub xml_namespace: Option<String>,
}

/// AWS wire protocol, closed over the five tokens botocore emits.
///
/// Spread over the 349 models surveyed on disk 2026-08-02:
/// `rest-json` 185 · `json` 129 · `query` 24 · `rest-xml` 10 · `ec2` 1.
///
/// There is deliberately **no** `Other(String)` escape hatch. An
/// unrecognized protocol is refused at the parse boundary
/// ([`ParseError::UnknownProtocol`]), so no value downstream of parsing
/// can hold an unknown protocol — the wrong-protocol state has no code
/// path past `from_botocore`. This matters because a wrong protocol
/// produces a request that fails *at AWS*, long after generation, rather
/// than at the point the mistake was made.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AwsProtocol {
    /// botocore `query` → Smithy `aws.protocols#awsQuery`.
    AwsQuery,
    /// botocore `json` → Smithy `aws.protocols#awsJson1_0` or
    /// `#awsJson1_1`, selected by `metadata.jsonVersion`. Flattening the
    /// two would mis-generate the 23 of 129 `json` services on 1.0.
    AwsJson(JsonVersion),
    /// botocore `rest-json` → Smithy `aws.protocols#restJson1`.
    RestJson1,
    /// botocore `rest-xml` → Smithy `aws.protocols#restXml`.
    RestXml,
    /// botocore `ec2` → Smithy `aws.protocols#ec2Query`.
    Ec2Query,
}

/// The `awsJson` wire version, from `metadata.jsonVersion`.
/// 1.1 on 106 of 129 `json` services, 1.0 on 23.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum JsonVersion {
    /// `metadata.jsonVersion == "1.0"`.
    V1_0,
    /// `metadata.jsonVersion == "1.1"`.
    V1_1,
}

impl AwsProtocol {
    /// The Smithy protocol-trait short name. This is the token emitted
    /// into the `.tlisp`.
    #[must_use]
    pub fn smithy_trait(self) -> &'static str {
        match self {
            Self::AwsQuery => "awsQuery",
            Self::AwsJson(JsonVersion::V1_0) => "awsJson1_0",
            Self::AwsJson(JsonVersion::V1_1) => "awsJson1_1",
            Self::RestJson1 => "restJson1",
            Self::RestXml => "restXml",
            Self::Ec2Query => "ec2Query",
        }
    }

    /// The `metadata.protocol` token this was parsed from.
    #[must_use]
    pub fn botocore_token(self) -> &'static str {
        match self {
            Self::AwsQuery => "query",
            Self::AwsJson(_) => "json",
            Self::RestJson1 => "rest-json",
            Self::RestXml => "rest-xml",
            Self::Ec2Query => "ec2",
        }
    }
}

/// AWS request-signing scheme, closed over the five `signatureVersion`
/// tokens botocore emits.
///
/// Counts over the 349 models surveyed 2026-08-02: `v4` 344 · `v2` 2 ·
/// `s3` 1 · `s3v4` 1 · `bearer` 1. As with [`AwsProtocol`] there is no
/// `Other` variant; an unknown token is [`ParseError::UnknownSignatureVersion`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AwsAuth {
    /// botocore `v4` → Smithy `aws.auth#sigv4`.
    SigV4,
    /// botocore `s3v4` → Smithy `aws.auth#sigv4` with S3's payload-signing
    /// rules. Smithy has no distinct trait for it; the distinction
    /// survives in [`AwsAuth::botocore_token`].
    SigV4S3,
    /// botocore `s3` — the pre-SigV4 S3 signer. No Smithy trait.
    S3Legacy,
    /// botocore `v2` — legacy SigV2. No Smithy trait.
    SigV2,
    /// botocore `bearer` → Smithy `smithy.api#httpBearerAuth`.
    Bearer,
}

impl AwsAuth {
    /// The Smithy auth-trait short name, when Smithy models one.
    ///
    /// `None` means pleme-io has no signer for this scheme. It is emitted
    /// as the symbol `unsupported` so a generated client refuses rather
    /// than quietly falling back to an unsigned request.
    #[must_use]
    pub fn smithy_scheme(self) -> Option<&'static str> {
        match self {
            Self::SigV4 | Self::SigV4S3 => Some("sigv4"),
            Self::Bearer => Some("httpBearerAuth"),
            Self::S3Legacy | Self::SigV2 => None,
        }
    }

    /// The `metadata.signatureVersion` token this was parsed from.
    #[must_use]
    pub fn botocore_token(self) -> &'static str {
        match self {
            Self::SigV4 => "v4",
            Self::SigV4S3 => "s3v4",
            Self::S3Legacy => "s3",
            Self::SigV2 => "v2",
            Self::Bearer => "bearer",
        }
    }
}

/// A per-operation override of the service signing posture
/// (`operations.<Op>.authtype`).
///
/// Real and load-bearing: over the 349 models surveyed 2026-08-02, 26
/// operations declare `none` and 8 declare `v4-unsigned-body`. Signing an
/// operation that must not be signed fails at AWS.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AuthOverride {
    /// botocore `none` → Smithy `smithy.api#noAuth`. Send unsigned.
    NoAuth,
    /// botocore `v4-unsigned-body` → Smithy `aws.auth#unsignedPayload`.
    /// SigV4 headers, payload hash literal `UNSIGNED-PAYLOAD`.
    UnsignedPayload,
}

impl AuthOverride {
    /// The Smithy auth-trait short name. Emitted into the `.tlisp`.
    #[must_use]
    pub fn smithy_scheme(self) -> &'static str {
        match self {
            Self::NoAuth => "noAuth",
            Self::UnsignedPayload => "unsignedPayload",
        }
    }

    /// The `authtype` token this was parsed from.
    #[must_use]
    pub fn botocore_token(self) -> &'static str {
        match self {
            Self::NoAuth => "none",
            Self::UnsignedPayload => "v4-unsigned-body",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Operation {
    pub name: String,
    pub method: Method,
    pub path: String,
    pub doc: Option<String>,
    pub input_shape: Option<String>,
    pub output_shape: Option<String>,
    /// `output.resultWrapper` — the XML element the real result is nested
    /// inside under `awsQuery` / `ec2Query`. Present on 1081 operations
    /// across the 349 models surveyed 2026-08-02. Without it a query-protocol
    /// response cannot be unwrapped.
    pub result_wrapper: Option<String>,
    /// `errors[].shape` — the modeled error shapes this operation can
    /// return.
    pub errors: Vec<String>,
    /// `authtype` — overrides the service-level [`AwsMetadata::auth`].
    pub auth_override: Option<AuthOverride>,
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
///
/// # Errors
///
/// Refuses, rather than guessing, when the model declares a protocol,
/// JSON version, signature version or operation `authtype` this crate
/// does not know, or when `metadata` / `metadata.protocol` /
/// `metadata.signatureVersion` are absent. Guessing any of these produces
/// a client whose requests fail at AWS.
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
        aws: Some(parse_aws_metadata(metadata)?),
        ..Default::default()
    };

    // Declared shapes.
    if let Some(shapes) = v.get("shapes").and_then(|x| x.as_object()) {
        for (sname, sval) in shapes {
            let doc = sval.get("documentation").and_then(|x| x.as_str()).map(String::from);
            let kind = parse_shape_kind(sval, sname, &mut svc.shapes)?;
            svc.shapes.insert(sname.clone(), Shape { kind, doc });
        }
    }

    // Operations.
    if let Some(ops) = v.get("operations").and_then(|x| x.as_object()) {
        for (oname, oval) in ops {
            let http = oval.get("http");
            let method_str = http
                .and_then(|h| h.get("method"))
                .and_then(|x| x.as_str())
                .unwrap_or("POST");
            let method = method_from_str(method_str);
            let path = http
                .and_then(|h| h.get("requestUri"))
                .and_then(|x| x.as_str())
                .unwrap_or("/")
                .to_string();
            let input_shape =
                resolve_io_shape(oname, oval.get("input"), IoRole::Input, &mut svc.shapes)?;
            let output_shape =
                resolve_io_shape(oname, oval.get("output"), IoRole::Output, &mut svc.shapes)?;
            let result_wrapper = oval
                .get("output")
                .and_then(|o| o.get("resultWrapper"))
                .and_then(|x| x.as_str())
                .map(String::from);
            let errors = oval
                .get("errors")
                .and_then(|x| x.as_array())
                .map(|a| {
                    a.iter()
                        .filter_map(|e| e.get("shape").and_then(|x| x.as_str()).map(String::from))
                        .collect()
                })
                .unwrap_or_default();
            let auth_override = match oval.get("authtype").and_then(|x| x.as_str()) {
                None => None,
                Some("none") => Some(AuthOverride::NoAuth),
                Some("v4-unsigned-body") => Some(AuthOverride::UnsignedPayload),
                Some(other) => return Err(ParseError::UnknownAuthType(other.to_string())),
            };
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
                    result_wrapper,
                    errors,
                    auth_override,
                },
            );
        }
    }

    Ok(svc)
}

/// Read the service-level wire + auth facts out of `metadata`.
fn parse_aws_metadata(metadata: &serde_json::Value) -> Result<AwsMetadata, ParseError> {
    let protocol = parse_protocol(metadata)?;
    let auth = match metadata
        .get("signatureVersion")
        .and_then(|x| x.as_str())
        .ok_or(ParseError::ShapeMissing("metadata.signatureVersion"))?
    {
        "v4" => AwsAuth::SigV4,
        "s3v4" => AwsAuth::SigV4S3,
        "s3" => AwsAuth::S3Legacy,
        "v2" => AwsAuth::SigV2,
        "bearer" => AwsAuth::Bearer,
        other => return Err(ParseError::UnknownSignatureVersion(other.to_string())),
    };
    let field = |k: &str| metadata.get(k).and_then(|x| x.as_str()).map(String::from);
    Ok(AwsMetadata {
        protocol,
        auth,
        endpoint_prefix: field("endpointPrefix").unwrap_or_default(),
        signing_name: field("signingName"),
        global_endpoint: field("globalEndpoint"),
        target_prefix: field("targetPrefix"),
        xml_namespace: field("xmlNamespace"),
    })
}

fn parse_protocol(metadata: &serde_json::Value) -> Result<AwsProtocol, ParseError> {
    let raw = metadata
        .get("protocol")
        .and_then(|x| x.as_str())
        .ok_or(ParseError::ShapeMissing("metadata.protocol"))?;
    Ok(match raw {
        "query" => AwsProtocol::AwsQuery,
        "json" => AwsProtocol::AwsJson(parse_json_version(metadata)?),
        "rest-json" => AwsProtocol::RestJson1,
        "rest-xml" => AwsProtocol::RestXml,
        "ec2" => AwsProtocol::Ec2Query,
        other => return Err(ParseError::UnknownProtocol(other.to_string())),
    })
}

fn parse_json_version(metadata: &serde_json::Value) -> Result<JsonVersion, ParseError> {
    match metadata.get("jsonVersion").and_then(|x| x.as_str()) {
        Some("1.0") => Ok(JsonVersion::V1_0),
        Some("1.1") => Ok(JsonVersion::V1_1),
        Some(other) => Err(ParseError::UnknownJsonVersion(other.to_string())),
        None => Err(ParseError::ShapeMissing("metadata.jsonVersion")),
    }
}

// ── Shape resolution (both botocore serializations) ───────────────────

/// Which side of an operation an inline structure was found on.
#[derive(Clone, Copy)]
enum IoRole {
    Input,
    Output,
}

impl IoRole {
    fn suffix(self) -> &'static str {
        match self {
            Self::Input => "Request",
            Self::Output => "Response",
        }
    }
}

/// Resolve an operation's `input` / `output` node to a shape name.
///
/// botocore ships the same models in two serializations:
///
/// - canonical `service-2.json` — `{"shape": "GetCallerIdentityRequest"}`,
///   a reference into the top-level `shapes` map;
/// - the minified `*.min.json` the JS SDK ships — the structure written
///   **inline** on the operation, with the top-level `shapes` map holding
///   only deduplicated `S<n>` entries.
///
/// Reading only `.shape` handles the first and silently drops the second.
/// Measured over the 349 minified models on disk 2026-08-02, 13340 of
/// 13369 operation inputs and 11853 of 12178 outputs are inline — so the
/// `.shape`-only read recovered the I/O type of roughly 0.2% of
/// operations. Inline structures are synthesized into named shapes here.
fn resolve_io_shape(
    op_name: &str,
    node: Option<&serde_json::Value>,
    role: IoRole,
    shapes: &mut IndexMap<String, Shape>,
) -> Result<Option<String>, ParseError> {
    let Some(node) = node else { return Ok(None) };
    if let Some(name) = node.get("shape").and_then(|x| x.as_str()) {
        return Ok(Some(name.to_string()));
    }
    let name = synthetic_name(op_name, role.suffix());
    let kind = parse_shape_kind(node, &name, shapes)?;
    let doc = node.get("documentation").and_then(|x| x.as_str()).map(String::from);
    insert_synthesized(shapes, &name, Shape { kind, doc })?;
    Ok(Some(name))
}

/// Parse one shape node into a [`ShapeKind`], synthesizing named shapes
/// for any inline children. `owner` names the shape being parsed and is
/// the stem for synthesized child names.
fn parse_shape_kind(
    sval: &serde_json::Value,
    owner: &str,
    shapes: &mut IndexMap<String, Shape>,
) -> Result<ShapeKind, ParseError> {
    let ty = sval.get("type").and_then(|x| x.as_str()).unwrap_or("structure");
    if sval.get("enum").is_some() && matches!(ty, "enum" | "string") {
        let values = sval
            .get("enum")
            .and_then(|x| x.as_array())
            .map(|a| a.iter().filter_map(|v| v.as_str().map(String::from)).collect())
            .unwrap_or_default();
        return Ok(ShapeKind::Enum { values });
    }
    Ok(match ty {
        "structure" => {
            let mut map = IndexMap::new();
            if let Some(members) = sval.get("members").and_then(|x| x.as_object()) {
                for (mname, mval) in members {
                    let shape = resolve_member_shape(mval, owner, mname, shapes)?;
                    map.insert(
                        mname.clone(),
                        ShapeMember {
                            shape,
                            doc: mval.get("documentation").and_then(|x| x.as_str()).map(String::from),
                            sensitive: mval.get("sensitive").and_then(|x| x.as_bool()).unwrap_or(false),
                        },
                    );
                }
            }
            let required = sval
                .get("required")
                .and_then(|x| x.as_array())
                .map(|a| a.iter().filter_map(|v| v.as_str().map(String::from)).collect())
                .unwrap_or_default();
            ShapeKind::Object { members: map, required }
        }
        "list" => {
            let item = match sval.get("member") {
                Some(m) => resolve_member_shape(m, owner, "Member", shapes)?,
                None => ensure_primitive(shapes, "string"),
            };
            ShapeKind::List { item }
        }
        "map" => {
            let value = match sval.get("value") {
                Some(m) => resolve_member_shape(m, owner, "Value", shapes)?,
                None => ensure_primitive(shapes, "string"),
            };
            ShapeKind::Map { value }
        }
        other => ShapeKind::Primitive(other.to_string()),
    })
}

/// Resolve one structure member (or list element / map value) to a shape
/// name.
///
/// In the minified serialization a member may carry no `shape` at all: a
/// bare `{}` means `string`, and `{"type": …}` writes the member's whole
/// type inline. Both are resolved to a named shape here rather than being
/// defaulted to `String`, which would have emitted a wrong type for every
/// inline non-string member.
fn resolve_member_shape(
    mval: &serde_json::Value,
    owner: &str,
    mname: &str,
    shapes: &mut IndexMap<String, Shape>,
) -> Result<String, ParseError> {
    if let Some(name) = mval.get("shape").and_then(|x| x.as_str()) {
        return Ok(name.to_string());
    }
    let ty = mval.get("type").and_then(|x| x.as_str()).unwrap_or("string");
    let composite = matches!(ty, "structure" | "list" | "map") || mval.get("enum").is_some();
    if !composite {
        return Ok(ensure_primitive(shapes, ty));
    }
    let name = synthetic_name(owner, mname);
    let kind = parse_shape_kind(mval, &name, shapes)?;
    let doc = mval.get("documentation").and_then(|x| x.as_str()).map(String::from);
    insert_synthesized(shapes, &name, Shape { kind, doc })?;
    Ok(name)
}

/// Register (idempotently) the named shape for a botocore primitive type
/// and return its name — `string` → `String`, `integer` → `Integer`.
///
/// The minified serialization writes primitives inline and declares no
/// primitive shapes, so without this every `:shape :string` in the output
/// would dangle.
fn ensure_primitive(shapes: &mut IndexMap<String, Shape>, ty: &str) -> String {
    let name = capitalize(ty);
    if !shapes.contains_key(&name) {
        shapes.insert(
            name.clone(),
            Shape {
                kind: ShapeKind::Primitive(ty.to_string()),
                doc: None,
            },
        );
    }
    name
}

/// Insert a synthesized shape, refusing to silently overwrite a shape the
/// model declared under the same name.
fn insert_synthesized(
    shapes: &mut IndexMap<String, Shape>,
    name: &str,
    shape: Shape,
) -> Result<(), ParseError> {
    if shapes.contains_key(name) {
        return Err(ParseError::SynthesizedShapeCollision(name.to_string()));
    }
    shapes.insert(name.to_string(), shape);
    Ok(())
}

/// `("GetCallerIdentity", "Response")` → `GetCallerIdentityResponse`.
/// Built by concatenation, never `format!()` — the result becomes an
/// emitted keyword.
fn synthetic_name(stem: &str, part: &str) -> String {
    let mut out = String::with_capacity(stem.len() + part.len());
    out.push_str(stem);
    out.push_str(&capitalize(part));
    out
}

fn capitalize(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut chars = s.chars();
    if let Some(first) = chars.next() {
        for c in first.to_uppercase() {
            out.push(c);
        }
    }
    out.push_str(chars.as_str());
    out
}

// ── OpenAPI adapter ───────────────────────────────────────────────────

/// Parse an OpenAPI 3.x document (YAML or JSON) into the canonical
/// Service. Operations keyed by `operationId` (falls back to
/// `${method}_${path}` when absent).
///
/// [`Service::aws`] is `None`: an OpenAPI document carries no AWS wire
/// protocol or signing scheme.
///
/// # Errors
///
/// Returns [`ParseError::Yaml`] if the document is not well-formed.
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
                        result_wrapper: None,
                        errors: Vec::new(),
                        auth_override: None,
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
    #[error("unknown botocore metadata.protocol {0:?} (known: query, json, rest-json, rest-xml, ec2)")]
    UnknownProtocol(String),
    #[error("unknown botocore metadata.jsonVersion {0:?} (known: 1.0, 1.1)")]
    UnknownJsonVersion(String),
    #[error(
        "unknown botocore metadata.signatureVersion {0:?} (known: v4, s3v4, s3, v2, bearer)"
    )]
    UnknownSignatureVersion(String),
    #[error("unknown botocore operation authtype {0:?} (known: none, v4-unsigned-body)")]
    UnknownAuthType(String),
    #[error("synthesized shape name {0:?} collides with a shape the model declares")]
    SynthesizedShapeCollision(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_minimal_botocore() {
        let json = r#"{
          "metadata": {
            "serviceId": "EC2",
            "apiVersion": "2016-11-15",
            "endpointPrefix": "ec2",
            "protocol": "ec2",
            "signatureVersion": "v4"
          },
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

    /// Build a minimal model with the given `metadata` body.
    fn with_metadata(metadata: &str) -> String {
        let mut s = String::from(r#"{"metadata": "#);
        s.push_str(metadata);
        s.push_str(r#", "shapes": {}, "operations": {}}"#);
        s
    }

    #[test]
    fn every_botocore_protocol_maps_to_its_smithy_trait() {
        // The closed set, with the counts measured over the 349 models on
        // disk 2026-08-02: rest-json 185, json 129, query 24, rest-xml 10,
        // ec2 1. A new protocol landing upstream lands here as a refusal.
        let cases = [
            (r#""query""#, "", AwsProtocol::AwsQuery, "awsQuery"),
            (
                r#""json""#,
                r#", "jsonVersion": "1.0""#,
                AwsProtocol::AwsJson(JsonVersion::V1_0),
                "awsJson1_0",
            ),
            (
                r#""json""#,
                r#", "jsonVersion": "1.1""#,
                AwsProtocol::AwsJson(JsonVersion::V1_1),
                "awsJson1_1",
            ),
            (r#""rest-json""#, "", AwsProtocol::RestJson1, "restJson1"),
            (r#""rest-xml""#, "", AwsProtocol::RestXml, "restXml"),
            (r#""ec2""#, "", AwsProtocol::Ec2Query, "ec2Query"),
        ];
        for (proto, extra, expected, smithy) in cases {
            let mut meta = String::from(r#"{"signatureVersion": "v4", "protocol": "#);
            meta.push_str(proto);
            meta.push_str(extra);
            meta.push('}');
            let svc = from_botocore(&with_metadata(&meta))
                .unwrap_or_else(|e| panic!("{proto} should parse: {e}"));
            let aws = svc.aws.expect("aws metadata");
            assert_eq!(aws.protocol, expected, "{proto}");
            assert_eq!(aws.protocol.smithy_trait(), smithy, "{proto}");
        }
    }

    #[test]
    fn every_signature_version_maps_to_its_smithy_scheme() {
        // v4 344, v2 2, s3 1, s3v4 1, bearer 1 over the same 349 models.
        // s3 and v2 have no Smithy trait and no pleme-io signer: they must
        // surface as `None`, never as sigv4.
        let cases = [
            ("v4", AwsAuth::SigV4, Some("sigv4")),
            ("s3v4", AwsAuth::SigV4S3, Some("sigv4")),
            ("bearer", AwsAuth::Bearer, Some("httpBearerAuth")),
            ("s3", AwsAuth::S3Legacy, None),
            ("v2", AwsAuth::SigV2, None),
        ];
        for (token, expected, smithy) in cases {
            let mut meta = String::from(r#"{"protocol": "query", "signatureVersion": ""#);
            meta.push_str(token);
            meta.push_str(r#""}"#);
            let svc = from_botocore(&with_metadata(&meta)).unwrap();
            let aws = svc.aws.expect("aws metadata");
            assert_eq!(aws.auth, expected, "{token}");
            assert_eq!(aws.auth.smithy_scheme(), smithy, "{token}");
            assert_eq!(aws.auth.botocore_token(), token, "{token} round-trips");
        }
    }

    #[test]
    fn unknown_protocol_is_refused_not_passed_through() {
        // A protocol we do not know produces a request that fails at AWS,
        // not at generation — so it is refused at the parse boundary.
        let meta = r#"{"protocol": "smithy-rpc-v2-cbor", "signatureVersion": "v4"}"#;
        let err = from_botocore(&with_metadata(meta)).expect_err("must refuse");
        assert!(
            matches!(&err, ParseError::UnknownProtocol(p) if p == "smithy-rpc-v2-cbor"),
            "got {err:?}"
        );
    }

    #[test]
    fn unknown_signature_version_and_authtype_are_refused() {
        let meta = r#"{"protocol": "query", "signatureVersion": "v9"}"#;
        assert!(matches!(
            from_botocore(&with_metadata(meta)).expect_err("must refuse"),
            ParseError::UnknownSignatureVersion(_)
        ));

        let json = r#"{
          "metadata": { "protocol": "query", "signatureVersion": "v4" },
          "operations": { "Op": { "authtype": "v9-magic" } }
        }"#;
        assert!(matches!(
            from_botocore(json).expect_err("must refuse"),
            ParseError::UnknownAuthType(_)
        ));
    }

    #[test]
    fn missing_protocol_or_signature_version_is_refused() {
        for meta in [
            r#"{"signatureVersion": "v4"}"#,
            r#"{"protocol": "query"}"#,
            r#"{"protocol": "json", "signatureVersion": "v4"}"#, // no jsonVersion
        ] {
            let err = from_botocore(&with_metadata(meta)).expect_err("must refuse");
            assert!(
                matches!(err, ParseError::ShapeMissing(_) | ParseError::UnknownJsonVersion(_)),
                "got {err:?} for {meta}"
            );
        }
    }

    #[test]
    fn operation_metadata_is_carried() {
        let json = r#"{
          "metadata": {
            "protocol": "json", "jsonVersion": "1.1", "signatureVersion": "v4",
            "endpointPrefix": "kms", "signingName": "kms",
            "targetPrefix": "TrentService"
          },
          "shapes": { "Req": { "type": "structure", "members": {} } },
          "operations": {
            "Encrypt": {
              "http": { "method": "POST", "requestUri": "/" },
              "input": { "shape": "Req" },
              "output": { "shape": "Req", "resultWrapper": "EncryptResult" },
              "errors": [
                { "shape": "NotFoundException" },
                { "shape": "KMSInternalException" }
              ],
              "authtype": "v4-unsigned-body"
            },
            "Anonymous": { "authtype": "none" }
          }
        }"#;
        let svc = from_botocore(json).unwrap();
        let aws = svc.aws.as_ref().unwrap();
        assert_eq!(aws.protocol, AwsProtocol::AwsJson(JsonVersion::V1_1));
        assert_eq!(aws.target_prefix.as_deref(), Some("TrentService"));
        assert_eq!(aws.signing_name.as_deref(), Some("kms"));

        let op = svc.operations.get("Encrypt").unwrap();
        assert_eq!(op.result_wrapper.as_deref(), Some("EncryptResult"));
        assert_eq!(op.errors, vec!["NotFoundException", "KMSInternalException"]);
        assert_eq!(op.auth_override, Some(AuthOverride::UnsignedPayload));
        assert_eq!(
            svc.operations.get("Anonymous").unwrap().auth_override,
            Some(AuthOverride::NoAuth)
        );
    }

    #[test]
    fn inline_members_resolve_instead_of_defaulting_to_string() {
        // The minified serialization writes member types inline. Defaulting
        // them all to `String` (the pre-change behaviour) emitted a wrong
        // type for every non-string member.
        let json = r#"{
          "metadata": { "protocol": "query", "signatureVersion": "v4" },
          "operations": {
            "Describe": {
              "output": {
                "type": "structure",
                "members": {
                  "Name": {},
                  "Count": { "type": "integer" },
                  "Tags": { "type": "list", "member": { "type": "structure",
                            "members": { "Key": {} } } }
                }
              }
            }
          }
        }"#;
        let svc = from_botocore(json).unwrap();
        let out = svc.operations.get("Describe").unwrap().output_shape.clone().unwrap();
        assert_eq!(out, "DescribeResponse");
        let ShapeKind::Object { members, .. } = &svc.shapes.get(&out).unwrap().kind else {
            panic!("expected object")
        };
        assert_eq!(members.get("Name").unwrap().shape, "String");
        assert_eq!(members.get("Count").unwrap().shape, "Integer");
        assert_eq!(members.get("Tags").unwrap().shape, "DescribeResponseTags");
        // The synthesized primitives are declared, so nothing dangles.
        assert!(matches!(
            svc.shapes.get("Integer").unwrap().kind,
            ShapeKind::Primitive(ref p) if p == "integer"
        ));
        // ...and the nested inline list element became a real shape.
        assert!(svc.shapes.contains_key("DescribeResponseTagsMember"));
    }

    #[test]
    fn synthesized_shape_never_silently_overwrites_a_declared_one() {
        // If a model ever declares a shape under the name we would
        // synthesize, refuse — silently overwriting it would emit one
        // shape's members under the other's name.
        let json = r#"{
          "metadata": { "protocol": "query", "signatureVersion": "v4" },
          "shapes": {
            "DescribeResponse": { "type": "structure",
                                  "members": { "Declared": { "shape": "S1" } } },
            "S1": { "type": "string" }
          },
          "operations": {
            "Describe": { "output": { "type": "structure",
                                      "members": { "Inline": {} } } }
          }
        }"#;
        let err = from_botocore(json).expect_err("must refuse");
        assert!(
            matches!(&err, ParseError::SynthesizedShapeCollision(n) if n == "DescribeResponse"),
            "got {err:?}"
        );
    }

    #[test]
    fn openapi_input_carries_no_aws_metadata() {
        let yaml = "openapi: 3.0.0\ninfo:\n  title: Demo\n  version: '1'\npaths: {}\n";
        assert!(from_openapi(yaml).unwrap().aws.is_none());
    }
}
