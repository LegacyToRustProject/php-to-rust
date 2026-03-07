# php-to-rust

**AI-powered PHP → Rust conversion agent.**

Converts entire PHP projects to idiomatic Rust — not just snippets, but full codebases with automated verification.

## Why

PHP powers 77% of the web. Much of it is legacy code with no maintainer, no specs, and growing security risk. The source code IS the spec. AI can read it, understand the intent, and generate equivalent Rust code. A verification loop drives correctness to 100%.

## How It Works

```
PHP project (source code + running instance)
    ↓ 1. Parse & analyze (file structure, dependencies, entry points)
    ↓ 2. AI converts each module to Rust
    ↓ 3. cargo check (must compile)
    ↓ 4. Run both PHP & Rust, compare outputs
    ↓ 5. Diff? → AI fixes → goto 3
    ↓ 6. Repeat until all outputs match
Verified Rust project
```

This is NOT a traditional transpiler. It's an **AI agent** that understands PHP semantics and writes idiomatic Rust — the same process a human engineer would follow, but at scale.

## Version Compatibility

**Initial target: PHP 8.x.** Then expanding backward — because we won't abandon anyone.

| PHP Version | Priority | Notes |
|-------------|----------|-------|
| 8.0 - 8.4 | **First** | Strict types, union types, named args. Easiest to convert. |
| 7.4 | Second | Return types, typed properties. WordPress 6.x minimum. |
| 7.0 - 7.3 | Third | Scalar type hints introduced. Large installed base. |
| 5.6 | Fourth | No type hints = AI infers types freely. WordPress 4.x era. |
| 5.3 - 5.5 | Fifth | Namespaces, traits. Still running on forgotten servers. |

Older versions are simpler (fewer features = fewer conversion patterns). The hardest version to convert is the latest. Once 8.x works, past versions follow naturally.

Auto-detection: `php-to-rust analyze` detects the PHP version and selects the appropriate conversion profile.

## Architecture

```
php-to-rust/
├── crates/
│   ├── php-parser/       # PHP source analysis (leverages php-rust-tools/parser)
│   ├── rust-generator/   # AI-powered Rust code generation
│   ├── verifier/         # Compile check + output comparison loop
│   └── cli/              # Command-line interface
├── profiles/
│   ├── wordpress/        # WordPress API mappings (wp_*, hooks, filters)
│   ├── laravel/          # Laravel framework mappings (future)
│   └── generic/          # Plain PHP
└── tests/
    ├── fixtures/         # PHP input → expected Rust output pairs
    └── integration/      # End-to-end conversion tests
```

## Profiles

Framework-specific knowledge is separated into **profiles**:

| Profile | Target | Status |
|---------|--------|--------|
| `wordpress` | WordPress plugins, themes, core functions | First priority |
| `laravel` | Laravel applications | Planned |
| `generic` | Plain PHP / any framework | Planned |

## Usage (Planned)

```bash
# Convert a WordPress plugin to Rust
php-to-rust convert ./my-wp-plugin --profile wordpress --verify

# Convert a generic PHP project
php-to-rust convert ./my-php-app --profile generic --verify

# Analyze without converting (compatibility report)
php-to-rust analyze ./my-php-app
```

## Verification

The key differentiator: every conversion is **verified**.

1. **Compile check** — Generated Rust must pass `cargo check`
2. **Output comparison** — Run the same inputs through PHP and Rust, diff the outputs
3. **AI fix loop** — If outputs differ, AI reads the diff and fixes the Rust code
4. **Repeat** — Until all test cases produce identical output

## Relationship to RustPress

[RustPress](https://github.com/LegacyToRustProject/RustPress) is the first and largest proof-of-concept: a WordPress-compatible CMS converted from PHP to Rust. php-to-rust will power RustPress's plugin and theme conversion pipeline, and RustPress serves as the real-world test bed for the conversion engine.

## Status

**Early development.** Architecture and core design in progress.

## License

MIT
