## MODIFIED Requirements

### Requirement: CLI supports first-party uploader families
The foundation release SHALL provide first-party uploaders for local filesystem targets, GitHub repository contents, and S3-compatible object storage endpoints, and the CLI SHALL also support installed uploader plugins discovered through the `zpic` plugin system.

#### Scenario: Upload with an S3-compatible backend
- **WHEN** the user configures an S3-compatible uploader such as Cloudflare R2 with endpoint, bucket, credentials, and public base URL
- **THEN** the upload command stores the object through that backend
- **THEN** the returned URL uses the configured public base URL and generated object key

#### Scenario: Upload with an installed plugin backend
- **WHEN** the user configures an uploader type provided by an installed plugin and makes it active
- **THEN** `zpic upload` resolves that uploader through the plugin registry
- **THEN** the command returns the same result structure used for built-in uploaders
