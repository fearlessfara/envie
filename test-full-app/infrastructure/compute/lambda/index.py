def handler(event, context):
    return {
        "statusCode": 200,
        "body": "Hello from Full Demo Lambda! Environment: " + context.environment.get("ENVIRONMENT", "unknown")
    }
