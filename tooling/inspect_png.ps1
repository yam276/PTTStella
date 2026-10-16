param(
    [Parameter(Mandatory = $true)][string]$PngFilePath,
    [string]$ReferenceFilePath,
    [int]$StartRow = 0,
    [int]$EndRow = 24
)
$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName System.Drawing
$PttImage = [System.Drawing.Bitmap]::new($PngFilePath)
try {
    if ($ReferenceFilePath) {
        if ($StartRow -lt 0 -or $StartRow -ge $EndRow -or $EndRow -gt 24) { throw 'Invalid row range' }
        $PttReference = [System.Drawing.Bitmap]::new($ReferenceFilePath)
        try {
            function Get-RegionHash([System.Drawing.Bitmap]$Bitmap, [int]$Top, [int]$Height) {
                $PttRect = [System.Drawing.Rectangle]::new(0, $Top, $Bitmap.Width, $Height)
                $PttBits = $Bitmap.LockBits($PttRect, [System.Drawing.Imaging.ImageLockMode]::ReadOnly, [System.Drawing.Imaging.PixelFormat]::Format32bppArgb)
                try {
                    if ($PttBits.Stride -ne $Bitmap.Width * 4) { throw 'Unexpected pixel stride' }
                    $PttBytes = [byte[]]::new($PttBits.Stride * $Height)
                    [Runtime.InteropServices.Marshal]::Copy($PttBits.Scan0, $PttBytes, 0, $PttBytes.Length)
                    $PttHash = [Security.Cryptography.SHA256]::Create()
                    try { return [Convert]::ToBase64String($PttHash.ComputeHash($PttBytes)) }
                    finally { $PttHash.Dispose() }
                } finally { $Bitmap.UnlockBits($PttBits) }
            }
            $PttHeight = ($EndRow - $StartRow) * 24
            if ($PttImage.Width -ne 960 -or $PttImage.Height -ne $PttHeight -or $PttReference.Width -ne 960 -or $PttReference.Height -ne 576) { throw 'Unexpected crop dimensions' }
            $PttMatches = (Get-RegionHash $PttImage 0 $PttHeight) -eq (Get-RegionHash $PttReference ($StartRow * 24) $PttHeight)
            @{ width = $PttImage.Width; height = $PttImage.Height; region_matches_reference = $PttMatches } | ConvertTo-Json -Compress
        } finally { $PttReference.Dispose() }
        return
    }
    $PttSamples = @()
    foreach ($PttPoint in @(@(1,73), @(13,73), @(5,461), @(1,455))) {
        $PttColour = $PttImage.GetPixel($PttPoint[0], $PttPoint[1])
        $PttSamples += ,@([int]$PttColour.R, [int]$PttColour.G, [int]$PttColour.B, [int]$PttColour.A)
    }
    @{ width = $PttImage.Width; height = $PttImage.Height; samples = $PttSamples } | ConvertTo-Json -Compress -Depth 4
} finally {
    $PttImage.Dispose()
}
