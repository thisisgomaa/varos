# Run from the repository root on Windows:
#   powershell -NoProfile -ExecutionPolicy Bypass -File tools/check_links.ps1

[CmdletBinding()]
param()

$ErrorActionPreference = "Stop"

function Get-RepositoryRoot {
    $root = (& git rev-parse --show-toplevel 2>$null)
    if ($LASTEXITCODE -ne 0 -or -not $root) {
        throw "check_links must run inside the Varos Git repository"
    }
    return [System.IO.Path]::GetFullPath($root.Trim())
}

function Get-FirstPartyDocuments([string] $RepositoryRoot) {
    $tracked = @(& git -C $RepositoryRoot ls-files -- "*.md" "*.html")
    if ($LASTEXITCODE -ne 0) {
        throw "git ls-files failed"
    }

    return @(
        $tracked |
            Where-Object { $_ -and $_ -notmatch '^(?i:varos/vendor)/' } |
            ForEach-Object {
                [System.IO.Path]::GetFullPath(
                    (Join-Path $RepositoryRoot ($_ -replace '/', [System.IO.Path]::DirectorySeparatorChar))
                )
            }
    )
}

function Get-RepositoryRelativePath([string] $RepositoryRoot, [string] $Path) {
    $rootUri = New-Object System.Uri(($RepositoryRoot.TrimEnd('\', '/') + [System.IO.Path]::DirectorySeparatorChar))
    $pathUri = New-Object System.Uri($Path)
    return [System.Uri]::UnescapeDataString($rootUri.MakeRelativeUri($pathUri).ToString())
}

function Remove-MarkdownCode([string] $Text) {
    $lines = $Text -split "`r?`n", 0, "RegexMatch"
    $kept = New-Object System.Collections.Generic.List[string]
    $fence = $null

    foreach ($line in $lines) {
        if ($null -eq $fence -and $line -match '^\s*(?<fence>`{3,}|~{3,})') {
            $fence = $Matches.fence.Substring(0, 1)
            $kept.Add("")
            continue
        }
        if ($null -ne $fence -and $line -match ("^\s*" + [regex]::Escape($fence) + "{3,}")) {
            $fence = $null
            $kept.Add("")
            continue
        }
        if ($null -ne $fence) {
            $kept.Add("")
            continue
        }

        # Inline code is not documentation navigation and may contain link examples.
        $kept.Add([regex]::Replace($line, '`+[^`]*`+', ''))
    }

    return ($kept -join "`n")
}

function ConvertTo-GitHubAnchor([string] $Heading) {
    $value = [System.Net.WebUtility]::HtmlDecode($Heading)
    $value = [regex]::Replace($value, '<[^>]+>', '')
    $value = $value.Trim().ToLowerInvariant()
    $value = [regex]::Replace($value, '[^\p{L}\p{Nd}-]+', '-')
    return $value.Trim('-')
}

function Get-DocumentAnchors([string] $Path) {
    $text = [System.IO.File]::ReadAllText($Path)
    $anchors = New-Object 'System.Collections.Generic.HashSet[string]' ([System.StringComparer]::OrdinalIgnoreCase)

    foreach ($match in [regex]::Matches($text, '(?i)\b(?:id|name)\s*=\s*["''](?<id>[^"'']+)["'']')) {
        [void] $anchors.Add([System.Net.WebUtility]::HtmlDecode($match.Groups['id'].Value))
    }

    if ([System.IO.Path]::GetExtension($Path) -ieq '.md') {
        $markdown = Remove-MarkdownCode $text
        $seen = @{}
        foreach ($line in ($markdown -split "`n")) {
            if ($line -notmatch '^\s{0,3}#{1,6}\s+(?<heading>.+?)\s*#*\s*$') {
                continue
            }

            $base = ConvertTo-GitHubAnchor $Matches.heading
            if (-not $base) {
                continue
            }

            $count = 0
            if ($seen.ContainsKey($base)) {
                $count = [int] $seen[$base] + 1
            }
            $seen[$base] = $count
            $anchor = if ($count -eq 0) { $base } else { "$base-$count" }
            [void] $anchors.Add($anchor)
        }
    }

    return $anchors
}

function Get-LinkTarget([string] $RawTarget) {
    $target = $RawTarget.Trim()
    if ($target.StartsWith('<') -and $target.EndsWith('>')) {
        $target = $target.Substring(1, $target.Length - 2)
    } elseif ($target -match '^(?<path>\S+)(?:\s+["''].*["''])$') {
        $target = $Matches.path
    }
    return [System.Net.WebUtility]::HtmlDecode($target)
}

function Get-DocumentLinks([string] $Path) {
    $text = [System.IO.File]::ReadAllText($Path)
    if ([System.IO.Path]::GetExtension($Path) -ieq '.md') {
        $text = Remove-MarkdownCode $text
    }

    $links = New-Object System.Collections.Generic.List[string]
    foreach ($match in [regex]::Matches($text, '!?\[[^\]]*\]\((?<target>[^\r\n)]*)\)')) {
        $links.Add((Get-LinkTarget $match.Groups['target'].Value))
    }
    foreach ($match in [regex]::Matches($text, '(?m)^\s{0,3}\[[^\]]+\]:\s*(?<target>\S+)')) {
        $links.Add((Get-LinkTarget $match.Groups['target'].Value))
    }
    foreach ($match in [regex]::Matches($text, '(?i)\b(?:href|src)\s*=\s*["''](?<target>[^"'']*)["'']')) {
        $links.Add((Get-LinkTarget $match.Groups['target'].Value))
    }
    return $links
}

function Test-IsRelativeLink([string] $Target) {
    if (-not $Target) {
        return $false
    }
    if ($Target.StartsWith('//') -or $Target.StartsWith('/')) {
        return $false
    }
    if ($Target -match '^[A-Za-z][A-Za-z0-9+.-]*:') {
        return $false
    }
    return $true
}

$repoRoot = Get-RepositoryRoot
$documents = @(Get-FirstPartyDocuments $repoRoot)
$anchorCache = @{}
$failures = New-Object System.Collections.Generic.List[string]
$relativeLinkCount = 0
$anchorCheckCount = 0

foreach ($document in $documents) {
    $sourceRelative = Get-RepositoryRelativePath $repoRoot $document
    foreach ($rawTarget in (Get-DocumentLinks $document)) {
        if (-not (Test-IsRelativeLink $rawTarget)) {
            continue
        }

        $relativeLinkCount++
        $parts = $rawTarget -split '#', 2
        $pathPart = ($parts[0] -split '\?', 2)[0]
        $fragment = if ($parts.Count -eq 2) { $parts[1] } else { $null }

        try {
            $decodedPath = [System.Uri]::UnescapeDataString($pathPart)
            $decodedFragment = if ($null -ne $fragment) { [System.Uri]::UnescapeDataString($fragment) } else { $null }
        } catch {
            $failures.Add("${sourceRelative}: malformed URL escape in '$rawTarget'")
            continue
        }

        $targetPath = if (-not $decodedPath) {
            $document
        } else {
            [System.IO.Path]::GetFullPath((Join-Path ([System.IO.Path]::GetDirectoryName($document)) ($decodedPath -replace '/', [System.IO.Path]::DirectorySeparatorChar)))
        }

        if (-not (Test-Path -LiteralPath $targetPath)) {
            $failures.Add("${sourceRelative}: missing target '$rawTarget'")
            continue
        }

        if ($null -ne $decodedFragment -and $decodedFragment -ne '') {
            $extension = [System.IO.Path]::GetExtension($targetPath)
            if ($extension -in @('.md', '.html', '.htm')) {
                $anchorCheckCount++
                if (-not $anchorCache.ContainsKey($targetPath)) {
                    $anchorCache[$targetPath] = Get-DocumentAnchors $targetPath
                }
                if (-not $anchorCache[$targetPath].Contains($decodedFragment)) {
                    $failures.Add("${sourceRelative}: missing anchor '#$decodedFragment' in '$rawTarget'")
                }
            }
        }
    }
}

if ($failures.Count -gt 0) {
    foreach ($failure in $failures) {
        [Console]::Error.WriteLine("ERROR: $failure")
    }
    [Console]::Error.WriteLine("check_links: FAIL ($($failures.Count) broken link(s))")
    exit 1
}

Write-Output "check_links: PASS ($($documents.Count) first-party docs, $relativeLinkCount relative links, $anchorCheckCount heading anchors)"
