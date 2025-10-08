use thiserror::Error;

#[derive(Error, Debug)]
pub enum EnvieError {
    #[error("Terraform error: {0}")]
    TerraformError(String),

    #[error("Configuration error: {0}")]
    ConfigError(String),

    #[error("File system error: {0}")]
    FileSystemError(String),

    #[error("Process execution error: {0}")]
    ProcessError(String),

    #[error("Validation error: {0}")]
    ValidationError(String),


    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),

    #[error("Serde JSON error: {0}")]
    SerdeJsonError(#[from] serde_json::Error),
    
    #[error("Serde YAML error: {0}")]
    SerdeYamlError(#[from] serde_yaml::Error),

    #[error("Regex error: {0}")]
    RegexError(#[from] regex::Error),

    #[error("Terraform crate error: {0}")]
    TerraformCrateError(String),
}

impl From<terraform::Error> for EnvieError {
    fn from(err: terraform::Error) -> Self {
        EnvieError::TerraformCrateError(format!("{:?}", err))
    }
}

pub type Result<T> = std::result::Result<T, EnvieError>;
