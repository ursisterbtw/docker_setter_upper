use clap::{Parser, Subcommand};
use serde::Serialize;
use serde_json::json;
use std::fs::File;
use std::io::Write;
use std::path::Path;
use tera::{Context, Tera};

// =====================
//     DATA STRUCTS
// =====================

#[derive(Debug, Serialize)]
struct DockerfileSpec {
    base_image: String,
    maintainer: String,
    packages: Vec<String>,
    workdir: String,
    entrypoint: String,
}

#[derive(Debug, Serialize)]
struct DevContainerSpec {
    name: String,
    dockerfile_path: String,
    remote_user: String,
    customizations: DevContainerCustomizations,
}

#[derive(Debug, Serialize)]
struct DevContainerCustomizations {
    vscode_extensions: Vec<String>,
    settings: serde_json::Value,
}

#[derive(Debug, Serialize)]
struct DockerComposeSpec {
    services: Vec<ServiceSpec>,
    networks: Vec<String>,
}

#[derive(Debug, Serialize)]
struct ServiceSpec {
    name: String,
    image: String,
    ports: Vec<String>,
    depends_on: Vec<String>,
    environment: Vec<(String, String)>,
    volumes: Vec<String>,
}

#[derive(Debug, Serialize)]
struct DockerBakeSpec {
    group_name: String,
    targets: Vec<BakeTarget>,
}

#[derive(Debug, Serialize)]
struct BakeTarget {
    name: String,
    context: String,
    dockerfile: String,
    tags: Vec<String>,
}

// =====================
//     TEMPLATES
// =====================

static DOCKERFILE_TEMPLATE: &str = r#"
# Generated Dockerfile
FROM {{ base_image }}
LABEL maintainer="{{ maintainer }}"
RUN apt-get update && apt-get install -y \
{%- for pkg in packages %}
    {{ pkg }} \
{%- endfor %}
WORKDIR {{ workdir }}
ENTRYPOINT ["{{ entrypoint }}"]
"#;

static DEVCONTAINER_TEMPLATE: &str = r#"
{
    "name": "{{ name }}",
    "build": {
        "dockerfile": "{{ dockerfile_path }}"
    },
    "remoteUser": "{{ remote_user }}",
    "customizations": {
        "vscode": {
            "extensions": {{ customizations.vscode_extensions | json_encode }},
            "settings": {{ customizations.settings | json_encode }}
        }
    }
}
"#;

static DOCKER_COMPOSE_TEMPLATE: &str = r#"
version: '3.8'
services:
{%- for service in services %}
  {{ service.name }}:
    image: {{ service.image }}
    ports:
    {%- for port in service.ports %}
      - "{{ port }}"
    {%- endfor %}
    depends_on:
    {%- for dep in service.depends_on %}
      - {{ dep }}
    {%- endfor %}
    environment:
    {%- for env in service.environment %}
      {{ env.0 }}: "{{ env.1 }}"
    {%- endfor %}
    volumes:
    {%- for volume in service.volumes %}
      - {{ volume }}
    {%- endfor %}
{%- endfor %}

networks:
{%- for net in networks %}
  {{ net }}:
{%- endfor %}
"#;

static DOCKER_BAKE_TEMPLATE: &str = r#"
group "{{ group_name }}" {
  targets = [
{%- for t in targets %}
    "{{ t.name }}",
{%- endfor %}
  ]
}

{%- for t in targets %}
target "{{ t.name }}" {
  context    = "{{ t.context }}"
  dockerfile = "{{ t.dockerfile }}"
  tags       = [
    {%- for tag in t.tags %}
    "{{ tag }}",
    {%- endfor %}
  ]
}
{%- endfor %}
"#;

// =====================
//   TEMPLATE RENDER
// =====================

fn render_template<T: Serialize>(template_str: &str, data: &T) -> Result<String, tera::Error> {
    let mut tera = Tera::default();
    tera.add_raw_template("dynamic_template", template_str)?;
    let context = Context::from_serialize(data)?;
    tera.render("dynamic_template", &context)
}

fn write_to_file(output_path: &Path, contents: &str) -> std::io::Result<()> {
    let mut file = File::create(output_path)?;
    file.write_all(contents.as_bytes())?;
    println!("Wrote file to: {}", output_path.display());
    Ok(())
}

// =====================
//     CLI COMMANDS
// =====================

#[derive(Parser)]
#[command(
    name = "configgen",
    version = "0.1.0",
    about = "Generates Docker/OCI-related config files in pure Rust!"
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Generate only a Dockerfile
    Dockerfile {
        /// Base image to use
        #[arg(long, default_value = "ubuntu:22.04")]
        base_image: String,
        /// Name/email of maintainer
        #[arg(long, default_value = "Jane Doe <jane@example.com>")]
        maintainer: String,
        /// Comma-separated list of packages
        #[arg(long, default_value = "curl,git")]
        packages: String,
        /// Working directory in container
        #[arg(long, default_value = "/app")]
        workdir: String,
        /// Entrypoint
        #[arg(long, default_value = "/bin/bash")]
        entrypoint: String,
        /// Output filename
        #[arg(short, long, default_value = "Dockerfile")]
        output: String,
    },
    /// Generate only a devcontainer.json
    Devcontainer {
        /// Dev container name
        #[arg(long, default_value = "My Dev Container")]
        name: String,
        /// Dockerfile path
        #[arg(long, default_value = "./Dockerfile")]
        dockerfile: String,
        /// Remote user name
        #[arg(long, default_value = "vscode")]
        remote_user: String,
        /// Comma-separated VSCode extensions
        #[arg(long, default_value = "ms-azuretools.vscode-docker,rust-lang.rust-analyzer")]
        extensions: String,
        /// Output filename
        #[arg(short, long, default_value = "devcontainer.json")]
        output: String,
    },
    /// Generate only a docker-compose.yml
    Compose {
        /// Output filename
        #[arg(short, long, default_value = "docker-compose.yml")]
        output: String,
    },
    /// Generate only a docker-bake.hcl
    Bake {
        /// Output filename
        #[arg(short, long, default_value = "docker-bake.hcl")]
        output: String,
    },
    /// Generate all files (Dockerfile, devcontainer.json, docker-compose.yml, docker-bake.hcl)
    All {
        /// Output folder (files will be named Dockerfile, devcontainer.json, etc.)
        #[arg(short, long, default_value = ".")]
        folder: String,
    },
}

fn main() -> std::io::Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Dockerfile {
            base_image,
            maintainer,
            packages,
            workdir,
            entrypoint,
            output,
        } => {
            let spec = DockerfileSpec {
                base_image,
                maintainer,
                packages: packages
                    .split(',')
                    .map(|s| s.trim().to_string())
                    .collect::<Vec<_>>(),
                workdir,
                entrypoint,
            };
            let rendered = render_template(DOCKERFILE_TEMPLATE, &spec)
                .expect("Failed to render Dockerfile template");
            write_to_file(Path::new(&output), &rendered)?;
        }

        Commands::Devcontainer {
            name,
            dockerfile,
            remote_user,
            extensions,
            output,
        } => {
            let spec = DevContainerSpec {
                name,
                dockerfile_path: dockerfile,
                remote_user,
                customizations: DevContainerCustomizations {
                    vscode_extensions: extensions
                        .split(',')
                        .map(|s| s.trim().to_string())
                        .collect::<Vec<_>>(),
                    settings: json!({
                        "editor.formatOnSave": true,
                        "terminal.integrated.shell.linux": "/bin/bash"
                    }),
                },
            };
            let rendered = render_template(DEVCONTAINER_TEMPLATE, &spec)
                .expect("Failed to render devcontainer template");
            write_to_file(Path::new(&output), &rendered)?;
        }

        Commands::Compose { output } => {
            // Hard-coded example for demonstration
            let compose_spec = DockerComposeSpec {
                services: vec![
                    ServiceSpec {
                        name: "web".to_string(),
                        image: "nginx:latest".to_string(),
                        ports: vec!["80:80".to_string()],
                        depends_on: vec!["db".to_string()],
                        environment: vec![
                            ("NGINX_HOST".to_string(), "localhost".to_string()),
                            ("NGINX_PORT".to_string(), "80".to_string()),
                        ],
                        volumes: vec!["./web_data:/usr/share/nginx/html".to_string()],
                    },
                    ServiceSpec {
                        name: "db".to_string(),
                        image: "postgres:latest".to_string(),
                        ports: vec!["5432:5432".to_string()],
                        depends_on: vec![],
                        environment: vec![
                            ("POSTGRES_USER".to_string(), "admin".to_string()),
                            ("POSTGRES_PASSWORD".to_string(), "password".to_string()),
                        ],
                        volumes: vec!["./db_data:/var/lib/postgresql/data".to_string()],
                    },
                ],
                networks: vec!["app_network".to_string()],
            };
            let rendered = render_template(DOCKER_COMPOSE_TEMPLATE, &compose_spec)
                .expect("Failed to render docker-compose.yml");
            write_to_file(Path::new(&output), &rendered)?;
        }

        Commands::Bake { output } => {
            // Hard-coded example for demonstration
            let bake_spec = DockerBakeSpec {
                group_name: "default".to_string(),
                targets: vec![
                    BakeTarget {
                        name: "serviceA".to_string(),
                        context: "./serviceA".to_string(),
                        dockerfile: "./serviceA/Dockerfile".to_string(),
                        tags: vec!["serviceA:latest".to_string()],
                    },
                    BakeTarget {
                        name: "serviceB".to_string(),
                        context: "./serviceB".to_string(),
                        dockerfile: "./serviceB/Dockerfile".to_string(),
                        tags: vec!["serviceB:latest".to_string()],
                    },
                ],
            };
            let rendered = render_template(DOCKER_BAKE_TEMPLATE, &bake_spec)
                .expect("Failed to render docker-bake.hcl");
            write_to_file(Path::new(&output), &rendered)?;
        }

        Commands::All { folder } => {
            // 1) Dockerfile
            let dockerfile_spec = DockerfileSpec {
                base_image: "ubuntu:22.04".to_string(),
                maintainer: "Jane Doe <jane@example.com>".to_string(),
                packages: vec!["curl".into(), "git".into()],
                workdir: "/app".to_string(),
                entrypoint: "/bin/bash".to_string(),
            };
            let dockerfile_str = render_template(DOCKERFILE_TEMPLATE, &dockerfile_spec)
                .expect("Failed to render Dockerfile");
            write_to_file(Path::new(&folder).join("Dockerfile").as_path(), &dockerfile_str)?;

            // 2) devcontainer.json
            let devcontainer_spec = DevContainerSpec {
                name: "My Dev Container".to_string(),
                dockerfile_path: "./Dockerfile".to_string(),
                remote_user: "vscode".to_string(),
                customizations: DevContainerCustomizations {
                    vscode_extensions: vec![
                        "ms-azuretools.vscode-docker".to_string(),
                        "rust-lang.rust-analyzer".to_string(),
                    ],
                    settings: json!({
                        "editor.formatOnSave": true,
                        "terminal.integrated.shell.linux": "/bin/bash"
                    }),
                },
            };
            let devcontainer_str = render_template(DEVCONTAINER_TEMPLATE, &devcontainer_spec)
                .expect("Failed to render devcontainer.json");
            write_to_file(
                Path::new(&folder).join("devcontainer.json").as_path(),
                &devcontainer_str,
            )?;

            // 3) docker-compose.yml
            let compose_spec = DockerComposeSpec {
                services: vec![
                    ServiceSpec {
                        name: "web".into(),
                        image: "nginx:latest".into(),
                        ports: vec!["80:80".into()],
                        depends_on: vec!["db".into()],
                        environment: vec![
                            ("NGINX_HOST".into(), "localhost".into()),
                            ("NGINX_PORT".into(), "80".into()),
                        ],
                        volumes: vec!["./web_data:/usr/share/nginx/html".into()],
                    },
                    ServiceSpec {
                        name: "db".into(),
                        image: "postgres:latest".into(),
                        ports: vec!["5432:5432".into()],
                        depends_on: vec![],
                        environment: vec![
                            ("POSTGRES_USER".into(), "admin".into()),
                            ("POSTGRES_PASSWORD".into(), "password".into()),
                        ],
                        volumes: vec!["./db_data:/var/lib/postgresql/data".into()],
                    },
                ],
                networks: vec!["app_network".into()],
            };
            let compose_str = render_template(DOCKER_COMPOSE_TEMPLATE, &compose_spec)
                .expect("Failed to render docker-compose.yml");
            write_to_file(
                Path::new(&folder).join("docker-compose.yml").as_path(),
                &compose_str,
            )?;

            // 4) docker-bake.hcl
            let bake_spec = DockerBakeSpec {
                group_name: "default".to_string(),
                targets: vec![
                    BakeTarget {
                        name: "serviceA".into(),
                        context: "./serviceA".into(),
                        dockerfile: "./serviceA/Dockerfile".into(),
                        tags: vec!["serviceA:latest".into()],
                    },
                    BakeTarget {
                        name: "serviceB".into(),
                        context: "./serviceB".into(),
                        dockerfile: "./serviceB/Dockerfile".into(),
                        tags: vec!["serviceB:latest".into()],
                    },
                ],
            };
            let bake_str = render_template(DOCKER_BAKE_TEMPLATE, &bake_spec)
                .expect("Failed to render docker-bake.hcl");
            write_to_file(Path::new(&folder).join("docker-bake.hcl").as_path(), &bake_str)?;
        }
    }

    Ok(())
}
