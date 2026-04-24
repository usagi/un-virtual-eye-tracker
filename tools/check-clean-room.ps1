param(
    [string]$RepoRoot = (Resolve-Path (Join-Path $PSScriptRoot ".."))
)

$ErrorActionPreference = "Stop"

$scanRoots = @("crates", "apps", "config")
$fileExtensions = @(".rs", ".toml", ".ts", ".js", ".svelte", ".c", ".cpp", ".h", ".hpp")

$denyList = @(
    @{ Pattern = 'TrackIR SDK'; Reason = 'proprietary SDK reference' },
    @{ Pattern = 'NaturalPoint SDK'; Reason = 'proprietary SDK reference' },
    @{ Pattern = '#include\s*[<"]NPClient\.h[>"]'; Reason = 'SDK header reference' },
    @{ Pattern = 'leaked\s+code|leak(ed)?\s+source'; Reason = 'leak-derived source reference' },
    @{ Pattern = 'decompil(e|ed)|disassembl(y|ed)'; Reason = 'reverse-engineering source reference' },
    @{ Pattern = 'private\s+internal\s+structure|proprietary\s+internal'; Reason = 'private internal reference' }
)

$hits = @()

foreach ($root in $scanRoots) {
    $fullRoot = Join-Path $RepoRoot $root
    if (-not (Test-Path $fullRoot)) {
        continue
    }

    Get-ChildItem -Path $fullRoot -Recurse -File | ForEach-Object {
        if ($fileExtensions -notcontains $_.Extension.ToLowerInvariant()) {
            return
        }

        foreach ($rule in $denyList) {
            $matches = Select-String -Path $_.FullName -Pattern $rule.Pattern -AllMatches -CaseSensitive:$false
            foreach ($m in $matches) {
                $hits += [pscustomobject]@{
                    File = $_.FullName
                    Line = $m.LineNumber
                    Reason = $rule.Reason
                    Text = $m.Line.Trim()
                }
            }
        }
    }
}

if ($hits.Count -gt 0) {
    Write-Host "Clean-room check failed:" -ForegroundColor Red
    foreach ($hit in $hits) {
        Write-Host ("- {0}:{1} [{2}] {3}" -f $hit.File, $hit.Line, $hit.Reason, $hit.Text)
    }
    exit 1
}

Write-Host "Clean-room check passed." -ForegroundColor Green
exit 0
