[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)][string]$Version,
    [string]$ReleaseName,
    [string]$ReleaseNotes,
    [string[]]$AssetPaths,
    [string]$TargetCommitish = 'main'
)

$ErrorActionPreference = 'Stop'
[Console]::OutputEncoding = [System.Text.Encoding]::UTF8

if (-not $ReleaseName) {
    $ReleaseName = "WinGlow v$Version"
}

$tag = if ($Version.StartsWith('v')) { $Version } else { 'v' + $Version }

if (-not $ReleaseNotes) {
    $ReleaseNotes = "- 发布 $ReleaseName"
}

function Get-GitHubToken {
    $tempQueryPath = Join-Path ([System.IO.Path]::GetTempPath()) ('winglow-credential-' + [guid]::NewGuid().ToString('N') + '.txt')

    try {
        Set-Content -LiteralPath $tempQueryPath -Encoding ASCII -NoNewline -Value "protocol=https`r`nhost=github.com`r`npath=soberbw-hash/WinGlow.git`r`n`r`n"
        $creds = & cmd /c ("git credential fill < `"" + $tempQueryPath + "`"")
        $passwordLine = $creds | Select-String '^password=' | Select-Object -First 1
        if (-not $passwordLine) {
            throw '没有读取到 GitHub token。'
        }

        return $passwordLine.ToString().Substring(9)
    }
    finally {
        if (Test-Path -LiteralPath $tempQueryPath) {
            Remove-Item -LiteralPath $tempQueryPath -Force -ErrorAction SilentlyContinue
        }
    }
}

function Invoke-GitHubJson {
    param(
        [Parameter(Mandatory = $true)][string]$Method,
        [Parameter(Mandatory = $true)][string]$Uri,
        [Parameter(Mandatory = $true)][hashtable]$Headers,
        [object]$Body
    )

    if ($null -eq $Body) {
        return Invoke-RestMethod -Method $Method -Uri $Uri -Headers $Headers
    }

    $json = $Body | ConvertTo-Json -Depth 8
    $bytes = [System.Text.Encoding]::UTF8.GetBytes($json)
    return Invoke-RestMethod -Method $Method -Uri $Uri -Headers $Headers -Body $bytes -ContentType 'application/json; charset=utf-8'
}

function Remove-ExistingAsset {
    param(
        [Parameter(Mandatory = $true)][object]$Release,
        [Parameter(Mandatory = $true)][string]$AssetName,
        [Parameter(Mandatory = $true)][hashtable]$Headers
    )

    $assets = @($Release.assets)
    foreach ($asset in $assets) {
        if ($asset -and $asset.name -eq $AssetName) {
            Invoke-RestMethod -Method Delete -Uri ("https://api.github.com/repos/soberbw-hash/WinGlow/releases/assets/" + $asset.id) -Headers $Headers | Out-Null
        }
    }
}

$token = Get-GitHubToken
$headers = @{
    Authorization = "Bearer $token"
    Accept = 'application/vnd.github+json'
    'X-GitHub-Api-Version' = '2022-11-28'
    'User-Agent' = 'WinGlow-ReleasePublisher'
}

$body = @{
    tag_name = $tag
    target_commitish = $TargetCommitish
    name = $ReleaseName
    body = $ReleaseNotes
    draft = $true
    prerelease = $false
}

try {
    $release = Invoke-GitHubJson -Method Post -Uri 'https://api.github.com/repos/soberbw-hash/WinGlow/releases' -Headers $headers -Body $body
}
catch {
    $response = $_.Exception.Response
    if ($response -and $response.StatusCode.Value__ -eq 422) {
        $release = Invoke-RestMethod -Method Get -Uri ("https://api.github.com/repos/soberbw-hash/WinGlow/releases/tags/" + $tag) -Headers $headers
        if (-not $release.draft) { throw "Release $tag is already public." }
        $release = Invoke-GitHubJson -Method Patch -Uri ("https://api.github.com/repos/soberbw-hash/WinGlow/releases/" + $release.id) -Headers $headers -Body @{
            name = $ReleaseName
            body = $ReleaseNotes
            draft = $true
            prerelease = $false
        }
    }
    else {
        throw
    }
}

foreach ($assetPath in ($AssetPaths | Where-Object { $_ })) {
    $resolvedPath = Resolve-Path $assetPath
    $assetName = Split-Path -Leaf $resolvedPath
    Remove-ExistingAsset -Release $release -AssetName $assetName -Headers $headers

    $uploadUrl = "https://uploads.github.com/repos/soberbw-hash/WinGlow/releases/$($release.id)/assets?name=$([uri]::EscapeDataString($assetName))"
    $auth = 'header = "Authorization: Bearer ' + $token + '"'
    $uploaded = $auth | & curl.exe --config - --silent --show-error --fail --http1.1 -X POST `
        -H "Accept: application/vnd.github+json" `
        -H "Content-Type: application/octet-stream" `
        --data-binary "@$resolvedPath" `
        $uploadUrl
    if ($LASTEXITCODE -ne 0) { throw "Upload failed for $assetName. Release remains a draft." }
    $asset = ($uploaded -join "`n") | ConvertFrom-Json
    $digest = 'sha256:' + (Get-FileHash -LiteralPath $resolvedPath -Algorithm SHA256).Hash.ToLowerInvariant()
    if ($asset.name -ne $assetName -or $asset.state -ne 'uploaded' -or
        $asset.size -ne (Get-Item -LiteralPath $resolvedPath).Length -or $asset.digest -ne $digest) {
        throw "GitHub did not confirm the upload and SHA-256 for $assetName. Release remains a draft."
    }
}

$release = Invoke-GitHubJson -Method Patch -Uri ("https://api.github.com/repos/soberbw-hash/WinGlow/releases/" + $release.id) -Headers $headers -Body @{
    draft = $false
    make_latest = 'true'
}
[PSCustomObject]@{
    Tag = $tag
    ReleaseUrl = $release.html_url
    Assets = @($AssetPaths | Where-Object { $_ } | ForEach-Object { Split-Path -Leaf $_ })
} | Format-List






