# Dockerfile Generator

A command-line tool to generate Docker-related configuration files for development environments.

## Installation

```bash
cargo build --release
```

The binary will be available at `target/release/dockerfile_generator`

## Commands

### 1. Generate Single File

Generate individual configuration files using the following commands:

```bash
dockerfile_generator <file-type> [options]
```

Available file types:

- `dockerfile`
- `devcontainer`
- `compose`
- `bake`

### 2. Generate All Files

Generate a complete development container setup:

```bash
dockerfile_generator all --folder <output-directory>
```

This creates all necessary files in the specified directory:

- `Dockerfile`
- `devcontainer.json`
- `docker-compose.yml`
- `docker-bake.hcl`

## Examples

### Generate a Dockerfile

```bash
dockerfile_generator dockerfile \
    --base-image rust:1.83-slim \
    --maintainer "Generated <generated@example.com>" \
    --packages "cargo,git" \
    --workdir /app \
    --entrypoint /bin/bash \
    --output Dockerfile
```

### Generate Development Container Files

```bash
dockerfile_generator all \
    --folder ./devcontainer \
    --base-image rust:1.83-slim \
    --maintainer "Generated <generated@example.com>"
```

## Options

### Global Options

- `--folder`: Output directory for generated files
- `--output`: Specify output filename (for single file generation)

### Dockerfile Options

- `--base-image`: Base Docker image
- `--maintainer`: Maintainer information
- `--packages`: Comma-separated list of packages to install
- `--workdir`: Working directory in container
- `--entrypoint`: Container entrypoint

### DevContainer Options

- `--name`: Container name
- `--features`: Additional features to include
- `--extensions`: VS Code extensions to install

## Contributing

Contributions are welcome! Please feel free to submit a Pull Request.

## License

[Add your license information here]
