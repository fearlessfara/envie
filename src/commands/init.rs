use crate::common::Result;
use crate::common::service_config::{ProjectInfo, WorkspaceConfig, ServiceConfig, ModuleConfig, ServiceDiscovery};
use std::collections::HashMap;
use std::io::{self, Write};
use std::path::Path;

#[derive(Debug, Clone)]
pub struct InitOptions {
    pub name: Option<String>,
    pub description: Option<String>,
    pub no_prompt: bool,
    pub verbose: bool,
}

pub struct InitCommand {
    working_directory: std::path::PathBuf,
}

impl InitCommand {
    pub fn new(working_directory: std::path::PathBuf) -> Self {
        Self { working_directory }
    }

    pub async fn execute(&self, options: InitOptions) -> Result<()> {
        if options.verbose {
            println!("🚀 Initializing Envie project...");
        }

        // Check if already initialized
        if self.is_already_initialized()? {
            if !options.no_prompt {
                print!("Project already initialized. Continue anyway? [y/N]: ");
                io::stdout().flush()?;
                let mut input = String::new();
                io::stdin().read_line(&mut input)?;
                if !input.trim().to_lowercase().starts_with('y') {
                    println!("Initialization cancelled.");
                    return Ok(());
                }
            }
        }

        // Get project information
        let project_info = self.get_project_info(&options)?;

        // Create workspace configuration
        let workspace_config = self.create_workspace_config(&project_info)?;
        
        // Write workspace.envie
        self.write_workspace_config(&workspace_config)?;

        // Create services directory structure
        self.create_services_structure()?;

        // Create example services
        self.create_example_services()?;

        // Create .gitignore entries
        self.update_gitignore()?;

        // Create README
        self.create_readme(&project_info)?;

        println!("\n✅ Envie project initialized successfully!");
        println!("\n📁 Project structure created:");
        println!("  ├── workspace.envie          # Global project configuration");
        println!("  ├── services/                # Service directory");
        println!("  │   ├── networking/          # Example networking service");
        println!("  │   ├── database/            # Example database service");
        println!("  │   └── api/                 # Example API service");
        println!("  └── README.md                # Project documentation");
        
        println!("\n🚀 Next steps:");
        println!("  1. Review and customize workspace.envie");
        println!("  2. Add your services to the services/ directory");
        println!("  3. Run 'envie deploy --service <name> --merge-request <id>' to deploy");

        Ok(())
    }

    fn is_already_initialized(&self) -> Result<bool> {
        let workspace_envie = self.working_directory.join("workspace.envie");
        Ok(workspace_envie.exists())
    }

    fn get_project_info(&self, options: &InitOptions) -> Result<ProjectInfo> {
        let name = if let Some(name) = &options.name {
            name.clone()
        } else if options.no_prompt {
            "my-envie-project".to_string()
        } else {
            print!("Project name [my-envie-project]: ");
            io::stdout().flush()?;
            let mut input = String::new();
            io::stdin().read_line(&mut input)?;
            let name = input.trim();
            if name.is_empty() {
                "my-envie-project".to_string()
            } else {
                name.to_string()
            }
        };

        let description = if let Some(description) = &options.description {
            description.clone()
        } else if options.no_prompt {
            "An Envie-managed Terraform project".to_string()
        } else {
            print!("Project description [An Envie-managed Terraform project]: ");
            io::stdout().flush()?;
            let mut input = String::new();
            io::stdin().read_line(&mut input)?;
            let description = input.trim();
            if description.is_empty() {
                "An Envie-managed Terraform project".to_string()
            } else {
                description.to_string()
            }
        };

        Ok(ProjectInfo {
            name,
            description,
        })
    }

    fn create_workspace_config(&self, project_info: &ProjectInfo) -> Result<WorkspaceConfig> {
        Ok(WorkspaceConfig {
            version: "1.0".to_string(),
            project: Some(project_info.clone()),
            services: vec![
                ServiceDiscovery {
                    name: Some("networking".to_string()),
                    path: "services/networking".to_string(),
                },
                ServiceDiscovery {
                    name: Some("database".to_string()),
                    path: "services/database".to_string(),
                },
                ServiceDiscovery {
                    name: Some("api".to_string()),
                    path: "services/api".to_string(),
                },
            ],
            defaults: HashMap::new(),
        })
    }

    fn write_workspace_config(&self, config: &WorkspaceConfig) -> Result<()> {
        let workspace_envie = self.working_directory.join("workspace.envie");
        let content = serde_yaml::to_string(config)?;
        std::fs::write(workspace_envie, content)?;
        Ok(())
    }

    fn create_services_structure(&self) -> Result<()> {
        let services_dir = self.working_directory.join("services");
        std::fs::create_dir_all(&services_dir)?;
        Ok(())
    }

    fn create_example_services(&self) -> Result<()> {
        // Create networking service
        self.create_networking_service()?;
        
        // Create database service
        self.create_database_service()?;
        
        // Create API service
        self.create_api_service()?;

        Ok(())
    }

    fn create_networking_service(&self) -> Result<()> {
        let service_dir = self.working_directory.join("services").join("networking");
        std::fs::create_dir_all(&service_dir)?;
        std::fs::create_dir_all(service_dir.join("modules").join("vpc"))?;
        std::fs::create_dir_all(service_dir.join("modules").join("subnets"))?;
        std::fs::create_dir_all(service_dir.join("modules").join("security-groups"))?;

        // Create .envie file
        let config = ServiceConfig {
            name: "networking".to_string(),
            description: "Networking infrastructure with VPC, subnets, and security groups".to_string(),
            modules: vec![
                ModuleConfig {
                    name: "vpc".to_string(),
                    description: "VPC configuration".to_string(),
                    path: "modules/vpc".to_string(),
                    depends: vec![],
                },
                ModuleConfig {
                    name: "subnets".to_string(),
                    description: "Subnet configuration".to_string(),
                    path: "modules/subnets".to_string(),
                    depends: vec![
                        crate::common::service_config::DependencyReference {
                            path: "./vpc".to_string(),
                            environment: "ephemeral".to_string(),
                        },
                    ],
                },
                ModuleConfig {
                    name: "security-groups".to_string(),
                    description: "Security group configuration".to_string(),
                    path: "modules/security-groups".to_string(),
                    depends: vec![
                        crate::common::service_config::DependencyReference {
                            path: "./vpc".to_string(),
                            environment: "ephemeral".to_string(),
                        },
                    ],
                },
            ],
            depends: vec![],
        };

        let content = serde_yaml::to_string(&config)?;
        std::fs::write(service_dir.join(".envie"), content)?;

        // Create example Terraform files
        self.create_example_terraform_files(&service_dir)?;

        Ok(())
    }

    fn create_database_service(&self) -> Result<()> {
        let service_dir = self.working_directory.join("services").join("database");
        std::fs::create_dir_all(&service_dir)?;
        std::fs::create_dir_all(service_dir.join("modules").join("dynamodb"))?;
        std::fs::create_dir_all(service_dir.join("modules").join("rds"))?;

        // Create .envie file
        let config = ServiceConfig {
            name: "database".to_string(),
            description: "Database layer with DynamoDB and RDS".to_string(),
            modules: vec![
                ModuleConfig {
                    name: "dynamodb".to_string(),
                    description: "DynamoDB table configuration".to_string(),
                    path: "modules/dynamodb".to_string(),
                    depends: vec![
                        crate::common::service_config::DependencyReference {
                            path: "../networking/modules/vpc".to_string(),
                            environment: "ephemeral".to_string(),
                        },
                    ],
                },
                ModuleConfig {
                    name: "rds".to_string(),
                    description: "RDS database configuration".to_string(),
                    path: "modules/rds".to_string(),
                    depends: vec![
                        crate::common::service_config::DependencyReference {
                            path: "../networking/modules/vpc".to_string(),
                            environment: "ephemeral".to_string(),
                        },
                        crate::common::service_config::DependencyReference {
                            path: "../networking/modules/security-groups".to_string(),
                            environment: "ephemeral".to_string(),
                        },
                    ],
                },
            ],
            depends: vec!["../networking".to_string()],
        };

        let content = serde_yaml::to_string(&config)?;
        std::fs::write(service_dir.join(".envie"), content)?;

        // Create example Terraform files
        self.create_example_terraform_files(&service_dir)?;

        Ok(())
    }

    fn create_api_service(&self) -> Result<()> {
        let service_dir = self.working_directory.join("services").join("api");
        std::fs::create_dir_all(&service_dir)?;
        std::fs::create_dir_all(service_dir.join("modules").join("lambda"))?;
        std::fs::create_dir_all(service_dir.join("modules").join("step-functions"))?;
        std::fs::create_dir_all(service_dir.join("modules").join("gateway"))?;

        // Create .envie file
        let config = ServiceConfig {
            name: "api".to_string(),
            description: "API layer with Lambda, Step Functions, and API Gateway".to_string(),
            modules: vec![
                ModuleConfig {
                    name: "lambda".to_string(),
                    description: "Lambda function for API handler".to_string(),
                    path: "modules/lambda".to_string(),
                    depends: vec![
                        crate::common::service_config::DependencyReference {
                            path: "../../database/modules/dynamodb".to_string(),
                            environment: "stable.sandbox".to_string(),
                        },
                        crate::common::service_config::DependencyReference {
                            path: "../../networking/modules/vpc".to_string(),
                            environment: "ephemeral".to_string(),
                        },
                    ],
                },
                ModuleConfig {
                    name: "step-functions".to_string(),
                    description: "Step Functions state machine".to_string(),
                    path: "modules/step-functions".to_string(),
                    depends: vec![
                        crate::common::service_config::DependencyReference {
                            path: "./lambda".to_string(),
                            environment: "ephemeral".to_string(),
                        },
                    ],
                },
                ModuleConfig {
                    name: "gateway".to_string(),
                    description: "API Gateway configuration".to_string(),
                    path: "modules/gateway".to_string(),
                    depends: vec![
                        crate::common::service_config::DependencyReference {
                            path: "./step-functions".to_string(),
                            environment: "ephemeral".to_string(),
                        },
                    ],
                },
            ],
            depends: vec!["../database".to_string(), "../networking".to_string()],
        };

        let content = serde_yaml::to_string(&config)?;
        std::fs::write(service_dir.join(".envie"), content)?;

        // Create example Terraform files
        self.create_example_terraform_files(&service_dir)?;

        Ok(())
    }

    fn create_example_terraform_files(&self, service_dir: &Path) -> Result<()> {
        // Create a simple main.tf file for each module
        for module_dir in std::fs::read_dir(service_dir.join("modules"))? {
            let module_dir = module_dir?;
            if module_dir.file_type()?.is_dir() {
                let main_tf = module_dir.path().join("main.tf");
                let content = format!(
                    r#"# {module_name} Module
# This is an example Terraform module for {module_name}

resource "null_resource" "example" {{
  provisioner "local-exec" {{
    command = "echo 'Hello from {module_name} module'"
  }}
}}

output "example_output" {{
  value = "This is output from {module_name} module"
  description = "Example output from {module_name} module"
}}
"#,
                    module_name = module_dir.file_name().to_string_lossy()
                );
                std::fs::write(main_tf, content)?;
            }
        }
        Ok(())
    }

    fn update_gitignore(&self) -> Result<()> {
        let gitignore_path = self.working_directory.join(".gitignore");
        let mut gitignore_content = if gitignore_path.exists() {
            std::fs::read_to_string(&gitignore_path)?
        } else {
            String::new()
        };

        let envie_entries = "\n# Envie generated files\n.envie-remote-state.tf\n.envie-variables.tf\n.terraform/\n.terraform.lock.hcl\n*.tfstate\n*.tfstate.*\n";

        if !gitignore_content.contains(".envie-remote-state.tf") {
            gitignore_content.push_str(envie_entries);
            std::fs::write(gitignore_path, gitignore_content)?;
        }

        Ok(())
    }

    fn create_readme(&self, project_info: &ProjectInfo) -> Result<()> {
        let readme_content = format!(
            r#"# {project_name}

{project_description}

This project is managed by [Envie](https://github.com/your-org/envie), a tool for managing multiple ephemeral environments in Terraform with layered dependencies and resource sharing.

## Project Structure

```
├── workspace.envie          # Global project configuration
├── services/                # Service directory
│   ├── networking/          # Networking infrastructure
│   │   ├── .envie          # Service configuration
│   │   └── modules/        # Terraform modules
│   ├── database/            # Database layer
│   │   ├── .envie          # Service configuration
│   │   └── modules/        # Terraform modules
│   └── api/                 # API layer
│       ├── .envie          # Service configuration
│       └── modules/        # Terraform modules
└── README.md                # This file
```

## Quick Start

1. **Deploy a service:**
   ```bash
   envie deploy --service networking --merge-request 123
   ```

2. **Deploy with environment overrides:**
   ```bash
   envie deploy --service api --merge-request 123 -E database:stable.sandbox
   ```

3. **List available services:**
   ```bash
   envie list
   ```

## Configuration

- `workspace.envie`: Global project configuration with environment definitions
- `services/*/.envie`: Per-service configuration with module dependencies

## Environments

- **Ephemeral**: Temporary environments for development (e.g., MR 123)
- **Stable**: Long-lived environments for shared resources
  - `stable.sandbox`: Development sandbox
  - `stable.staging`: Staging environment
  - `stable.production`: Production environment

## Dependencies

Services can depend on other services using relative paths:
- `../networking`: Reference to networking service
- `./lambda`: Reference to lambda module within same service

## More Information

For more information about Envie, see the [documentation](https://github.com/your-org/envie/docs).
"#,
            project_name = project_info.name,
            project_description = project_info.description
        );

        std::fs::write(self.working_directory.join("README.md"), readme_content)?;
        Ok(())
    }
}