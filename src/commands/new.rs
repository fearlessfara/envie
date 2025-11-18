use crate::common::Result;
use crate::common::service_config::{ServiceConfig, ModuleConfig, DependencyReference, StateManagement};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::fs;

#[derive(Debug, Clone)]
pub struct NewOptions {
    pub name: String,
    pub template: Option<String>,
    pub path: Option<String>,
    pub no_prompt: bool,
    pub verbose: bool,
    pub modules: Vec<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum UnitTemplate {
    Simple,
    WithModules,
    Networking,
    Database,
    Api,
    Compute,
}

impl UnitTemplate {
    fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "simple" => Some(UnitTemplate::Simple),
            "with-modules" | "modules" => Some(UnitTemplate::WithModules),
            "networking" | "network" => Some(UnitTemplate::Networking),
            "database" | "db" => Some(UnitTemplate::Database),
            "api" => Some(UnitTemplate::Api),
            "compute" | "lambda" => Some(UnitTemplate::Compute),
            _ => None,
        }
    }

    fn description(&self) -> &str {
        match self {
            UnitTemplate::Simple => "Simple unit with single main.tf",
            UnitTemplate::WithModules => "Unit with example modules structure",
            UnitTemplate::Networking => "Networking service (VPC, subnets, security groups)",
            UnitTemplate::Database => "Database service (DynamoDB, RDS)",
            UnitTemplate::Api => "API service (Lambda, API Gateway, Step Functions)",
            UnitTemplate::Compute => "Compute service (Lambda functions)",
        }
    }
}

pub struct NewCommand {
    working_directory: PathBuf,
}

impl NewCommand {
    pub fn new(working_directory: PathBuf) -> Self {
        Self { working_directory }
    }

    pub async fn execute(&self, options: NewOptions) -> Result<()> {
        if options.verbose {
            println!("🚀 Creating new Envie unit '{}'...", options.name);
        }

        // Determine template
        let template = self.determine_template(&options)?;

        if options.verbose {
            println!("📋 Using template: {} - {}",
                match template {
                    UnitTemplate::Simple => "simple",
                    UnitTemplate::WithModules => "with-modules",
                    UnitTemplate::Networking => "networking",
                    UnitTemplate::Database => "database",
                    UnitTemplate::Api => "api",
                    UnitTemplate::Compute => "compute",
                },
                template.description()
            );
        }

        // Determine unit path
        let unit_path = self.determine_unit_path(&options)?;

        // Check if path already exists
        if unit_path.exists() {
            return Err(crate::common::EnvieError::ProcessError(
                format!("Unit path already exists: {}", unit_path.display())
            ));
        }

        // Create unit directory
        fs::create_dir_all(&unit_path)?;

        // Create unit based on template
        match template {
            UnitTemplate::Simple => self.create_simple_unit(&unit_path, &options.name)?,
            UnitTemplate::WithModules => self.create_unit_with_modules(&unit_path, &options.name, &options.modules)?,
            UnitTemplate::Networking => self.create_networking_unit(&unit_path, &options.name)?,
            UnitTemplate::Database => self.create_database_unit(&unit_path, &options.name)?,
            UnitTemplate::Api => self.create_api_unit(&unit_path, &options.name)?,
            UnitTemplate::Compute => self.create_compute_unit(&unit_path, &options.name)?,
        }

        println!("\n✅ Unit '{}' created successfully!", options.name);
        println!("\n📁 Created at: {}", unit_path.display());
        println!("\n🚀 Next steps:");
        println!("  1. Customize the envie.yaml configuration");
        println!("  2. Add your Terraform code to the modules");
        println!("  3. Add dependencies if needed");
        println!("  4. Run 'envie deploy --unit {} --env <env-id>' to deploy", options.name);

        Ok(())
    }

    fn determine_template(&self, options: &NewOptions) -> Result<UnitTemplate> {
        if let Some(template_str) = &options.template {
            UnitTemplate::from_str(template_str)
                .ok_or_else(|| crate::common::EnvieError::ProcessError(
                    format!("Invalid template '{}'. Valid templates: simple, with-modules, networking, database, api, compute", template_str)
                ))
        } else if !options.modules.is_empty() {
            Ok(UnitTemplate::WithModules)
        } else if options.no_prompt {
            Ok(UnitTemplate::Simple)
        } else {
            self.prompt_for_template()
        }
    }

    fn prompt_for_template(&self) -> Result<UnitTemplate> {
        println!("Select a template:");
        println!("  1. simple         - Simple unit with single main.tf");
        println!("  2. with-modules   - Unit with example modules structure");
        println!("  3. networking     - Networking service (VPC, subnets, security groups)");
        println!("  4. database       - Database service (DynamoDB, RDS)");
        println!("  5. api            - API service (Lambda, API Gateway, Step Functions)");
        println!("  6. compute        - Compute service (Lambda functions)");
        print!("\nChoice [1]: ");
        io::stdout().flush()?;

        let mut input = String::new();
        io::stdin().read_line(&mut input)?;
        let choice = input.trim();

        match choice {
            "" | "1" => Ok(UnitTemplate::Simple),
            "2" => Ok(UnitTemplate::WithModules),
            "3" => Ok(UnitTemplate::Networking),
            "4" => Ok(UnitTemplate::Database),
            "5" => Ok(UnitTemplate::Api),
            "6" => Ok(UnitTemplate::Compute),
            _ => Err(crate::common::EnvieError::ProcessError("Invalid choice".to_string())),
        }
    }

    fn determine_unit_path(&self, options: &NewOptions) -> Result<PathBuf> {
        if let Some(path_str) = &options.path {
            Ok(PathBuf::from(path_str))
        } else {
            // Default to services/<name>
            Ok(self.working_directory.join("services").join(&options.name))
        }
    }

    fn create_simple_unit(&self, unit_path: &Path, name: &str) -> Result<()> {
        // Create envie.yaml
        let config = ServiceConfig {
            name: name.to_string(),
            description: format!("Simple {} service", name),
            modules: vec![],
            dependencies: vec![],
        };

        let yaml_content = serde_yaml::to_string(&config)?;
        fs::write(unit_path.join("envie.yaml"), yaml_content)?;

        // Create main.tf
        let main_tf = format!(r#"# {} Service
# Main Terraform configuration

terraform {{
  required_version = ">= 1.0"
}}

variable "envie_workspace" {{
  type        = string
  description = "The Envie workspace name"
}}

variable "envie_environment" {{
  type        = string
  description = "The Envie environment identifier"
}}

locals {{
  service_name = "{}"
  workspace    = var.envie_workspace
  environment  = var.envie_environment
}}

# Add your resources here
# Example:
# resource "null_resource" "example" {{
#   provisioner "local-exec" {{
#     command = "echo 'Hello from {}'"
#   }}
# }}

output "service_name" {{
  value       = local.service_name
  description = "The name of this service"
}}

output "workspace" {{
  value       = local.workspace
  description = "The workspace this service is deployed to"
}}
"#, name, name, name);

        fs::write(unit_path.join("main.tf"), main_tf)?;

        println!("  ✓ Created envie.yaml");
        println!("  ✓ Created main.tf");

        Ok(())
    }

    fn create_unit_with_modules(&self, unit_path: &Path, name: &str, module_names: &[String]) -> Result<()> {
        let modules_to_create = if module_names.is_empty() {
            vec!["module1".to_string(), "module2".to_string()]
        } else {
            module_names.to_vec()
        };

        // Create modules directory
        let modules_dir = unit_path.join("modules");
        fs::create_dir_all(&modules_dir)?;

        // Create module configs
        let mut module_configs = Vec::new();
        for (i, module_name) in modules_to_create.iter().enumerate() {
            let module_dir = modules_dir.join(module_name);
            fs::create_dir_all(&module_dir)?;

            // Create main.tf for module
            let main_tf = format!(r#"# {} Module

variable "envie_workspace" {{
  type = string
}}

locals {{
  module_name = "{}"
  workspace   = var.envie_workspace
}}

# Add your resources here

output "module_name" {{
  value       = local.module_name
  description = "The name of this module"
}}
"#, module_name, module_name);

            fs::write(module_dir.join("main.tf"), main_tf)?;

            // Add module to config (modules after first one depend on previous)
            let dependencies = if i > 0 {
                vec![DependencyReference {
                    path: format!("./{}", modules_to_create[i - 1]),
                    environment: "ephemeral".to_string(),
                }]
            } else {
                vec![]
            };

            module_configs.push(ModuleConfig {
                name: module_name.clone(),
                description: format!("{} module", module_name),
                path: format!("modules/{}", module_name),
                dependencies,
                state_management: StateManagement::Service,
            });

            println!("  ✓ Created modules/{}/main.tf", module_name);
        }

        // Create envie.yaml
        let config = ServiceConfig {
            name: name.to_string(),
            description: format!("{} service with modules", name),
            modules: module_configs,
            dependencies: vec![],
        };

        let yaml_content = serde_yaml::to_string(&config)?;
        fs::write(unit_path.join("envie.yaml"), yaml_content)?;

        println!("  ✓ Created envie.yaml");

        Ok(())
    }

    fn create_networking_unit(&self, unit_path: &Path, name: &str) -> Result<()> {
        let modules_dir = unit_path.join("modules");
        fs::create_dir_all(&modules_dir)?;

        // Create VPC module
        fs::create_dir_all(modules_dir.join("vpc"))?;
        let vpc_tf = r#"# VPC Module

variable "envie_workspace" {
  type = string
}

variable "cidr_block" {
  type    = string
  default = "10.0.0.0/16"
}

resource "aws_vpc" "main" {
  cidr_block           = var.cidr_block
  enable_dns_hostnames = true
  enable_dns_support   = true

  tags = {
    Name      = "${var.envie_workspace}-vpc"
    Workspace = var.envie_workspace
  }
}

output "vpc_id" {
  value       = aws_vpc.main.id
  description = "The VPC ID"
}

output "vpc_cidr" {
  value       = aws_vpc.main.cidr_block
  description = "The VPC CIDR block"
}
"#;
        fs::write(modules_dir.join("vpc").join("main.tf"), vpc_tf)?;

        // Create subnets module
        fs::create_dir_all(modules_dir.join("subnets"))?;
        let subnets_tf = r#"# Subnets Module

variable "envie_workspace" {
  type = string
}

variable "vpc_id" {
  type        = string
  description = "VPC ID from vpc module"
}

variable "availability_zones" {
  type    = list(string)
  default = ["us-east-1a", "us-east-1b"]
}

resource "aws_subnet" "public" {
  count             = length(var.availability_zones)
  vpc_id            = var.vpc_id
  cidr_block        = cidrsubnet("10.0.0.0/16", 8, count.index)
  availability_zone = var.availability_zones[count.index]

  tags = {
    Name      = "${var.envie_workspace}-public-${count.index + 1}"
    Workspace = var.envie_workspace
  }
}

output "subnet_ids" {
  value       = aws_subnet.public[*].id
  description = "List of subnet IDs"
}
"#;
        fs::write(modules_dir.join("subnets").join("main.tf"), subnets_tf)?;

        // Create envie.yaml
        let config = ServiceConfig {
            name: name.to_string(),
            description: "Networking infrastructure".to_string(),
            modules: vec![
                ModuleConfig {
                    name: "vpc".to_string(),
                    description: "VPC configuration".to_string(),
                    path: "modules/vpc".to_string(),
                    dependencies: vec![],
                    state_management: StateManagement::Service,
                },
                ModuleConfig {
                    name: "subnets".to_string(),
                    description: "Subnet configuration".to_string(),
                    path: "modules/subnets".to_string(),
                    dependencies: vec![
                        DependencyReference {
                            path: "./vpc".to_string(),
                            environment: "ephemeral".to_string(),
                        },
                    ],
                    state_management: StateManagement::Service,
                },
            ],
            dependencies: vec![],
        };

        let yaml_content = serde_yaml::to_string(&config)?;
        fs::write(unit_path.join("envie.yaml"), yaml_content)?;

        println!("  ✓ Created envie.yaml");
        println!("  ✓ Created modules/vpc/main.tf");
        println!("  ✓ Created modules/subnets/main.tf");

        Ok(())
    }

    fn create_database_unit(&self, unit_path: &Path, name: &str) -> Result<()> {
        let modules_dir = unit_path.join("modules");
        fs::create_dir_all(&modules_dir)?;

        // Create DynamoDB module
        fs::create_dir_all(modules_dir.join("dynamodb"))?;
        let dynamodb_tf = r#"# DynamoDB Module

variable "envie_workspace" {
  type = string
}

variable "table_name" {
  type    = string
  default = "main-table"
}

resource "aws_dynamodb_table" "main" {
  name           = "${var.envie_workspace}-${var.table_name}"
  billing_mode   = "PAY_PER_REQUEST"
  hash_key       = "id"

  attribute {
    name = "id"
    type = "S"
  }

  tags = {
    Name      = "${var.envie_workspace}-${var.table_name}"
    Workspace = var.envie_workspace
  }
}

output "table_name" {
  value       = aws_dynamodb_table.main.name
  description = "DynamoDB table name"
}

output "table_arn" {
  value       = aws_dynamodb_table.main.arn
  description = "DynamoDB table ARN"
}
"#;
        fs::write(modules_dir.join("dynamodb").join("main.tf"), dynamodb_tf)?;

        // Create envie.yaml
        let config = ServiceConfig {
            name: name.to_string(),
            description: "Database infrastructure".to_string(),
            modules: vec![
                ModuleConfig {
                    name: "dynamodb".to_string(),
                    description: "DynamoDB table".to_string(),
                    path: "modules/dynamodb".to_string(),
                    dependencies: vec![],
                    state_management: StateManagement::Dedicated,
                },
            ],
            dependencies: vec![],
        };

        let yaml_content = serde_yaml::to_string(&config)?;
        fs::write(unit_path.join("envie.yaml"), yaml_content)?;

        println!("  ✓ Created envie.yaml");
        println!("  ✓ Created modules/dynamodb/main.tf");

        Ok(())
    }

    fn create_api_unit(&self, unit_path: &Path, name: &str) -> Result<()> {
        let modules_dir = unit_path.join("modules");
        fs::create_dir_all(&modules_dir)?;

        // Create Lambda module
        fs::create_dir_all(modules_dir.join("lambda"))?;
        let lambda_tf = r#"# Lambda Module

variable "envie_workspace" {
  type = string
}

variable "function_name" {
  type    = string
  default = "api-handler"
}

resource "aws_lambda_function" "api" {
  function_name = "${var.envie_workspace}-${var.function_name}"
  role          = aws_iam_role.lambda.arn
  handler       = "index.handler"
  runtime       = "nodejs18.x"

  # Placeholder for function code
  filename      = "lambda.zip"

  tags = {
    Name      = "${var.envie_workspace}-${var.function_name}"
    Workspace = var.envie_workspace
  }
}

resource "aws_iam_role" "lambda" {
  name = "${var.envie_workspace}-lambda-role"

  assume_role_policy = jsonencode({
    Version = "2012-10-17"
    Statement = [{
      Action = "sts:AssumeRole"
      Effect = "Allow"
      Principal = {
        Service = "lambda.amazonaws.com"
      }
    }]
  })
}

output "function_arn" {
  value       = aws_lambda_function.api.arn
  description = "Lambda function ARN"
}

output "function_name" {
  value       = aws_lambda_function.api.function_name
  description = "Lambda function name"
}
"#;
        fs::write(modules_dir.join("lambda").join("main.tf"), lambda_tf)?;

        // Create API Gateway module
        fs::create_dir_all(modules_dir.join("gateway"))?;
        let gateway_tf = r#"# API Gateway Module

variable "envie_workspace" {
  type = string
}

variable "lambda_function_arn" {
  type        = string
  description = "Lambda function ARN from lambda module"
}

resource "aws_apigatewayv2_api" "main" {
  name          = "${var.envie_workspace}-api"
  protocol_type = "HTTP"

  tags = {
    Name      = "${var.envie_workspace}-api"
    Workspace = var.envie_workspace
  }
}

output "api_endpoint" {
  value       = aws_apigatewayv2_api.main.api_endpoint
  description = "API Gateway endpoint URL"
}
"#;
        fs::write(modules_dir.join("gateway").join("main.tf"), gateway_tf)?;

        // Create envie.yaml
        let config = ServiceConfig {
            name: name.to_string(),
            description: "API service with Lambda and API Gateway".to_string(),
            modules: vec![
                ModuleConfig {
                    name: "lambda".to_string(),
                    description: "Lambda functions".to_string(),
                    path: "modules/lambda".to_string(),
                    dependencies: vec![],
                    state_management: StateManagement::Dedicated,
                },
                ModuleConfig {
                    name: "gateway".to_string(),
                    description: "API Gateway configuration".to_string(),
                    path: "modules/gateway".to_string(),
                    dependencies: vec![
                        DependencyReference {
                            path: "./lambda".to_string(),
                            environment: "ephemeral".to_string(),
                        },
                    ],
                    state_management: StateManagement::Service,
                },
            ],
            dependencies: vec![],
        };

        let yaml_content = serde_yaml::to_string(&config)?;
        fs::write(unit_path.join("envie.yaml"), yaml_content)?;

        println!("  ✓ Created envie.yaml");
        println!("  ✓ Created modules/lambda/main.tf");
        println!("  ✓ Created modules/gateway/main.tf");

        Ok(())
    }

    fn create_compute_unit(&self, unit_path: &Path, name: &str) -> Result<()> {
        let modules_dir = unit_path.join("modules");
        fs::create_dir_all(&modules_dir)?;

        // Create Lambda module
        fs::create_dir_all(modules_dir.join("functions"))?;
        let lambda_tf = r#"# Lambda Functions Module

variable "envie_workspace" {
  type = string
}

resource "aws_lambda_function" "worker" {
  function_name = "${var.envie_workspace}-worker"
  role          = aws_iam_role.lambda.arn
  handler       = "index.handler"
  runtime       = "nodejs18.x"

  filename      = "lambda.zip"

  tags = {
    Name      = "${var.envie_workspace}-worker"
    Workspace = var.envie_workspace
  }
}

resource "aws_iam_role" "lambda" {
  name = "${var.envie_workspace}-lambda-role"

  assume_role_policy = jsonencode({
    Version = "2012-10-17"
    Statement = [{
      Action = "sts:AssumeRole"
      Effect = "Allow"
      Principal = {
        Service = "lambda.amazonaws.com"
      }
    }]
  })
}

output "function_arn" {
  value       = aws_lambda_function.worker.arn
  description = "Lambda function ARN"
}
"#;
        fs::write(modules_dir.join("functions").join("main.tf"), lambda_tf)?;

        // Create envie.yaml
        let config = ServiceConfig {
            name: name.to_string(),
            description: "Compute service with Lambda functions".to_string(),
            modules: vec![
                ModuleConfig {
                    name: "functions".to_string(),
                    description: "Lambda functions".to_string(),
                    path: "modules/functions".to_string(),
                    dependencies: vec![],
                    state_management: StateManagement::Dedicated,
                },
            ],
            dependencies: vec![],
        };

        let yaml_content = serde_yaml::to_string(&config)?;
        fs::write(unit_path.join("envie.yaml"), yaml_content)?;

        println!("  ✓ Created envie.yaml");
        println!("  ✓ Created modules/functions/main.tf");

        Ok(())
    }
}
