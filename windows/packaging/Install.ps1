$ErrorActionPreference = 'Stop'
$package = Join-Path $PSScriptRoot 'MyOCR-x64.msix'
$certificate = Join-Path $PSScriptRoot 'MyOCR-Test.cer'
$cert = [System.Security.Cryptography.X509Certificates.X509Certificate2]::new($certificate)
Write-Host 'MyOCR test build: Windows 11 x64. This installs for the current user.'
Write-Host "Test certificate SHA1: $($cert.Thumbprint)"
Write-Host 'Trust this certificate only if you downloaded this build from wynemo/myocr.'
if (!(Test-Path "Cert:\LocalMachine\TrustedPeople\$($cert.Thumbprint)")) {
    # Elevate only certificate import. App installation stays in this user's session.
    $escaped = $certificate.Replace("'", "''")
    $command = "Import-Certificate -FilePath '$escaped' -CertStoreLocation Cert:\LocalMachine\TrustedPeople -ErrorAction Stop | Out-Null"
    $encoded = [Convert]::ToBase64String([Text.Encoding]::Unicode.GetBytes($command))
    $process = Start-Process powershell.exe -Verb RunAs -Wait -PassThru `
        -ArgumentList @('-NoProfile', '-EncodedCommand', $encoded)
    if ($process.ExitCode -ne 0) { throw 'Certificate import failed or was cancelled.' }
}
Get-Process myocr -ErrorAction SilentlyContinue | Stop-Process
Add-AppxPackage -Path $package
Write-Host 'Installed. Open MyOCR from the Start menu, then press Ctrl+Alt+O.'
Write-Host 'Each CI build currently has a new test certificate. Updates may ask for trust again.'
