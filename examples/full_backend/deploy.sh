#!/bin/bash
set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Check if environment ID is provided
if [ -z "$1" ]; then
    echo -e "${RED}Error: Environment ID required${NC}"
    echo "Usage: $0 <env-id> [--destroy]"
    echo ""
    echo "Examples:"
    echo "  $0 dev123              # Deploy to dev123 environment"
    echo "  $0 dev123 --destroy    # Destroy dev123 environment"
    exit 1
fi

ENV_ID=$1
ACTION=${2:-deploy}

# Find envie binary
SCRIPT_DIR="$( cd "$( dirname "${BASH_SOURCE[0]}" )" &> /dev/null && pwd )"
ENVIE_BIN="$SCRIPT_DIR/../../target/release/envie"

if [ ! -f "$ENVIE_BIN" ]; then
    echo -e "${RED}Error: envie binary not found at $ENVIE_BIN${NC}"
    echo "Please build Envie first:"
    echo "  cd ../../ && cargo build --release"
    exit 1
fi

echo -e "${GREEN}Using Envie: $ENVIE_BIN${NC}"
echo ""

echo -e "${BLUE}╔════════════════════════════════════════════════════╗${NC}"
echo -e "${BLUE}║    Full Backend Example - Envie Deployment        ║${NC}"
echo -e "${BLUE}╚════════════════════════════════════════════════════╝${NC}"
echo ""

if [ "$ACTION" = "--destroy" ]; then
    echo -e "${YELLOW}🗑️  Destroying environment: $ENV_ID${NC}"
    echo ""

    # Destroy in reverse order
    echo -e "${BLUE}Step 1/3: Destroying api unit...${NC}"
    "$ENVIE_BIN" destroy --unit api --env "$ENV_ID"

    echo -e "${BLUE}Step 2/3: Destroying db unit...${NC}"
    "$ENVIE_BIN" destroy --unit db --env "$ENV_ID"

    echo -e "${BLUE}Step 3/3: Destroying core unit...${NC}"
    "$ENVIE_BIN" destroy --unit core --env "$ENV_ID"

    echo ""
    echo -e "${GREEN}✅ Environment $ENV_ID destroyed successfully!${NC}"
else
    echo -e "${GREEN}🚀 Deploying to environment: $ENV_ID${NC}"
    echo ""

    # Deploy in dependency order
    echo -e "${BLUE}Step 1/3: Deploying core unit (API Gateway)...${NC}"
    "$ENVIE_BIN" deploy --unit core --env "$ENV_ID"

    echo -e "${BLUE}Step 2/3: Deploying db unit (DynamoDB)...${NC}"
    "$ENVIE_BIN" deploy --unit db --env "$ENV_ID"

    echo -e "${BLUE}Step 3/3: Deploying api unit (Lambda + Endpoints)...${NC}"
    "$ENVIE_BIN" deploy --unit api --env "$ENV_ID"

    echo ""
    echo -e "${GREEN}✅ Deployment completed successfully!${NC}"
    echo ""
    echo -e "${YELLOW}📋 API Endpoints:${NC}"

    # Try to get the API URL
    cd units/core
    API_URL=$(terraform output -raw api_invoke_url 2>/dev/null || echo "")
    cd ../..

    if [ -n "$API_URL" ]; then
        echo -e "  ${GREEN}POST${NC}   $API_URL/items"
        echo -e "  ${GREEN}GET${NC}    $API_URL/items"
        echo -e "  ${GREEN}GET${NC}    $API_URL/items/{id}"
        echo -e "  ${GREEN}PUT${NC}    $API_URL/items/{id}"
        echo -e "  ${GREEN}DELETE${NC} $API_URL/items/{id}"
        echo ""
        echo -e "${YELLOW}Test it:${NC}"
        echo "  curl -X POST $API_URL/items -H 'Content-Type: application/json' -d '{\"name\":\"Test\",\"description\":\"My item\"}'"
    fi
fi

echo ""
echo -e "${BLUE}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"
