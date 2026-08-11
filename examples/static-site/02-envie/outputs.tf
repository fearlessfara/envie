output "bucket_name" {
  description = "Name of the website bucket"
  value       = aws_s3_bucket.site.bucket
}

output "website_endpoint" {
  description = "S3 website endpoint"
  value       = aws_s3_bucket_website_configuration.site.website_endpoint
}
