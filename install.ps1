#!/usr/bin/env pwsh

$version = 'v0.8'
$APP = 'maple'
$url = "https://github.com/liuchengxu/vim-clap/releases/download/$version/$APP-"
$output = "$PSScriptRoot\bin\$APP.exe"

if ([Environment]::Is64BitOperatingSystem) {
  $url += 'x86_64-pc-windows-msvc'
} else {
  echo 'No prebuilt maple binary for 32-bit Windows system'
  Exit 1
}

if (Test-Path -LiteralPath $output) {
  Remove-Item -Force -LiteralPath $output
}

echo "Downloading $url, please wait a second......"

$start_time = Get-Date

(New-Object System.Net.WebClient).DownloadFile($url, $output)

Write-Output "Download the maple binary successfully, time taken: $((Get-Date).Subtract($start_time).Seconds) second(s)"
