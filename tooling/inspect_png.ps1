param([Parameter(Mandatory = $true)][string]$PngFilePath)
$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName System.Drawing
$PttImage = [System.Drawing.Bitmap]::new($PngFilePath)
try {
    $PttSamples = @()
    foreach ($PttPoint in @(@(1,73), @(13,73), @(5,461), @(1,455))) {
        $PttColour = $PttImage.GetPixel($PttPoint[0], $PttPoint[1])
        $PttSamples += ,@([int]$PttColour.R, [int]$PttColour.G, [int]$PttColour.B, [int]$PttColour.A)
    }
    @{ width = $PttImage.Width; height = $PttImage.Height; samples = $PttSamples } | ConvertTo-Json -Compress -Depth 4
} finally {
    $PttImage.Dispose()
}
