# Generator for OpenAPI v3.1 rust clients

```
opage -s spec.openapi.yaml -o output_client
```

Default configuration

```json
{
  "project_metadata": {
    "name": "project-name",
    "version": "0.0.0"
  },
  "name_mapping": {
    "struct_mapping": {
      "/Component/SubComponent/TestObject": "TestObjectData"
    },
    "property_mapping": {},
    "module_mapping": {},
    "status_code_mapping": {}
  },
  "ignore": {
    "paths": [],
    "components": []
  }
}
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

## Tests

sccache will be used and set in tests.sh. Its used to reduce build times as each test project is build from scratch and should use the same packages and configuration

```
cargo install sccache
./tests.sh
```
