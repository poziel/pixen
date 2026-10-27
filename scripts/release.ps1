[CmdletBinding()]
param([switch]$PackageOnly)

$ErrorActionPreference = 'Stop'
$projectRoot = Split-Path $PSScriptRoot -Parent
Push-Location $projectRoot
try {
    $manifest = Get-Content -LiteralPath (Join-Path $projectRoot 'package.json') -Raw | ConvertFrom-Json
    if (-not $PackageOnly) {
        # Local packaging uses sibling sources, like bun run dev/build.
        # GitHub builds published dependencies and calls -PackageOnly.
        & bun run build
        if ($LASTEXITCODE -ne 0) { throw 'Pixen desktop build failed.' }
        & bun run build:tools
        if ($LASTEXITCODE -ne 0) { throw 'Pixen CLI/MCP build failed.' }
    }
    $architecture = (& rustc -vV | Select-String '^host:').ToString().Split(':', 2)[1].Trim()
    if ($LASTEXITCODE -ne 0) { throw 'Could not determine the Rust host target.' }
    $name = "Pixen-$($manifest.version)-$architecture"
    $releaseRoot = Join-Path $projectRoot 'target/portable'
    $publishedDestination = Join-Path $releaseRoot $name
    $stagingRoot = Join-Path $releaseRoot ('.staging-' + [Guid]::NewGuid().ToString('N'))
    $destination = Join-Path $stagingRoot $name
    New-Item -ItemType Directory -Path $destination -Force | Out-Null
    foreach ($binary in @('pixen.exe', 'pixen-cli.exe', 'pixen-mcp.exe')) {
        $source = Join-Path $projectRoot "target/release/$binary"
        if (-not (Test-Path -LiteralPath $source -PathType Leaf)) { throw "Release executable missing: $source" }
        Copy-Item -LiteralPath $source -Destination $destination -Force
    }
    # Remove only obsolete files from earlier generated packages; preserve user themes.
    $legacyIcons = [IO.Path]::GetFullPath((Join-Path $destination 'icons'))
    if (-not $legacyIcons.StartsWith([IO.Path]::GetFullPath($releaseRoot) + [IO.Path]::DirectorySeparatorChar)) { throw 'Invalid generated asset directory.' }
    if (Test-Path -LiteralPath $legacyIcons) { Remove-Item -LiteralPath $legacyIcons -Recurse -Force }
    $themeDirectory = Join-Path $destination 'themes'
    New-Item -ItemType Directory -Path $themeDirectory -Force | Out-Null
    foreach ($legacyTheme in @('pixen.yaml', 'manifest.json')) {
        $legacyFile = Join-Path $themeDirectory $legacyTheme
        if (Test-Path -LiteralPath $legacyFile) { Remove-Item -LiteralPath $legacyFile -Force }
    }
    $themeHelp = 'Put custom Frame themes here (JSON, JSONC, JSON5, YAML, TOML, or INI), then restart Pixen. Valid files appear in Settings > Appearance. Invalid files are ignored. Pixen is embedded and cannot be replaced. Copy the optional template from ../theme-examples/ to get started.'
    Set-Content -LiteralPath (Join-Path $themeDirectory 'README.txt') -Value $themeHelp -Encoding utf8
    $exampleDirectory = Join-Path $destination 'theme-examples'
    New-Item -ItemType Directory -Path $exampleDirectory -Force | Out-Null
    $example = Get-Content -LiteralPath (Join-Path $projectRoot 'assets/themes/pixen.yaml') -Raw
    $example = $example.Replace('id: pixen', 'id: pixen-example').Replace('name: Pixen', 'name: Pixen example')
    Set-Content -LiteralPath (Join-Path $exampleDirectory 'pixen-example.yaml') -Value $example -Encoding utf8
    foreach ($skill in @('pixen', 'pixel-art')) {
        $skillSource = Join-Path $projectRoot "skills/$skill"
        if (-not (Test-Path -LiteralPath (Join-Path $skillSource 'SKILL.md'))) { throw "Missing skill: $skill" }
        $skillDestination = Join-Path $destination "skills/$skill"
        Get-ChildItem -LiteralPath $skillSource -Recurse -File | ForEach-Object {
            $relative = $_.FullName.Substring($skillSource.Length).TrimStart([char[]]'\/')
            $output = Join-Path $skillDestination $relative
            New-Item -ItemType Directory -Path (Split-Path $output -Parent) -Force | Out-Null
            Copy-Item -LiteralPath $_.FullName -Destination $output -Force
        }
    }
    $readme = @"
Pixen $($manifest.version) - Windows release

Run pixen.exe to open the editor. Keep all three executables together:
the CLI and MCP server launch the sibling pixen.exe for live previews.
Microsoft Edge WebView2 Runtime must be installed on the computer.
Storage modes are available in Settings > Installation:
- Default: use this computer's user folders (no portable marker).
- Portable: keep all Pixen data in data/ beside the executables (empty portable marker).
- Semi-portable: keep configuration, themes, and recovery in data/; browser/cache/temp/logs
  remain on this computer (portable marker containing semi-portable).
Apply and restart transfers existing data, backs up destination conflicts, and reopens
unfinished artwork. Original files are retained; undo history and AI conversations reset.
Close other Pixen windows and MCP connections before applying.
Manually changing the portable marker selects a profile without transferring data.
Keep data/, portable, custom themes and backups when replacing the application files.
Settings file: data/config/settings.json in portable mode; otherwise
%APPDATA%/com.poziel.pixen/settings.json on Windows.
Keyboard shortcuts use keybindings.json beside settings.json in either mode.
Existing browser preferences migrate when that settings file is first created.
AI providers require their own installed tools and authentication.
The skills/pixen and skills/pixel-art folders contain complete AI skill bundles.
Install or reference them using your AI client; bundling does not register them globally.

MCP client configuration (replace the path with your extracted folder):

[mcp_servers.pixen]
command = 'C:\Apps\Pixen\pixen-mcp.exe'
args = ["--viewer"]

The MCP client starts the server. Do not launch another server manually.
Omit --viewer to keep editing headless until a preview is requested.
"@
    Set-Content -LiteralPath (Join-Path $destination 'README.txt') -Value $readme -Encoding utf8
    $checksums = Get-ChildItem -LiteralPath $destination -Filter '*.exe' | ForEach-Object {
        $stream = [IO.File]::OpenRead($_.FullName)
        $algorithm = [Security.Cryptography.SHA256]::Create()
        try {
            $hash = [BitConverter]::ToString($algorithm.ComputeHash($stream)).Replace('-', '').ToLowerInvariant()
            "$hash  $($_.Name)"
        } finally {
            $algorithm.Dispose()
            $stream.Dispose()
        }
    }
    Set-Content -LiteralPath (Join-Path $destination 'SHA256SUMS.txt') -Value $checksums -Encoding ascii
    $archive = Join-Path $releaseRoot "$name.zip"
    [Reflection.Assembly]::LoadWithPartialName('System.IO.Compression.FileSystem') | Out-Null
    if ([IO.File]::Exists($archive)) { [IO.File]::Delete($archive) }
    [IO.Compression.ZipFile]::CreateFromDirectory($destination, $archive, [IO.Compression.CompressionLevel]::Optimal, $true)
    & (Join-Path $PSScriptRoot 'verify-windows-package.ps1') -Archive $archive
    # Publish only generated files. Existing portable data is never read into the ZIP.
    # Never stop a running release to replace it. Publish beside it when locked.
    $releaseInUse = $false
    foreach ($binary in @('pixen.exe', 'pixen-cli.exe', 'pixen-mcp.exe')) {
        $existing = Join-Path $publishedDestination $binary
        if ([IO.File]::Exists($existing)) {
            try { $probe = [IO.File]::Open($existing, [IO.FileMode]::Open, [IO.FileAccess]::ReadWrite, [IO.FileShare]::None); $probe.Dispose() }
            catch [IO.IOException] { $releaseInUse = $true; break }
        }
    }
    if ($releaseInUse) {
        $publishedDestination = Join-Path $releaseRoot ($name + '-build-' + [DateTime]::UtcNow.ToString('yyyyMMddHHmmssfff'))
        Write-Output "Existing release is in use; publishing beside it: $publishedDestination"
    }
    New-Item -ItemType Directory -Path $publishedDestination -Force | Out-Null
    Get-ChildItem -LiteralPath $destination | Copy-Item -Destination $publishedDestination -Recurse -Force
    $resolvedStaging = [IO.Path]::GetFullPath($stagingRoot)
    if (-not $resolvedStaging.StartsWith([IO.Path]::GetFullPath($releaseRoot) + [IO.Path]::DirectorySeparatorChar)) { throw 'Invalid staging path.' }
    Remove-Item -LiteralPath $resolvedStaging -Recurse -Force
    Write-Output "Release folder: $publishedDestination"
    Get-Item -LiteralPath $archive | Select-Object FullName, Length
    Get-ChildItem -LiteralPath $publishedDestination -Filter '*.exe' | Select-Object Name, Length
} finally {
    Pop-Location
}
