use clap::{Parser, Subcommand};
use serde::Serialize;
use serde_json::json;
use std::fs::File;
use std::io::{self, Write};
use std::path::Path;
use tera::{Context, Tera};
use std::collections::HashMap;

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
    networks: HashMap<String, NetworkConfig>,
}

#[derive(Debug, Serialize)]
struct NetworkConfig {
    driver: String,
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
    {%- if service.depends_on | length > 0 %}
    depends_on:
    {%- for dep in service.depends_on %}
      - {{ dep }}
    {%- endfor %}
    {%- endif %}
    {%- if service.environment | length > 0 %}
    environment:
    {%- for env in service.environment %}
      {{ env.0 }}: "{{ env.1 }}"
    {%- endfor %}
    {%- endif %}
    volumes:
    {%- for volume in service.volumes %}
      - {{ volume }}
    {%- endfor %}
{%- endfor %}

{%- if networks | length > 0 %}
networks:
{%- for name, config in networks %}
  {{ name }}:
    driver: {{ config.driver }}
{%- endfor %}
{%- endif %}
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
    /// Generate a docker-compose.yml with customizable services
    Compose {
        /// Output filename
        #[arg(short, long, default_value = "docker-compose.yml")]
        output: String,
        /// Comma-separated list of services to include (e.g., nginx,postgres,redis)
        #[arg(long)]
        services: Option<String>,
        /// Comma-separated list of ports for each service (e.g., "80:80,5432:5432,6379:6379")
        #[arg(long)]
        ports: Option<String>,
        /// Comma-separated list of volumes (e.g., "./data:/var/lib/postgresql/data")
        #[arg(long)]
        volumes: Option<String>,
        /// Comma-separated list of environment variables (e.g., "POSTGRES_USER=admin,POSTGRES_PASSWORD=secret")
        #[arg(long)]
        env: Option<String>,
        /// Comma-separated list of networks to create (defaults to bridge driver)
        #[arg(long, default_value = "app_network")]
        networks: String,
        /// Comma-separated list of service dependencies (e.g., "web:db,cache:db")
        #[arg(long)]
        depends_on: Option<String>,
    },
    /// Generate a docker-bake.hcl with customizable targets
    Bake {
        /// Output filename
        #[arg(short, long, default_value = "docker-bake.hcl")]
        output: String,
        /// Group name for the targets
        #[arg(long, default_value = "default")]
        group: String,
        /// Comma-separated list of target names (e.g., "api,worker,scheduler")
        #[arg(long)]
        targets: Option<String>,
        /// Comma-separated list of contexts for each target (e.g., "./api,./worker,./scheduler")
        #[arg(long)]
        contexts: Option<String>,
        /// Comma-separated list of Dockerfile paths (e.g., "./api/Dockerfile,./worker/Dockerfile")
        #[arg(long)]
        dockerfiles: Option<String>,
        /// Comma-separated list of tags for each target (e.g., "api:latest,worker:latest")
        #[arg(long)]
        tags: Option<String>,
    },
    /// Generate a development container configuration
    Devcontainer {
        /// Container name
        #[arg(long, default_value = "Dev Container")]
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
    /// Generate a complete development environment
    Init {
        /// Project name
        #[arg(long, default_value = "myproject")]
        name: String,
        /// Programming language/framework (e.g., python, node, rust)
        #[arg(long, default_value = "python")]
        language: String,
        /// Database type (e.g., postgres, mysql, mongodb)
        #[arg(long)]
        database: Option<String>,
        /// Additional services (comma-separated, e.g., redis,elasticsearch)
        #[arg(long)]
        services: Option<String>,
        /// Output directory
        #[arg(short, long, default_value = ".")]
        output_dir: String,
    },
}

fn prompt(message: &str) -> io::Result<String> {
    print!("{}: ", message);
    io::stdout().flush()?;
    let mut input = String::new();
    io::stdin().read_line(&mut input)?;
    Ok(input.trim().to_string())
}

fn select_option(options: &[&str], prompt_msg: &str) -> io::Result<usize> {
    println!("\n{}", prompt_msg);
    for (i, opt) in options.iter().enumerate() {
        println!("{}. {}", i + 1, opt);
    }
    
    loop {
        let input = prompt("Enter number")?;
        if let Ok(num) = input.parse::<usize>() {
            if num > 0 && num <= options.len() {
                return Ok(num - 1);
            }
        }
        println!("Please enter a number between 1 and {}", options.len());
    }
}

fn confirm(message: &str) -> io::Result<bool> {
    loop {
        let input = prompt(&format!("{} (y/n)", message))?.to_lowercase();
        match input.as_str() {
            "y" | "yes" => return Ok(true),
            "n" | "no" => return Ok(false),
            _ => println!("Please enter 'y' or 'n'"),
        }
    }
}

fn interactive_cli() -> io::Result<()> {
    println!("\n=== Docker Configuration Generator ===\n");
    
    let options = ["Generate Dockerfile", "Generate Docker Compose", "Generate Dev Container", "Generate Docker Bake", "Generate Complete Environment"];
    let choice = select_option(&options, "What would you like to generate?")?;

    match choice {
        0 => {
            // Dockerfile
            let base_image = prompt("Base image (default: ubuntu:22.04)")?;
            let base_image = if base_image.is_empty() { "ubuntu:22.04".to_string() } else { base_image };
            
            let maintainer = prompt("Maintainer (default: Generated <generated@example.com>)")?;
            let maintainer = if maintainer.is_empty() { "Generated <generated@example.com>".to_string() } else { maintainer };
            
            let packages = prompt("Packages (comma-separated, default: curl,git)")?;
            let packages = if packages.is_empty() { "curl,git".to_string() } else { packages };
            
            let workdir = prompt("Working directory (default: /app)")?;
            let workdir = if workdir.is_empty() { "/app".to_string() } else { workdir };
            
            let entrypoint = prompt("Entrypoint (default: /bin/bash)")?;
            let entrypoint = if entrypoint.is_empty() { "/bin/bash".to_string() } else { entrypoint };
            
            let output = prompt("Output filename (default: Dockerfile)")?;
            let output = if output.is_empty() { "Dockerfile".to_string() } else { output };

            let spec = DockerfileSpec {
                base_image,
                maintainer,
                packages: packages.split(',').map(|s| s.trim().to_string()).collect(),
                workdir,
                entrypoint,
            };
            let rendered = render_template(DOCKERFILE_TEMPLATE, &spec).expect("Failed to render Dockerfile");
            write_to_file(Path::new(&output), &rendered)?;
        }
        1 => {
            // Docker Compose
            let mut services = Vec::new();
            loop {
                println!("\n=== Add Service ===");
                let name = prompt("Service name")?;
                let image = prompt("Image (default: latest)")?;
                let image = if image.is_empty() { format!("{}:latest", name) } else { image };
                
                let ports = prompt("Ports (comma-separated, e.g., 80:80,443:443)")?;
                let ports: Vec<String> = if ports.is_empty() {
                    vec!["80:80".to_string()]
                } else {
                    ports.split(',').map(|s| s.trim().to_string()).collect()
                };

                let volumes = prompt("Volumes (comma-separated, e.g., ./data:/data)")?;
                let volumes: Vec<String> = if volumes.is_empty() {
                    vec!["./data:/data".to_string()]
                } else {
                    volumes.split(',').map(|s| s.trim().to_string()).collect()
                };

                let env_input = prompt("Environment variables (KEY=VALUE,KEY2=VALUE2)")?;
                let environment: Vec<(String, String)> = if env_input.is_empty() {
                    Vec::new()
                } else {
                    env_input
                        .split(',')
                        .filter_map(|pair| {
                            pair.split_once('=').map(|(k, v)| {
                                (k.trim().to_string(), v.trim().to_string())
                            })
                        })
                        .collect()
                };

                services.push(ServiceSpec {
                    name,
                    image,
                    ports,
                    depends_on: Vec::new(),
                    environment,
                    volumes,
                });

                if !confirm("Add another service?")? {
                    break;
                }
            }

            let mut networks_map = HashMap::new();
            if confirm("Add networks?")? {
                loop {
                    let network = prompt("Network name")?;
                    networks_map.insert(network, NetworkConfig {
                        driver: "bridge".to_string(),
                    });
                    if !confirm("Add another network?")? {
                        break;
                    }
                }
            }

            let output = prompt("Output filename (default: docker-compose.yml)")?;
            let output = if output.is_empty() { "docker-compose.yml".to_string() } else { output };

            let spec = DockerComposeSpec {
                services,
                networks: networks_map,
            };
            let rendered = render_template(DOCKER_COMPOSE_TEMPLATE, &spec).expect("Failed to render docker-compose.yml");
            write_to_file(Path::new(&output), &rendered)?;
        }
        2 => {
            // Dev Container
            let name = prompt("Container name (default: Dev Container)")?;
            let name = if name.is_empty() { "Dev Container".to_string() } else { name };
            
            let dockerfile = prompt("Dockerfile path (default: ./Dockerfile)")?;
            let dockerfile = if dockerfile.is_empty() { "./Dockerfile".to_string() } else { dockerfile };
            
            let remote_user = prompt("Remote user (default: vscode)")?;
            let remote_user = if remote_user.is_empty() { "vscode".to_string() } else { remote_user };
            
            let extensions = prompt("VSCode extensions (comma-separated)")?;
            let extensions = if extensions.is_empty() {
                vec!["ms-azuretools.vscode-docker".to_string()]
            } else {
                extensions.split(',').map(|s| s.trim().to_string()).collect()
            };

            let output = prompt("Output filename (default: devcontainer.json)")?;
            let output = if output.is_empty() { "devcontainer.json".to_string() } else { output };

            let spec = DevContainerSpec {
                name,
                dockerfile_path: dockerfile,
                remote_user,
                customizations: DevContainerCustomizations {
                    vscode_extensions: extensions,
                    settings: json!({
                        "editor.formatOnSave": true,
                        "terminal.integrated.shell.linux": "/bin/bash"
                    }),
                },
            };
            let rendered = render_template(DEVCONTAINER_TEMPLATE, &spec).expect("Failed to render devcontainer.json");
            write_to_file(Path::new(&output), &rendered)?;
        }
        3 => {
            // Docker Bake
            let mut targets = Vec::new();
            loop {
                println!("\n=== Add Target ===");
                let name = prompt("Target name")?;
                let context = prompt("Context (default: ./)")?;
                let context = if context.is_empty() { "./".to_string() } else { context };
                
                let dockerfile = prompt("Dockerfile path (default: ./Dockerfile)")?;
                let dockerfile = if dockerfile.is_empty() { "./Dockerfile".to_string() } else { dockerfile };
                
                let tag = prompt("Tag (default: latest)")?;
                let tag = if tag.is_empty() { "latest".to_string() } else { tag };

                targets.push(BakeTarget {
                    name: name.clone(),
                    context,
                    dockerfile,
                    tags: vec![format!("{}:{}", name, tag)],
                });

                if !confirm("Add another target?")? {
                    break;
                }
            }

            let group = prompt("Group name (default: default)")?;
            let group = if group.is_empty() { "default".to_string() } else { group };

            let output = prompt("Output filename (default: docker-bake.hcl)")?;
            let output = if output.is_empty() { "docker-bake.hcl".to_string() } else { output };

            let spec = DockerBakeSpec {
                group_name: group,
                targets,
            };
            let rendered = render_template(DOCKER_BAKE_TEMPLATE, &spec).expect("Failed to render docker-bake.hcl");
            write_to_file(Path::new(&output), &rendered)?;
        }
        4 => {
            // Complete Environment
            let name = prompt("Project name")?;
            
            let language_options = ["Python", "Node.js", "Rust", "Other"];
            let language_idx = select_option(&language_options, "Select programming language:")?;
            let language = language_options[language_idx].to_lowercase();

            let db_options = ["None", "PostgreSQL", "MySQL", "MongoDB"];
            let db_idx = select_option(&db_options, "Select database:")?;
            let database = if db_idx == 0 {
                None
            } else {
                Some(db_options[db_idx].to_lowercase())
            };

            let service_options = ["None", "Redis", "Elasticsearch"];
            let mut selected_services = Vec::new();
            while confirm("Add additional service?")? {
                let service_idx = select_option(&service_options, "Select service:")?;
                if service_idx > 0 {
                    selected_services.push(service_options[service_idx].to_lowercase());
                }
            }
            let services = if selected_services.is_empty() {
                None
            } else {
                Some(selected_services.join(","))
            };

            let output_dir = prompt("Output directory (default: .)")?;
            let output_dir = if output_dir.is_empty() { ".".to_string() } else { output_dir };

            // Call the existing init implementation
            Commands::Init {
                name,
                language,
                database,
                services,
                output_dir,
            }.execute()?;
        }
        _ => unreachable!(),
    }

    println!("\nConfiguration files generated successfully!");
    Ok(())
}

// Add execute method to Commands enum
impl Commands {
    fn execute(self) -> io::Result<()> {
        match self {
            Self::Init { name, language, database, services, output_dir } => {
                // Create output directory if it doesn't exist
                std::fs::create_dir_all(&output_dir)?;

                // 1. Generate Dockerfile based on language
                let (base_image, packages) = match language.as_str() {
                    "python" => ("python:3.12-slim", "python3-pip,python3-dev,build-essential"),
                    "node" => ("node:22-slim", "npm"),
                    "rust" => ("rust:1.83-slim", "cargo"),
                    _ => ("ubuntu:23.10", "curl,git"),
                };

                let dockerfile_spec = DockerfileSpec {
                    base_image: base_image.to_string(),
                    maintainer: "Generated <generated@example.com>".to_string(),
                    packages: packages.split(',').map(|s| s.trim().to_string()).collect(),
                    workdir: "/app".to_string(),
                    entrypoint: "/bin/bash".to_string(),
                };
                let dockerfile = render_template(DOCKERFILE_TEMPLATE, &dockerfile_spec)
                    .expect("Failed to render Dockerfile");
                write_to_file(&Path::new(&output_dir).join("Dockerfile"), &dockerfile)?;

                // 2. Generate docker-compose.yml with services
                let mut service_specs = Vec::new();
                let mut networks_map = HashMap::new();
                networks_map.insert("app_network".to_string(), NetworkConfig {
                    driver: "bridge".to_string(),
                });

                // Add main app service
                service_specs.push(ServiceSpec {
                    name: name.clone(),
                    image: format!("{}:latest", name),
                    ports: vec!["8000:8000".to_string()],
                    depends_on: Vec::new(),
                    environment: Vec::new(),
                    volumes: vec!["./:/app".to_string()],
                });

                // Add database if specified
                if let Some(db) = database {
                    let (db_image, db_port, db_env) = match db.as_str() {
                        "postgres" => ("postgres:latest", "5432:5432", vec![
                            ("POSTGRES_USER".to_string(), "admin".to_string()),
                            ("POSTGRES_PASSWORD".to_string(), "password".to_string()),
                        ]),
                        "mysql" => ("mysql:latest", "3306:3306", vec![
                            ("MYSQL_ROOT_PASSWORD".to_string(), "password".to_string()),
                            ("MYSQL_DATABASE".to_string(), "app".to_string()),
                        ]),
                        "mongodb" => ("mongo:latest", "27017:27017", vec![
                            ("MONGO_INITDB_ROOT_USERNAME".to_string(), "admin".to_string()),
                            ("MONGO_INITDB_ROOT_PASSWORD".to_string(), "password".to_string()),
                        ]),
                        _ => ("postgres:latest", "5432:5432", vec![
                            ("POSTGRES_USER".to_string(), "admin".to_string()),
                            ("POSTGRES_PASSWORD".to_string(), "password".to_string()),
                        ]),
                    };

                    service_specs.push(ServiceSpec {
                        name: "db".to_string(),
                        image: db_image.to_string(),
                        ports: vec![db_port.to_string()],
                        depends_on: Vec::new(),
                        environment: db_env,
                        volumes: vec!["./data:/var/lib/postgresql/data".to_string()],
                    });

                    // Update main app's depends_on
                    service_specs[0].depends_on.push("db".to_string());
                }

                // Add additional services if specified
                if let Some(additional_services) = services {
                    for service in additional_services.split(',') {
                        let service = service.trim();
                        match service {
                            "redis" => {
                                service_specs.push(ServiceSpec {
                                    name: "redis".to_string(),
                                    image: "redis:latest".to_string(),
                                    ports: vec!["6379:6379".to_string()],
                                    depends_on: Vec::new(),
                                    environment: Vec::new(),
                                    volumes: vec!["./redis-data:/data".to_string()],
                                });
                                service_specs[0].depends_on.push("redis".to_string());
                            },
                            "elasticsearch" => {
                                service_specs.push(ServiceSpec {
                                    name: "elasticsearch".to_string(),
                                    image: "elasticsearch:8.7.0".to_string(),
                                    ports: vec!["9200:9200".to_string()],
                                    depends_on: Vec::new(),
                                    environment: vec![
                                        ("discovery.type".to_string(), "single-node".to_string()),
                                        ("ES_JAVA_OPTS".to_string(), "-Xms512m -Xmx512m".to_string()),
                                    ],
                                    volumes: vec!["./es-data:/usr/share/elasticsearch/data".to_string()],
                                });
                                service_specs[0].depends_on.push("elasticsearch".to_string());
                            },
                            _ => (),
                        }
                    }
                }

                let compose_spec = DockerComposeSpec {
                    services: service_specs,
                    networks: networks_map,
                };
                let compose = render_template(DOCKER_COMPOSE_TEMPLATE, &compose_spec)
                    .expect("Failed to render docker-compose.yml");
                write_to_file(&Path::new(&output_dir).join("docker-compose.yml"), &compose)?;

                // 3. Generate devcontainer.json
                let devcontainer_spec = DevContainerSpec {
                    name: format!("{} Dev Container", name),
                    dockerfile_path: "./Dockerfile".to_string(),
                    remote_user: "vscode".to_string(),
                    customizations: DevContainerCustomizations {
                        vscode_extensions: match language.as_str() {
                            "python" => vec![
                                "ms-python.python".to_string(),
                                "ms-python.vscode-pylance".to_string(),
                            ],
                            "node" => vec![
                                "dbaeumer.vscode-eslint".to_string(),
                                "esbenp.prettier-vscode".to_string(),
                            ],
                            "rust" => vec![
                                "rust-lang.rust-analyzer".to_string(),
                                "serayuzgur.crates".to_string(),
                            ],
                            _ => vec![],
                        },
                        settings: json!({
                            "editor.formatOnSave": true,
                            "terminal.integrated.shell.linux": "/bin/bash"
                        }),
                    },
                };
                let devcontainer = render_template(DEVCONTAINER_TEMPLATE, &devcontainer_spec)
                    .expect("Failed to render devcontainer.json");
                write_to_file(&Path::new(&output_dir).join("devcontainer.json"), &devcontainer)?;

                println!("Generated development environment in: {}", output_dir);
                Ok(())
            }
            _ => unreachable!(),
        }
    }
}

fn main() -> io::Result<()> {
    // Check if any command-line arguments were provided
    if std::env::args().len() > 1 {
        // Use the existing CLI parser
        let cli = Cli::parse();
        cli.command.execute()
    } else {
        // No arguments provided, launch interactive mode
        interactive_cli()
    }
}
