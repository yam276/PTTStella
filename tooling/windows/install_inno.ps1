$ErrorActionPreference = 'Stop'
$tools = Get-Content -LiteralPath (Join-Path $PSScriptRoot 'installer-tools.json') -Raw | ConvertFrom-Json
$root = Join-Path $env:RUNNER_TEMP 'pttstella-inno'
New-Item -ItemType Directory -Path $root -Force | Out-Null
$installer = Join-Path $root 'compiler-setup.exe'
Invoke-WebRequest -Uri $tools.url -OutFile $installer
if ((Get-FileHash -LiteralPath $installer -Algorithm SHA256).Hash.ToLowerInvariant() -ne $tools.sha256) {
    throw 'Inno Setup download checksum mismatch'
}
$destination = Join-Path $root 'compiler'
$process = Start-Process -FilePath $installer -ArgumentList @(
    '/VERYSILENT', '/SUPPRESSMSGBOXES', '/NORESTART', '/SP-', '/CURRENTUSER',
    ('/DIR="{0}"' -f $destination)
) -Wait -PassThru
if ($process.ExitCode -ne 0) { throw "Inno Setup installation failed: $($process.ExitCode)" }
$compiler = Join-Path $destination 'ISCC.exe'
if (-not (Test-Path -LiteralPath $compiler)) { throw 'Inno Setup compiler was not installed' }
"INNO_SETUP_COMPILER=$compiler" | Out-File -FilePath $env:GITHUB_ENV -Append -Encoding utf8
