# Generator for OpenAPI v3.1 rust clients

```
opage -s spec.openapi.yaml -o output_client
```

## Arguments

| Name       | Short | Example              | Description                                                                     |
| ---------- | ----- | -------------------- | ------------------------------------------------------------------------------- |
| spec       | s     | -s spec.openapi.yaml | File which contains the spec                                                    |
| output-dir | p     | -o output            | Target directory for generated client                                           |
| config     | c     | -m mapping.yaml      | File which contains name mappings or ignores if rust conflicts with given names |

## Build

```
cargo build --release
```

## Unsupported Properties

- prefixItems
