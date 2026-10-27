[CmdletBinding()]
param([Parameter(Mandatory = $true)][string]$Target)

$ErrorActionPreference = 'Stop'
$projectRoot = Split-Path $PSScriptRoot -Parent
$version = (Get-Content -LiteralPath (Join-Path $projectRoot 'package.json') -Raw | ConvertFrom-Json).version
if ($Target -notmatch '^[a-zA-Z0-9_-]+$') { throw 'Invalid target name.' }
$name = "Pixen-tools-$version-$Target"
$packageRoot = Join-Path $projectRoot 'target/packages'
$stage = Join-Path $projectRoot "target/package-staging/$name"
if (Test-Path -LiteralPath $stage) { throw "Staging folder already exists: $stage" }
New-Item -ItemType Directory -Path $stage -Force | Out-Null
New-Item -ItemType Directory -Path $packageRoot -Force | Out-Null
$suffix = if ($Target -like '*windows*') { '.exe' } else { '' }
foreach ($binary in @('pixen-cli', 'pixen-mcp')) {
    $source = Join-Path $projectRoot "target/release/$binary$suffix"
    if (-not (Test-Path -LiteralPath $source -PathType Leaf)) { throw "Missing binary: $source" }
    Copy-Item -LiteralPath $source -Destination $stage
}
Copy-Item -LiteralPath (Join-Path $projectRoot 'LICENSE') -Destination $stage
Copy-Item -LiteralPath (Join-Path $projectRoot 'skills') -Destination $stage -Recurse
Set-Content -LiteralPath (Join-Path $stage 'README.txt') -Value @'
Pixen headless tools

pixen-cli: file commands and exports; use --help for commands.
pixen-mcp: stdio MCP server, launched by your MCP client.
This package does not contain the desktop editor. Do not use --viewer or
request desktop previews without installing the matching pixen executable
beside these tools. PNG inspection and headless editing remain available.
Video export requires FFmpeg. AI skills are included in skills/.
'@ -Encoding utf8
$hashes = Get-ChildItem -LiteralPath $stage -File | Where-Object { $_.Name -like 'pixen-*' } | ForEach-Object {
    $hash = (Get-FileHash -LiteralPath $_.FullName -Algorithm SHA256).Hash.ToLowerInvariant()
    "$hash  $($_.Name)"
}
Set-Content -LiteralPath (Join-Path $stage 'SHA256SUMS.txt') -Value $hashes -Encoding ascii
if ($suffix) {
    Compress-Archive -LiteralPath $stage -DestinationPath (Join-Path $packageRoot "$name.zip")
} else {
    & tar -czf (Join-Path $packageRoot "$name.tar.gz") -C (Split-Path $stage -Parent) $name
    if ($LASTEXITCODE -ne 0) { throw 'Creating tools archive failed.' }
}
