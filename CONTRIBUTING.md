# Contributing to the HAL Enforcement Service

Thanks for your interest in contributing! This service is developed within the ELASTIC project, together with [WasmHAL](https://github.com/elasticproject-eu/wasmhal), and is open to external contributions.

## Reporting issues

- Use [GitHub Issues](https://github.com/syafiq/enforcement-service/issues) for bugs and feature requests.
- For bugs, include:
  - the Rust version;
  - the policy file, or a minimal version of it;
  - the command you ran;
  - the output, preferably with `--verbose`.
- **Security vulnerabilities:** please do not open a public issue. See [SECURITY.md](SECURITY.md).

## Making changes

1. Open an issue first for larger changes, such as new policy fields or API endpoints, so the design can be discussed before you write code.
2. Fork the repository and create a branch from `main`.
3. Keep changes focused: one logical change per pull request.
4. Before opening a pull request, make sure the following pass:

   ```bash
   cargo fmt --all -- --check
   cargo clippy --all-targets
   cargo test
   cargo run -- --policy policies/example.yaml --validate-only
   ```

5. Add or update tests and documentation, including the example policies in `policies/`, for any change in behaviour.
6. Open a pull request against `main` with a short description of what changed and why.

## Code of conduct

This project follows the [Code of Conduct](CODE_OF_CONDUCT.md), which is based on the Contributor Covenant 2.1.

## Licence

By contributing, you agree that your contributions will be licensed under the [MIT License](LICENSE).
