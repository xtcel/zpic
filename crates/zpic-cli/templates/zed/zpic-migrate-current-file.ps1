param(
  [string]$FilePath = ""
)

$ErrorActionPreference = "Stop"

if ([string]::IsNullOrWhiteSpace($FilePath)) {
  Write-Error "zpic migrate requires a file path from Zed"
}

$zpicBin = if ([string]::IsNullOrWhiteSpace($env:ZPIC_BIN)) { "zpic" } else { $env:ZPIC_BIN }
& $zpicBin migrate $FilePath
exit $LASTEXITCODE
