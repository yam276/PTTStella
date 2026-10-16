param(
    [Parameter(Mandatory = $true)][string]$PngFilePath,
    [string]$ReferenceFilePath,
    [int]$StartRow = 0,
    [int]$EndRow = 24,
    [switch]$CheckSymbols,
    [switch]$CheckMedia
)
$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName System.Drawing
$PttImage = [System.Drawing.Bitmap]::new($PngFilePath)
try {
    if ($CheckMedia) {
        $PttPixel = $PttImage.GetPixel([int]($PttImage.Width / 2), [int]($PttImage.Height / 2))
        @{ rgba = @([int]$PttPixel.R, [int]$PttPixel.G, [int]$PttPixel.B, [int]$PttPixel.A); width = $PttImage.Width; height = $PttImage.Height } | ConvertTo-Json -Compress
        return
    }
    if ($CheckSymbols) {
        if ($PttImage.Width -ne 960 -or $PttImage.Height -ne 576) { throw 'Unexpected symbol image dimensions' }
        $PttSymbols = @()
        for ($PttSymbol = 0; $PttSymbol -lt 4; $PttSymbol++) {
            $PttLeft = 24; $PttRight = -1; $PttTop = 24; $PttBottom = -1
            for ($PttY = 0; $PttY -lt 24; $PttY++) {
                for ($PttX = 0; $PttX -lt 24; $PttX++) {
                    $PttPixel = $PttImage.GetPixel($PttSymbol * 24 + $PttX, 12 * 24 + $PttY)
                    if ($PttPixel.R -gt 60 -and $PttPixel.G -gt 60 -and $PttPixel.B -gt 60) {
                        $PttLeft = [Math]::Min($PttLeft, $PttX); $PttRight = [Math]::Max($PttRight, $PttX)
                        $PttTop = [Math]::Min($PttTop, $PttY); $PttBottom = [Math]::Max($PttBottom, $PttY)
                    }
                }
            }
            $PttSymbols += ,@(($PttRight - $PttLeft + 1), ($PttBottom - $PttTop + 1))
        }
        @{ symbols = $PttSymbols } | ConvertTo-Json -Compress -Depth 4
        return
    }
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
