# lava-api-forge

Cloud API spec → typed tatara-lisp bindings, for the
[lava](https://github.com/pleme-io) suite.

Skips the Terraform provider middleman. Consumes the upstream API spec directly
— **OpenAPI 3** or **AWS botocore `service-2.json`** — and emits a `.tlisp`
source file with one typed `(defapi-operation …)` form per operation and one
`(defapi-shape …)` form per data type.

The output is consumed by `lava-provider-gen` to autogenerate full
tfplugin5/6-compatible magma providers.

## Pipeline

```text
<service>.openapi.yaml   |   <service>.botocore.json
        │
        ▼  spec::from_openapi | spec::from_botocore
Service                                   ← typed canonical IR
        │
        ▼  emit
(defapi-operation …) / (defapi-shape …) .tlisp forms
```

## Usage

```toml
[dependencies]
lava-api-forge = "0.1"
```

## Sibling

`lava-forge` generates the same typed surface from a **Terraform provider
schema** instead of a cloud API spec. Use lava-forge when a provider already
exists and is authoritative; use lava-api-forge when you want the API itself as
the source of truth.

## License

MIT
