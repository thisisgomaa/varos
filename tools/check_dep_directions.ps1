# Run from the repository root on Windows:
#   powershell -NoProfile -ExecutionPolicy Bypass -File tools/check_dep_directions.ps1

[CmdletBinding()]
param()

$ErrorActionPreference = "Stop"

function Get-RepositoryRoot {
    $root = (& git rev-parse --show-toplevel 2>$null)
    if ($LASTEXITCODE -ne 0 -or -not $root) {
        throw "check_dep_directions must run inside the Varos Git repository"
    }
    return [System.IO.Path]::GetFullPath($root.Trim())
}

function Assert-ExactSet {
    param(
        [string] $Label,
        [object[]] $Actual,
        [object[]] $Expected,
        [System.Collections.Generic.List[string]] $Violations
    )

    $actualValues = @($Actual | ForEach-Object { [string] $_ } | Sort-Object -Unique)
    $expectedValues = @($Expected | ForEach-Object { [string] $_ } | Sort-Object -Unique)
    if (($actualValues -join "|") -ne ($expectedValues -join "|")) {
        $actualText = if ($actualValues.Count) { $actualValues -join ", " } else { "(none)" }
        $expectedText = if ($expectedValues.Count) { $expectedValues -join ", " } else { "(none)" }
        $Violations.Add("${Label}: found [$actualText], expected [$expectedText]")
    }
}

function Remove-RustComments([string] $Text) {
    $withoutBlocks = [regex]::Replace(
        $Text,
        '/\*.*?\*/',
        '',
        [System.Text.RegularExpressions.RegexOptions]::Singleline
    )
    return [regex]::Replace($withoutBlocks, '(?m)//.*$', '')
}

$exitCode = 0

try {
    $repoRoot = Get-RepositoryRoot
    $workspaceManifest = Join-Path $repoRoot "varos/Cargo.toml"
    $metadataJson = (& cargo metadata --format-version 1 --no-deps --manifest-path $workspaceManifest)
    if ($LASTEXITCODE -ne 0 -or -not $metadataJson) {
        throw "cargo metadata failed for '$workspaceManifest'"
    }
    $metadata = $metadataJson | ConvertFrom-Json

    $memberIds = New-Object 'System.Collections.Generic.HashSet[string]' ([System.StringComparer]::Ordinal)
    foreach ($id in $metadata.workspace_members) {
        [void] $memberIds.Add([string] $id)
    }
    $workspacePackages = @($metadata.packages | Where-Object { $memberIds.Contains([string] $_.id) })
    $workspaceNames = @($workspacePackages | ForEach-Object { $_.name } | Sort-Object -Unique)
    $expectedWorkspace = @("varos-app", "varos-core", "varos-pdf", "varos-render-wgpu")
    $violations = New-Object System.Collections.Generic.List[string]
    Assert-ExactSet "workspace members" $workspaceNames $expectedWorkspace $violations

    $packages = @{}
    foreach ($package in $workspacePackages) {
        $packages[$package.name] = $package
    }
    foreach ($required in $expectedWorkspace) {
        if (-not $packages.ContainsKey($required)) {
            $violations.Add("required workspace package is missing: $required")
        }
    }

    if ($packages.ContainsKey("varos-core")) {
        $core = $packages["varos-core"]
        $coreInternal = @($core.dependencies | Where-Object { $workspaceNames -contains $_.name } | ForEach-Object { $_.name })
        Assert-ExactSet "varos-core internal dependencies" $coreInternal @() $violations

        foreach ($dependency in $core.dependencies) {
            $normalized = ([string] $dependency.name).Replace('_', '-')
            if ($normalized -match '^(wgpu|winit|egui(?:-|$)|windows(?:-|$))') {
                $violations.Add("varos-core forbidden UI/GPU/platform dependency: $($dependency.name)")
            }
        }
    }

    if ($packages.ContainsKey("varos-render-wgpu")) {
        $renderer = $packages["varos-render-wgpu"]
        $rendererInternal = @($renderer.dependencies | Where-Object { $workspaceNames -contains $_.name } | ForEach-Object { $_.name })
        Assert-ExactSet "varos-render-wgpu internal dependencies" $rendererInternal @("varos-core") $violations

        foreach ($dependency in $renderer.dependencies) {
            $normalized = ([string] $dependency.name).Replace('_', '-')
            if ($normalized -match '^winit(?:-|$)') {
                $violations.Add("varos-render-wgpu must not depend on winit: $($dependency.name)")
            }
        }
    }

    if ($packages.ContainsKey("varos-pdf")) {
        $pdf = $packages["varos-pdf"]
        $pdfInternal = @($pdf.dependencies | Where-Object { $workspaceNames -contains $_.name } | ForEach-Object { $_.name })
        Assert-ExactSet "varos-pdf internal dependencies" $pdfInternal @("varos-core") $violations
    }

    if ($packages.ContainsKey("varos-app")) {
        $app = $packages["varos-app"]
        $appInternal = @($app.dependencies | Where-Object { $workspaceNames -contains $_.name } | ForEach-Object { $_.name })
        Assert-ExactSet "varos-app internal dependencies" $appInternal @("varos-core", "varos-pdf", "varos-render-wgpu") $violations
        if (@($app.dependencies | Where-Object { $_.name -eq "egui_tiles" }).Count -ne 1) {
            $violations.Add("varos-app must declare exactly one egui_tiles dependency")
        }
    }

    $appSource = [System.IO.Path]::GetFullPath((Join-Path $repoRoot "varos/crates/varos-app/src"))
    $sourcePrefixLength = $appSource.TrimEnd('\', '/').Length + 1
    $eguiTilesUsers = New-Object System.Collections.Generic.List[string]
    foreach ($sourceFile in (Get-ChildItem -LiteralPath $appSource -Recurse -File -Filter "*.rs")) {
        $code = Remove-RustComments ([System.IO.File]::ReadAllText($sourceFile.FullName))
        if ($code -match '\begui_tiles\b') {
            $relativePath = $sourceFile.FullName.Substring($sourcePrefixLength).Replace('\', '/')
            $eguiTilesUsers.Add($relativePath)
        }
    }
    Assert-ExactSet "varos-app egui_tiles code use" $eguiTilesUsers @("shell/boxtree.rs") $violations

    if ($violations.Count -gt 0) {
        foreach ($violation in $violations) {
            [Console]::Error.WriteLine("ERROR: $violation")
        }
        [Console]::Error.WriteLine("check_dep_directions: FAIL ($($violations.Count) violation(s))")
        $exitCode = 1
    } else {
        Write-Output "check_dep_directions: PASS"
        Write-Output "workspace: varos-core, varos-render-wgpu, varos-pdf, varos-app"
        Write-Output "internal edges: renderer -> core; pdf -> core; app -> core, renderer, pdf"
        Write-Output "egui_tiles code use: varos-app/src/shell/boxtree.rs only"
    }
} catch {
    [Console]::Error.WriteLine("check_dep_directions: FAIL - $($_.Exception.Message)")
    $exitCode = 1
}

exit $exitCode
