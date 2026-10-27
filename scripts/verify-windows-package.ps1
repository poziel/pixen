[CmdletBinding()]
param([Parameter(Mandatory = $true)][string]$Archive)

$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName System.IO.Compression.FileSystem
$zip = [IO.Compression.ZipFile]::OpenRead([IO.Path]::GetFullPath($Archive))
try {
    foreach ($binary in @('pixen.exe', 'pixen-cli.exe', 'pixen-mcp.exe')) {
        $entries = @($zip.Entries | Where-Object { $_.Name -ceq $binary -and $_.Length -gt 0 })
        if ($entries.Count -ne 1) { throw "Application ZIP must contain exactly one nonempty $binary" }
    }
    $desktop = $zip.Entries | Where-Object { $_.Name -ceq 'pixen.exe' }
    $folder = $desktop.FullName.Substring(0, $desktop.FullName.Length - $desktop.Name.Length)
    foreach ($binary in @('pixen-cli.exe', 'pixen-mcp.exe')) {
        if (-not ($zip.Entries | Where-Object { $_.FullName -ceq "$folder$binary" })) {
            throw "$binary must be beside pixen.exe in the application ZIP"
        }
    }
    Write-Output "Verified application ZIP: $Archive (editor, CLI, and MCP server)"
} finally {
    $zip.Dispose()
}
