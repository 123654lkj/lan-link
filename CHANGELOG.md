# Changelog

## v0.3.0 (2026-06-04)

### Added
- Integrated ll-vpn P2P VPN mesh as `crates/vpn/`
- `lan-linkd --vpn` / `--node-name` / `--vpn-port` CLI flags for VPN mode
- `lan-linkctl --vpn` CLI flag (routing placeholder)
- CHANGELOG, CONTRIBUTING, LICENSE files for GitHub open source

### Changed
- Workspace includes `crates/vpn` member
- daemon Cargo.toml depends on `lan-link-vpn` crate
- Version bumped to 0.3.0 across all crates

### Removed
- input/ directory (dead code, replaced by native_cmd)
- gui/ directory (pure CLI tool)

## v0.2.0 (2026-06-03)
- Remove input crate, single-file native_cmd, GitHub push
