## ADDED Requirements

### Requirement: CLI uploads local image files
The `zpic upload` command SHALL accept one or more local image file paths, upload each file through the resolved uploader, and return one result for each successfully processed file.

#### Scenario: Upload a single image
- **WHEN** the user runs `zpic upload ./cover.png`
- **THEN** the command uploads `cover.png` using the active uploader
- **THEN** the command prints the configured output for that upload

#### Scenario: Upload multiple images
- **WHEN** the user runs `zpic upload ./a.png ./b.jpg`
- **THEN** the command uploads both files in the same invocation
- **THEN** the command returns one result item per uploaded file

### Requirement: CLI supports clipboard image uploads
The upload command SHALL support `--clipboard` to read image data from the system clipboard and treat it as the upload input when image data is available.

#### Scenario: Upload image from clipboard
- **WHEN** the user runs `zpic upload --clipboard`
- **THEN** the command reads image data from the clipboard
- **THEN** the command uploads that image through the resolved uploader

#### Scenario: Clipboard does not contain an image
- **WHEN** the user runs `zpic upload --clipboard` and the clipboard does not contain image data
- **THEN** the command exits with a non-zero status
- **THEN** the command explains that no clipboard image was found

### Requirement: CLI supports first-party uploader families
The foundation release SHALL provide first-party uploaders for local filesystem targets, GitHub repository contents, and S3-compatible object storage endpoints.

#### Scenario: Upload with an S3-compatible backend
- **WHEN** the user configures an S3-compatible uploader such as Cloudflare R2 with endpoint, bucket, credentials, and public base URL
- **THEN** the upload command stores the object through that backend
- **THEN** the returned URL uses the configured public base URL and generated object key

### Requirement: CLI resolves uploader and naming policy deterministically
The upload command SHALL resolve the uploader, output format, and target object key from CLI flags first and otherwise fall back to the active configuration defaults.

#### Scenario: CLI override takes precedence
- **WHEN** the active config default uploader is `github` and the user runs `zpic upload ./cover.png --uploader r2`
- **THEN** the command uploads through `r2`
- **THEN** the result identifies `r2` as the uploader used

#### Scenario: Upload path uses template variables
- **WHEN** the active rename strategy is `images/{yyyy}/{mm}/{dd}/{hash8}.{ext}`
- **THEN** the upload command generates an object key that matches that template
- **THEN** the returned URL and key reference the generated object path

### Requirement: CLI supports formatted and machine-readable output
The upload command SHALL support human-readable formatted output and a JSON mode that returns structured metadata for every uploaded item.

#### Scenario: Request Markdown output
- **WHEN** the user runs `zpic upload ./cover.png --format markdown`
- **THEN** the command prints Markdown that references the uploaded image URL

#### Scenario: Request JSON output
- **WHEN** the user runs `zpic upload ./cover.png --json`
- **THEN** the command prints valid JSON
- **THEN** each result item includes the source path, URL, key, uploader, MIME type, and size
