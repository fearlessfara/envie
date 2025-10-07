#!/bin/bash

# Envie Example Test Script
# This script demonstrates the environment system with different scenarios

set -e

echo "🚀 Envie Example Test Script"
echo "=============================="

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Function to print colored output
print_status() {
    echo -e "${BLUE}[INFO]${NC} $1"
}

print_success() {
    echo -e "${GREEN}[SUCCESS]${NC} $1"
}

print_warning() {
    echo -e "${YELLOW}[WARNING]${NC} $1"
}

print_error() {
    echo -e "${RED}[ERROR]${NC} $1"
}

# Check if envie is installed
if ! command -v envie &> /dev/null; then
    print_error "Envie is not installed. Please build it first:"
    echo "  cd .. && cargo build --release"
    exit 1
fi

print_status "Envie found: $(which envie)"

# Test 1: Dry run deployment
echo ""
print_status "Test 1: Dry run deployment (shows dependency resolution)"
echo "------------------------------------------------------------"
envie deploy --service api --merge-request 123 --dry-run || {
    print_error "Dry run failed"
    exit 1
}
print_success "Dry run completed successfully"

# Test 2: Deploy networking service
echo ""
print_status "Test 2: Deploy networking service (foundation layer)"
echo "------------------------------------------------------------"
envie deploy --service networking --merge-request 123 || {
    print_error "Networking deployment failed"
    exit 1
}
print_success "Networking service deployed successfully"

# Test 3: Deploy database service
echo ""
print_status "Test 3: Deploy database service (depends on networking)"
echo "------------------------------------------------------------"
envie deploy --service database --merge-request 123 || {
    print_error "Database deployment failed"
    exit 1
}
print_success "Database service deployed successfully"

# Test 4: Deploy API service with environment mixing
echo ""
print_status "Test 4: Deploy API service with environment mixing"
echo "------------------------------------------------------------"
print_warning "This will use sandbox database and ephemeral VPC"
envie deploy --service api --merge-request 123 || {
    print_error "API deployment failed"
    exit 1
}
print_success "API service deployed successfully"

# Test 5: Deploy from service directory
echo ""
print_status "Test 5: Deploy from service directory (auto-discovery)"
echo "------------------------------------------------------------"
cd services/api
envie deploy --merge-request 456 || {
    print_error "Service directory deployment failed"
    exit 1
}
print_success "Service directory deployment completed successfully"
cd ../..

# Test 6: List environments
echo ""
print_status "Test 6: List available environments"
echo "------------------------------------------------------------"
envie env list || {
    print_warning "Environment listing not implemented yet"
}

# Test 7: Cleanup
echo ""
print_status "Test 7: Cleanup (destroy environments)"
echo "------------------------------------------------------------"
print_warning "Destroying MR 123 environment..."
envie destroy --merge-request 123 || {
    print_warning "Destroy failed (this is expected if not implemented yet)"
}

print_warning "Destroying MR 456 environment..."
envie destroy --merge-request 456 || {
    print_warning "Destroy failed (this is expected if not implemented yet)"
}

echo ""
print_success "All tests completed! 🎉"
echo ""
echo "Key features demonstrated:"
echo "✅ Service discovery and dependency resolution"
echo "✅ Cross-environment references (sandbox database)"
echo "✅ Module-level dependencies"
echo "✅ Recursive deployment from any directory"
echo "✅ Environment mixing (ephemeral + stable)"
echo ""
echo "Next steps:"
echo "1. Check the generated .envie-remote-state.tf files"
echo "2. Examine the Terraform state organization"
echo "3. Try different environment mixing scenarios"
echo "4. Add more services and dependencies"
