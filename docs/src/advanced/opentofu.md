# OpenTofu Compatibility

Lambdaform works identically with [OpenTofu](https://opentofu.org/) and Terraform. Both use the same `.tf` HCL syntax for resource definitions.

## No Configuration Needed

Lambdaform parses `.tf` files directly — it doesn't shell out to `terraform` or `tofu`. This means:

- No Terraform or OpenTofu binary is required at runtime
- No state files are read
- No provider initialization is needed
- `.tf` files from either tool work the same way

## OpenTofu-Specific Features

OpenTofu's extended features (e.g., `encryption` blocks, state encryption) don't affect Lambdaform since it only reads resource definitions.

## Testing

Lambdaform's test suite includes OpenTofu-specific test fixtures to verify compatibility.
