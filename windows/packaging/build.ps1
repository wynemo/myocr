$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest
$root = Split-Path $PSScriptRoot -Parent
$stage = Join-Path $root 'target/package'
$out = Join-Path $root 'dist'
New-Item -ItemType Directory -Force "$stage/Assets", $out | Out-Null
Copy-Item "$root/target/release/myocr.exe" "$stage/myocr.exe"
Copy-Item "$PSScriptRoot/AppxManifest.xml" "$stage/AppxManifest.xml"
# CI run number gives test packages a monotonically increasing version.
$version = if ($env:GITHUB_RUN_NUMBER) { "0.1.$($env:GITHUB_RUN_NUMBER).$($env:GITHUB_RUN_ATTEMPT)" } else { '0.1.0.0' }
[xml]$manifest = Get-Content "$stage/AppxManifest.xml"
$manifest.Package.Identity.Version = $version
$manifest.Save("$stage/AppxManifest.xml")

Add-Type -AssemblyName System.Drawing
foreach ($asset in @(@('StoreLogo', 50), @('Square44x44Logo', 44), @('Square150x150Logo', 150))) {
    $size = [int]$asset[1]
    $bitmap = [System.Drawing.Bitmap]::new($size, $size)
    $graphics = [System.Drawing.Graphics]::FromImage($bitmap)
    $graphics.Clear([System.Drawing.Color]::FromArgb(21, 94, 117))
    $font = [System.Drawing.Font]::new('Segoe UI', [single]($size * 0.28), [System.Drawing.FontStyle]::Bold)
    $format = [System.Drawing.StringFormat]::new()
    $format.Alignment = [System.Drawing.StringAlignment]::Center
    $format.LineAlignment = [System.Drawing.StringAlignment]::Center
    $graphics.DrawString('OCR', $font, [System.Drawing.Brushes]::White, [System.Drawing.RectangleF]::new(0, 0, $size, $size), $format)
    $bitmap.Save("$stage/Assets/$($asset[0]).png", [System.Drawing.Imaging.ImageFormat]::Png)
    $format.Dispose(); $font.Dispose(); $graphics.Dispose(); $bitmap.Dispose()
}
$sdk = Get-ChildItem "${env:ProgramFiles(x86)}/Windows Kits/10/bin" -Directory |
    Where-Object { Test-Path "$($_.FullName)/x64/makeappx.exe" } |
    Sort-Object Name -Descending | Select-Object -First 1
if (!$sdk) { throw 'Windows SDK packaging tools not found.' }
$tools = Join-Path $sdk.FullName 'x64'
& "$tools/makeappx.exe" pack /d $stage /p "$out/MyOCR-x64.msix" /o
if ($LASTEXITCODE) { throw 'makeappx failed' }
# A short-lived test certificate: only the public certificate is distributed.
$cert = New-SelfSignedCertificate -Type Custom -Subject 'CN=MyOCR Test' -FriendlyName 'MyOCR CI test signing' `
    -CertStoreLocation 'Cert:\CurrentUser\My' -KeyAlgorithm RSA -KeyLength 2048 -HashAlgorithm SHA256 `
    -KeyUsage DigitalSignature -NotAfter (Get-Date).AddMonths(6) `
    -TextExtension @('2.5.29.37={text}1.3.6.1.5.5.7.3.3', '2.5.29.19={text}')
try {
    & "$tools/signtool.exe" sign /fd SHA256 /sha1 $cert.Thumbprint "$out/MyOCR-x64.msix"
    if ($LASTEXITCODE) { throw 'signtool failed' }
    Export-Certificate -Cert $cert -FilePath "$out/MyOCR-Test.cer" | Out-Null
} finally {
    Remove-Item "Cert:\CurrentUser\My\$($cert.Thumbprint)" -DeleteKey
}
Copy-Item "$PSScriptRoot/Install.ps1", "$root/README.md" $out
Get-ChildItem $out -File | Get-FileHash -Algorithm SHA256 |
    ForEach-Object { "$($_.Hash)  $(Split-Path $_.Path -Leaf)" } |
    Set-Content "$out/SHA256SUMS.txt"
