$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest
$root = Split-Path $PSScriptRoot -Parent
$out = Join-Path $root 'dist/portable'
New-Item -ItemType Directory -Force $out | Out-Null
Copy-Item "$root/target/release/myocr.exe" "$out/myocr.exe"
Copy-Item "$root/README.md" "$out/README.md"
Copy-Item "$root/THIRD_PARTY_NOTICES.md" "$out/THIRD_PARTY_NOTICES.md"
Get-FileHash -Algorithm SHA256 -Path "$out/myocr.exe", "$out/README.md", "$out/THIRD_PARTY_NOTICES.md" |
    ForEach-Object { "$($_.Hash)  $(Split-Path $_.Path -Leaf)" } |
    Set-Content "$out/SHA256SUMS.txt"
# upload-artifact wraps these four files in the downloadable portable ZIP.
