//! End-to-end proof on a real AWS model: `sts:GetCallerIdentity`.
//!
//! `tests/fixtures/sts-2011-06-15.min.json` is AWS's own STS model,
//! copied byte-for-byte from the `aws-sdk` JS distribution
//! (sha256 0e20e394…ce3aab6). Nothing about it is synthetic.
//!
//! `GetCallerIdentity` is the smallest real AWS call there is: an empty
//! request structure, three untyped strings back, no pagination, no
//! waiters, no endpoint rules, no nested types. If the emitted `.tlisp`
//! carries enough for a runtime to call *this*, the metadata plumbing is
//! right; everything harder is a bigger shape graph over the same facts.

use lava_api_forge::{from_botocore, render_service, AwsAuth, AwsProtocol};

const STS: &str = include_str!("fixtures/sts-2011-06-15.min.json");

/// Collapse whitespace runs so assertions describe *content*, not
/// lava-forge's line breaking.
fn flat(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

#[test]
fn sts_metadata_is_carried_into_the_ir() {
    let svc = from_botocore(STS).expect("real STS model parses");
    assert_eq!(svc.name, "STS");
    assert_eq!(svc.version, "2011-06-15");

    let aws = svc.aws.as_ref().expect("botocore input yields AWS metadata");
    assert_eq!(aws.protocol, AwsProtocol::AwsQuery);
    assert_eq!(aws.auth, AwsAuth::SigV4);
    assert_eq!(aws.endpoint_prefix, "sts");
    assert_eq!(aws.global_endpoint.as_deref(), Some("sts.amazonaws.com"));
    assert_eq!(
        aws.xml_namespace.as_deref(),
        Some("https://sts.amazonaws.com/doc/2011-06-15/")
    );
    // STS is a query service: no X-Amz-Target.
    assert_eq!(aws.target_prefix, None);
}

#[test]
fn sts_get_caller_identity_shapes_survive_the_inline_serialization() {
    let svc = from_botocore(STS).expect("real STS model parses");
    let op = svc
        .operations
        .get("GetCallerIdentity")
        .expect("GetCallerIdentity is modeled");

    // The minified serialization writes input/output inline, so these are
    // synthesized names — but they must exist and must resolve.
    let input = op.input_shape.as_deref().expect("input shape resolved");
    let output = op.output_shape.as_deref().expect("output shape resolved");
    assert_eq!(input, "GetCallerIdentityRequest");
    assert_eq!(output, "GetCallerIdentityResponse");
    assert!(svc.shapes.contains_key(input), "input shape registered");

    // The result wrapper is what lets a runtime unwrap the query response.
    assert_eq!(op.result_wrapper.as_deref(), Some("GetCallerIdentityResult"));

    // Input is an empty structure; output carries exactly the three
    // documented strings.
    match &svc.shapes.get(input).expect("present").kind {
        lava_api_forge::ShapeKind::Object { members, required } => {
            assert!(members.is_empty(), "GetCallerIdentity takes no arguments");
            assert!(required.is_empty());
        }
        other => panic!("expected an object input, got {other:?}"),
    }
    match &svc.shapes.get(output).expect("present").kind {
        lava_api_forge::ShapeKind::Object { members, .. } => {
            let mut names: Vec<&str> = members.keys().map(String::as_str).collect();
            names.sort_unstable();
            assert_eq!(names, ["Account", "Arn", "UserId"]);
            for (name, m) in members {
                assert_eq!(m.shape, "String", "{name} is an untyped string");
            }
        }
        other => panic!("expected an object output, got {other:?}"),
    }

    // Every member reference resolves to a shape we actually emit — no
    // dangling `:shape :string`.
    for (sname, shape) in &svc.shapes {
        if let lava_api_forge::ShapeKind::Object { members, .. } = &shape.kind {
            for (mname, m) in members {
                assert!(
                    svc.shapes.contains_key(&m.shape),
                    "{sname}.{mname} references undeclared shape {}",
                    m.shape
                );
            }
        }
    }
}

#[test]
fn sts_emission_carries_protocol_auth_endpoint_and_result_wrapper() {
    let svc = from_botocore(STS).expect("real STS model parses");
    let out = flat(&render_service(&svc));

    // Service-level wire + auth facts.
    assert!(out.contains("(defapi-service :sts"), "service form present");
    assert!(out.contains(":protocol awsQuery"), "Smithy protocol name emitted");
    assert!(out.contains(":auth sigv4"), "Smithy auth scheme emitted");
    assert!(
        out.contains(":auth-source \"v4\""),
        "botocore token preserved alongside the Smithy name"
    );
    assert!(out.contains(":endpoint-prefix \"sts\""));
    assert!(out.contains(":global-endpoint \"sts.amazonaws.com\""));
    assert!(out.contains(":xml-namespace \"https://sts.amazonaws.com/doc/2011-06-15/\""));

    // Operation-level.
    assert!(out.contains(":sts/get-caller-identity"));
    assert!(out.contains(":result-wrapper \"GetCallerIdentityResult\""));
    assert!(out.contains(":input :get-caller-identity-request"));
    assert!(out.contains(":output :get-caller-identity-response"));

    // None of this existed before: the pre-change emitter emitted no
    // protocol, no auth and no endpoint at all.
    for absent in [":protocol", ":auth ", ":endpoint-prefix"] {
        assert!(out.contains(absent), "{absent} must appear in the emission");
    }
}
