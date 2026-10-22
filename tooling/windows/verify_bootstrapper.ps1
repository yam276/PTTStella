param([Parameter(Mandatory = $true)][string]$Path)
$ErrorActionPreference = 'Stop'
$signature = Get-AuthenticodeSignature -LiteralPath $Path
if ($signature.Status -ne 'Valid' -or $null -eq $signature.SignerCertificate) {
    throw 'WebView2 bootstrapper does not have a valid Authenticode signature'
}
$publisher = $signature.SignerCertificate.GetNameInfo(
    [System.Security.Cryptography.X509Certificates.X509NameType]::SimpleName, $false)
if ($publisher -ne 'Microsoft Corporation') { throw "Unexpected bootstrapper publisher: $publisher" }
@{
    status = 'Valid'
    publisher = $publisher
    sha256 = (Get-FileHash -LiteralPath $Path -Algorithm SHA256).Hash.ToLowerInvariant()
    file_version = (Get-Item -LiteralPath $Path).VersionInfo.FileVersion
} | ConvertTo-Json -Compress
