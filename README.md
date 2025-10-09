# test-project

Test project

This project is managed by [Envie](https://github.com/your-org/envie), a tool for managing multiple ephemeral environments in Terraform with layered dependencies and resource sharing.

## Project Structure

```
├── workspace.envie.yaml     # Global project configuration
├── services/                # Units directory
│   ├── networking/          # Networking infrastructure
│   │   ├── envie.yaml      # Unit configuration
│   │   └── modules/        # Terraform modules
│   ├── database/            # Database layer
│   │   ├── envie.yaml      # Unit configuration
│   │   └── modules/        # Terraform modules
│   └── api/                 # API layer
│       ├── envie.yaml      # Unit configuration
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

- `workspace.envie.yaml`: Global project configuration with environment definitions
- `services/*/envie.yaml`: Per-unit configuration with dependencies

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
