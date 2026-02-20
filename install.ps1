$ErrorActionPreference = "Stop"

$BaseUrl = "https://downloads.blazedb.online"
$File = "blazedb-windows-x86_64.exe"
$Url = "$BaseUrl/releases/$File"
$ChecksumUrl = "$Url.sha256"

$InstallDir = "$env:USERPROFILE\.blazedb\bin"
$BinaryPath = "$InstallDir\blazedb.exe"

New-Item -ItemType Directory -Force -Path $InstallDir | Out-Null

Write-Host "Downloading blazedb..."

Invoke-WebRequest $Url -OutFile $BinaryPath
Invoke-WebRequest $ChecksumUrl -OutFile "$BinaryPath.sha256"

$ExpectedHash = (Get-Content "$BinaryPath.sha256").Split(" ")[0].ToLower()
$ActualHash = (Get-FileHash $BinaryPath -Algorithm SHA256).Hash.ToLower()

if ($ExpectedHash -ne $ActualHash)
{
    Write-Error "Checksum verification failed!"
    Remove-Item $BinaryPath -ErrorAction Ignore
    exit 1
}

$UserPath = [Environment]::GetEnvironmentVariable("Path", "User")

if ($UserPath -notlike "*$InstallDir*")
{
    [Environment]::SetEnvironmentVariable(
            "Path",
            "$UserPath;$InstallDir",
            "User"
    )
}

Write-Host "Installed successfully. Restart terminal."