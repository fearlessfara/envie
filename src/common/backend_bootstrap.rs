use crate::common::*;
use std::process::Command;

/// Manages the creation and validation of Terraform backend infrastructure (S3 + DynamoDB)
pub struct BackendBootstrap {
    bucket_name: String,
    dynamodb_table: String,
    region: String,
}

impl BackendBootstrap {
    pub fn new(bucket_name: String, dynamodb_table: String, region: String) -> Self {
        Self {
            bucket_name,
            dynamodb_table,
            region,
        }
    }
    
    /// Check if the backend infrastructure exists
    pub fn check_exists(&self) -> Result<BackendStatus> {
        let bucket_exists = self.check_s3_bucket_exists()?;
        let table_exists = self.check_dynamodb_table_exists()?;
        
        Ok(BackendStatus {
            bucket_exists,
            table_exists,
            bucket_name: self.bucket_name.clone(),
            dynamodb_table: self.dynamodb_table.clone(),
        })
    }
    
    /// Create the backend infrastructure (S3 bucket + DynamoDB table)
    pub fn create(&self, no_prompt: bool) -> Result<()> {
        let status = self.check_exists()?;
        
        if status.bucket_exists && status.table_exists {
            println!("✅ Backend infrastructure already exists");
            return Ok(());
        }
        
        // Print what will be created
        println!("\n🏗️  Backend Infrastructure Setup");
        println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
        
        if !status.bucket_exists {
            println!("📦 S3 Bucket to create:");
            println!("   Name: {}", self.bucket_name);
            println!("   Region: {}", self.region);
            println!("   Purpose: Terraform state storage");
        }
        
        if !status.table_exists {
            println!("\n🔒 DynamoDB Table to create:");
            println!("   Name: {}", self.dynamodb_table);
            println!("   Region: {}", self.region);
            println!("   Purpose: Terraform state locking");
        }
        
        println!("\n━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
        
        // Ask for confirmation unless --no-prompt is set
        if !no_prompt {
            println!("\n⚠️  This will create AWS resources that may incur costs.");
            print!("Do you want to proceed? (yes/no): ");
            std::io::Write::flush(&mut std::io::stdout())?;
            
            let mut input = String::new();
            std::io::stdin().read_line(&mut input)?;
            
            let response = input.trim().to_lowercase();
            if response != "yes" && response != "y" {
                return Err(EnvieError::ValidationError(
                    "Backend setup cancelled by user".to_string()
                ));
            }
        }
        
        // Create S3 bucket if needed
        if !status.bucket_exists {
            println!("\n📦 Creating S3 bucket...");
            self.create_s3_bucket()?;
            println!("✅ S3 bucket created successfully");
        }
        
        // Create DynamoDB table if needed
        if !status.table_exists {
            println!("\n🔒 Creating DynamoDB table...");
            self.create_dynamodb_table()?;
            println!("✅ DynamoDB table created successfully");
        }
        
        println!("\n✅ Backend infrastructure is ready!");
        
        Ok(())
    }
    
    fn check_s3_bucket_exists(&self) -> Result<bool> {
        let output = Command::new("aws")
            .args(&[
                "s3api",
                "head-bucket",
                "--bucket",
                &self.bucket_name,
                "--region",
                &self.region,
            ])
            .output();
        
        match output {
            Ok(result) => Ok(result.status.success()),
            Err(_) => Ok(false),
        }
    }
    
    fn check_dynamodb_table_exists(&self) -> Result<bool> {
        let output = Command::new("aws")
            .args(&[
                "dynamodb",
                "describe-table",
                "--table-name",
                &self.dynamodb_table,
                "--region",
                &self.region,
            ])
            .output();
        
        match output {
            Ok(result) => Ok(result.status.success()),
            Err(_) => Ok(false),
        }
    }
    
    fn create_s3_bucket(&self) -> Result<()> {
        // Create bucket
        let create_result = if self.region == "us-east-1" {
            // us-east-1 doesn't need location constraint
            Command::new("aws")
                .args(&[
                    "s3api",
                    "create-bucket",
                    "--bucket",
                    &self.bucket_name,
                    "--region",
                    &self.region,
                ])
                .output()
        } else {
            Command::new("aws")
                .args(&[
                    "s3api",
                    "create-bucket",
                    "--bucket",
                    &self.bucket_name,
                    "--region",
                    &self.region,
                    "--create-bucket-configuration",
                    &format!("LocationConstraint={}", self.region),
                ])
                .output()
        };
        
        match create_result {
            Ok(output) if output.status.success() => {
                // Enable versioning
                let _ = Command::new("aws")
                    .args(&[
                        "s3api",
                        "put-bucket-versioning",
                        "--bucket",
                        &self.bucket_name,
                        "--versioning-configuration",
                        "Status=Enabled",
                        "--region",
                        &self.region,
                    ])
                    .output();
                
                // Enable encryption
                let _ = Command::new("aws")
                    .args(&[
                        "s3api",
                        "put-bucket-encryption",
                        "--bucket",
                        &self.bucket_name,
                        "--server-side-encryption-configuration",
                        r#"{"Rules":[{"ApplyServerSideEncryptionByDefault":{"SSEAlgorithm":"AES256"}}]}"#,
                        "--region",
                        &self.region,
                    ])
                    .output();
                
                // Block public access
                let _ = Command::new("aws")
                    .args(&[
                        "s3api",
                        "put-public-access-block",
                        "--bucket",
                        &self.bucket_name,
                        "--public-access-block-configuration",
                        "BlockPublicAcls=true,IgnorePublicAcls=true,BlockPublicPolicy=true,RestrictPublicBuckets=true",
                        "--region",
                        &self.region,
                    ])
                    .output();
                
                Ok(())
            }
            Ok(output) => {
                let error_msg = String::from_utf8_lossy(&output.stderr);
                Err(EnvieError::ValidationError(
                    format!("Failed to create S3 bucket: {}", error_msg)
                ))
            }
            Err(e) => Err(EnvieError::ValidationError(
                format!("Failed to execute AWS CLI: {}", e)
            )),
        }
    }
    
    fn create_dynamodb_table(&self) -> Result<()> {
        let output = Command::new("aws")
            .args(&[
                "dynamodb",
                "create-table",
                "--table-name",
                &self.dynamodb_table,
                "--attribute-definitions",
                "AttributeName=LockID,AttributeType=S",
                "--key-schema",
                "AttributeName=LockID,KeyType=HASH",
                "--billing-mode",
                "PAY_PER_REQUEST",
                "--region",
                &self.region,
            ])
            .output();
        
        match output {
            Ok(result) if result.status.success() => {
                // Wait for table to be active
                println!("   Waiting for table to be active...");
                let _ = Command::new("aws")
                    .args(&[
                        "dynamodb",
                        "wait",
                        "table-exists",
                        "--table-name",
                        &self.dynamodb_table,
                        "--region",
                        &self.region,
                    ])
                    .output();
                
                Ok(())
            }
            Ok(result) => {
                let error_msg = String::from_utf8_lossy(&result.stderr);
                Err(EnvieError::ValidationError(
                    format!("Failed to create DynamoDB table: {}", error_msg)
                ))
            }
            Err(e) => Err(EnvieError::ValidationError(
                format!("Failed to execute AWS CLI: {}", e)
            )),
        }
    }
}

#[derive(Debug)]
pub struct BackendStatus {
    pub bucket_exists: bool,
    pub table_exists: bool,
    pub bucket_name: String,
    pub dynamodb_table: String,
}

impl BackendStatus {
    pub fn is_ready(&self) -> bool {
        self.bucket_exists && self.table_exists
    }
}

