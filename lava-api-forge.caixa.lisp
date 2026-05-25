(defcaixa
  :name
  "lava-api-forge"
  :kind
  :Biblioteca
  :ecosystem
  :rust-single-crate
  :package
  {:name "lava-api-forge"
   :version "0.1.0"
   :description "OpenAPI / AWS botocore service spec → typed tatara-lisp (defapi-operation ...) bindings. Skips the Terraform provider middleman; generates directly from cloud API specs. Output consumed by lava-provider-gen to autogenerate full tfplugin5/6-compatible magma providers."
   :license "MIT"
   :repository "https://github.com/pleme-io/lava-api-forge"}
  :ci-config
  {:bump {:default-type "patch"}
   :publish {:no-verify true}}
  :workflows
  [:auto-release :pre-merge-gate :security-gate])
