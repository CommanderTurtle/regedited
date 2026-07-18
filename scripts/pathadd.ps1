$ErrorActionPreference = 'Stop'

$root = Split-Path -Parent $PSScriptRoot
$release = Join-Path $root 'target\release'
$regedited = Join-Path $release 'regedited.exe'
$rgd = Join-Path $release 'rgd.exe'

if (-not (Test-Path -LiteralPath $regedited -PathType Leaf)) {
    throw "Release binary not found: $regedited`nRun: cargo build --release"
}

if (Test-Path -LiteralPath $rgd) {
    Remove-Item -LiteralPath $rgd -Force
}
New-Item -ItemType HardLink -Path $rgd -Target $regedited | Out-Null

function Test-PathEntry {
    param([string]$PathValue, [string]$Candidate)

    $comparison = [System.StringComparison]::OrdinalIgnoreCase
    foreach ($entry in ($PathValue -split ';')) {
        if ([string]::Equals($entry.Trim().TrimEnd('\'), $Candidate.TrimEnd('\'), $comparison)) {
            return $true
        }
    }
    return $false
}

$userPath = [Environment]::GetEnvironmentVariable('Path', 'User')
if (-not (Test-PathEntry -PathValue $userPath -Candidate $release)) {
    $newUserPath = if ([string]::IsNullOrWhiteSpace($userPath)) {
        $release
    } else {
        "$($userPath.TrimEnd(';'));$release"
    }
    [Environment]::SetEnvironmentVariable('Path', $newUserPath, 'User')
    Write-Host "Added to user PATH: $release"
} else {
    Write-Host "User PATH already contains: $release"
}

if (-not (Test-PathEntry -PathValue $env:Path -Candidate $release)) {
    $env:Path = "$release;$env:Path"
}

$regeditedCommand = Get-Command regedited -CommandType Application -ErrorAction Stop
$rgdCommand = Get-Command rgd -CommandType Application -ErrorAction Stop

Write-Host "regedited: $($regeditedCommand.Source)"
Write-Host "rgd hard link: $($rgdCommand.Source)"
Write-Host 'The current PowerShell process and future child processes can now use both commands.'
