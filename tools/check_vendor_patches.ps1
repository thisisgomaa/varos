# Run from the repository root on Windows:
#   powershell -NoProfile -ExecutionPolicy Bypass -File tools/check_vendor_patches.ps1
# For an audited offline source tree, add: -UpstreamPath <path-to-egui_tiles-0.16.0>

[CmdletBinding()]
param(
    [string] $UpstreamPath
)

$ErrorActionPreference = "Stop"

$PackageName = "egui_tiles"
$PackageVersion = "0.16.0"
$UpstreamVcsSha = "62ac74717ebe284749a0066adf9566bbbab9ee42"
$PackageSha256 = "9EB8FEF6130BD04FCB7BB3584845605E57C56FED249BC3CA5A568E696CC0A174"
$PackageUrl = "https://static.crates.io/crates/$PackageName/$PackageName-$PackageVersion.crate"
$ExpectedModified = @(
    "src/behavior.rs"
    "src/container/linear.rs"
    "src/container/tabs.rs"
    "src/lib.rs"
    "src/tree.rs"
)
$RegistryOnly = @(
    ".cargo-ok"
    ".cargo_vcs_info.json"
    "Cargo.toml.orig"
)

function Get-RepositoryRoot {
    $root = (& git rev-parse --show-toplevel 2>$null)
    if ($LASTEXITCODE -ne 0 -or -not $root) {
        throw "check_vendor_patches must run inside the Varos Git repository"
    }
    return [System.IO.Path]::GetFullPath($root.Trim())
}

function Get-RelativeFiles([string] $Root) {
    $prefixLength = $Root.TrimEnd('\', '/').Length + 1
    return @(
        Get-ChildItem -LiteralPath $Root -Recurse -File -Force |
            ForEach-Object { $_.FullName.Substring($prefixLength).Replace('\', '/') } |
            Sort-Object
    )
}

function Get-NormalizedSha256([string] $Path) {
    $bytes = [System.IO.File]::ReadAllBytes($Path)
    if ($bytes -contains 0) {
        return (Get-FileHash -LiteralPath $Path -Algorithm SHA256).Hash
    }

    $text = [System.IO.File]::ReadAllText($Path)
    $text = $text.Replace("`r`n", "`n").Replace("`r", "`n")
    $normalized = (New-Object System.Text.UTF8Encoding($false)).GetBytes($text)
    $sha = [System.Security.Cryptography.SHA256]::Create()
    try {
        return (($sha.ComputeHash($normalized) | ForEach-Object { $_.ToString("x2") }) -join "").ToUpperInvariant()
    } finally {
        $sha.Dispose()
    }
}

function Assert-UpstreamIdentity([string] $Root) {
    $manifestPath = Join-Path $Root "Cargo.toml"
    $vcsPath = Join-Path $Root ".cargo_vcs_info.json"
    if (-not (Test-Path -LiteralPath $manifestPath) -or -not (Test-Path -LiteralPath $vcsPath)) {
        throw "upstream tree is missing Cargo.toml or .cargo_vcs_info.json: $Root"
    }

    $manifest = [System.IO.File]::ReadAllText($manifestPath)
    $package = [regex]::Match($manifest, '(?ms)^\[package\]\s*(?<body>.*?)(?=^\[|\z)')
    $packageBody = $package.Groups['body'].Value
    if (
        -not $package.Success -or
        $packageBody -notmatch "(?m)^name\s*=\s*`"$([regex]::Escape($PackageName))`"\s*$" -or
        $packageBody -notmatch "(?m)^version\s*=\s*`"$([regex]::Escape($PackageVersion))`"\s*$"
    ) {
        throw "upstream Cargo.toml is not $PackageName $PackageVersion"
    }

    $vcs = [System.IO.File]::ReadAllText($vcsPath) | ConvertFrom-Json
    if ($vcs.git.sha1 -ne $UpstreamVcsSha) {
        throw "upstream VCS SHA is '$($vcs.git.sha1)', expected '$UpstreamVcsSha'"
    }
}

function Get-CrateArchive([string] $CargoHome, [string] $TemporaryRoot) {
    $cacheRoot = Join-Path $CargoHome "registry/cache"
    $archive = $null
    if (Test-Path -LiteralPath $cacheRoot) {
        $archive = Get-ChildItem -Path $cacheRoot -Recurse -File -Filter "$PackageName-$PackageVersion.crate" -ErrorAction SilentlyContinue |
            Select-Object -First 1
    }

    if (-not $archive) {
        $archivePath = Join-Path $TemporaryRoot "$PackageName-$PackageVersion.crate"
        [System.Net.ServicePointManager]::SecurityProtocol = [System.Net.SecurityProtocolType]::Tls12
        Write-Verbose "upstream archive not found in Cargo cache; downloading immutable crates.io package"
        Invoke-WebRequest -UseBasicParsing -Uri $PackageUrl -OutFile $archivePath | Out-Null
        $archive = Get-Item -LiteralPath $archivePath
    }

    $actualHash = (Get-FileHash -LiteralPath $archive.FullName -Algorithm SHA256).Hash.ToUpperInvariant()
    if ($actualHash -ne $PackageSha256) {
        throw "crate archive SHA-256 is '$actualHash', expected '$PackageSha256'"
    }
    return $archive.FullName
}

$temporaryRoot = $null
$exitCode = 0
$archiveVerified = $false

try {
    $repoRoot = Get-RepositoryRoot
    $vendorRoot = [System.IO.Path]::GetFullPath((Join-Path $repoRoot "varos/vendor/egui_tiles"))
    if (-not (Test-Path -LiteralPath $vendorRoot)) {
        throw "vendored fork not found: $vendorRoot"
    }

    if ($UpstreamPath) {
        $upstreamRoot = [System.IO.Path]::GetFullPath((Resolve-Path -LiteralPath $UpstreamPath).Path)
    } else {
        $temporaryRoot = Join-Path ([System.IO.Path]::GetTempPath()) ("varos-egui-tiles-" + [guid]::NewGuid().ToString("N"))
        New-Item -ItemType Directory -Path $temporaryRoot | Out-Null
        $cargoHome = if ($env:CARGO_HOME) { $env:CARGO_HOME } else { Join-Path $HOME ".cargo" }
        $archivePath = Get-CrateArchive $cargoHome $temporaryRoot
        $archiveVerified = $true
        $tar = Get-Command tar.exe -ErrorAction SilentlyContinue
        if (-not $tar) {
            $tar = Get-Command tar -ErrorAction SilentlyContinue
        }
        if (-not $tar) {
            throw "tar is required to extract the verified crates.io archive"
        }
        & $tar.Source -xf $archivePath -C $temporaryRoot
        if ($LASTEXITCODE -ne 0) {
            throw "tar failed to extract '$archivePath'"
        }
        $upstreamRoot = Join-Path $temporaryRoot "$PackageName-$PackageVersion"
    }

    Assert-UpstreamIdentity $upstreamRoot

    $upstreamFiles = @(Get-RelativeFiles $upstreamRoot)
    $comparableUpstream = @($upstreamFiles | Where-Object { $_ -notin $RegistryOnly } | Sort-Object)
    $vendorFiles = @(Get-RelativeFiles $vendorRoot)
    $fileSetDelta = @(Compare-Object -ReferenceObject $comparableUpstream -DifferenceObject $vendorFiles)
    if ($fileSetDelta.Count -gt 0) {
        $details = ($fileSetDelta | ForEach-Object { "$($_.SideIndicator) $($_.InputObject)" }) -join "; "
        throw "vendor/upstream comparable file sets differ: $details"
    }

    $modified = New-Object System.Collections.Generic.List[string]
    foreach ($relativePath in $comparableUpstream) {
        $upstreamFile = Join-Path $upstreamRoot ($relativePath.Replace('/', [System.IO.Path]::DirectorySeparatorChar))
        $vendorFile = Join-Path $vendorRoot ($relativePath.Replace('/', [System.IO.Path]::DirectorySeparatorChar))
        if ((Get-NormalizedSha256 $upstreamFile) -ne (Get-NormalizedSha256 $vendorFile)) {
            $modified.Add($relativePath)
        }
    }

    $modified = @($modified | Sort-Object)
    $expected = @($ExpectedModified | Sort-Object)
    $patchDelta = @(Compare-Object -ReferenceObject $expected -DifferenceObject $modified)
    if ($patchDelta.Count -gt 0) {
        $details = ($patchDelta | ForEach-Object { "$($_.SideIndicator) $($_.InputObject)" }) -join "; "
        throw "modified-file contract differs from the expected five files: $details"
    }

    Write-Output "check_vendor_patches: PASS"
    Write-Output "upstream: $PackageName $PackageVersion ($UpstreamVcsSha)"
    if ($archiveVerified) {
        Write-Output "archive SHA-256: $PackageSha256"
    } else {
        Write-Output "source: explicit upstream tree (package version and VCS SHA verified)"
    }
    Write-Output "comparable files: $($comparableUpstream.Count); modified files: $($modified.Count)"
    $modified | ForEach-Object { Write-Output "  $_" }
} catch {
    [Console]::Error.WriteLine("check_vendor_patches: FAIL - $($_.Exception.Message)")
    $exitCode = 1
} finally {
    if ($temporaryRoot -and (Test-Path -LiteralPath $temporaryRoot)) {
        Remove-Item -LiteralPath $temporaryRoot -Recurse -Force
    }
}

exit $exitCode
