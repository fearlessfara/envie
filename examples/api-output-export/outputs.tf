output "api_endpoint" {
  description = "HTTP API invoke URL"
  value       = aws_apigatewayv2_api.http.api_endpoint
}

output "function_name" {
  description = "Lambda function name"
  value       = aws_lambda_function.api.function_name
}
