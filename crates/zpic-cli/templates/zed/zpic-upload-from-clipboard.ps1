param(
  [string]$Format = "markdown",
  [string]$Selection = ""
)

$ErrorActionPreference = "Stop"

$zpicBin = if ([string]::IsNullOrWhiteSpace($env:ZPIC_BIN)) { "zpic" } else { $env:ZPIC_BIN }
$normalized = ($Selection -replace "`r", " " -replace "`n", " " -replace "`t", " ").Trim()

$args = @("upload", "--clipboard", "--format", $Format, "--copy")

if (-not [string]::IsNullOrWhiteSpace($normalized)) {
  $collapsed = [regex]::Replace($normalized, "\s+", " ")
  $safeName = $collapsed.ToLowerInvariant()
  $safeName = [regex]::Replace($safeName, "[^a-z0-9._-]+", "-").Trim("-")
  if ($safeName.Length -gt 80) {
    $safeName = $safeName.Substring(0, 80).Trim("-")
  }
  if (-not [string]::IsNullOrWhiteSpace($safeName)) {
    $args += @("--name", $safeName)
  }
  $args += @("--alt", $collapsed)
}

& $zpicBin @args
exit $LASTEXITCODE
