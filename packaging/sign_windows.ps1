param(
    [Parameter(Mandatory = $true)]
    [ValidateScript({ Test-Path -LiteralPath $_ -PathType Leaf })]
    [string]$Path
)

$ErrorActionPreference = 'Stop'
if (-not $env:CQ_WINDOWS_PFX_BASE64 -or -not $env:CQ_WINDOWS_PFX_PASSWORD) {
    throw 'Windows signing requires CQ_WINDOWS_PFX_BASE64 and CQ_WINDOWS_PFX_PASSWORD.'
}

$sdk = Join-Path ${env:ProgramFiles(x86)} 'Windows Kits\10\bin'
$signTool = Get-ChildItem -LiteralPath $sdk -Filter signtool.exe -Recurse -File |
    Where-Object { $_.DirectoryName -match '[\\/]x64$' } |
    Sort-Object FullName -Descending |
    Select-Object -First 1
if (-not $signTool) { throw 'The Windows SDK x64 signtool.exe was not found.' }

$certificate = Join-Path $env:RUNNER_TEMP ([guid]::NewGuid().ToString() + '.pfx')
try {
    [IO.File]::WriteAllBytes($certificate, [Convert]::FromBase64String($env:CQ_WINDOWS_PFX_BASE64))
    & $signTool.FullName sign /f $certificate /p $env:CQ_WINDOWS_PFX_PASSWORD /fd SHA256 /tr https://timestamp.digicert.com /td SHA256 $Path
    if ($LASTEXITCODE -ne 0) { throw "Authenticode signing failed for $Path." }
    & $signTool.FullName verify /pa /all /v /tw $Path
    if ($LASTEXITCODE -ne 0) { throw "Authenticode verification failed for $Path." }
    $signature = Get-AuthenticodeSignature -LiteralPath $Path
    if ($signature.Status -ne 'Valid' -or -not $signature.TimeStamperCertificate) {
        throw "The signature or RFC 3161 timestamp is invalid for $Path."
    }
}
finally {
    Remove-Item -LiteralPath $certificate -Force -ErrorAction SilentlyContinue
}
