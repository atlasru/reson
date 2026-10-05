$ErrorActionPreference = 'Stop'
$tag = '2026-10-04-c152964208'
$name = 'mpv-dev-lgpl-x86_64-20261004-git-c152964208.7z'
$expected = 'b656b1d3f18f3db619894c22b68023acabd856e9e026b2a8516f30f1a44ee0bc'
$root = Join-Path $PSScriptRoot '..'
$resources = Join-Path $root 'src-tauri/resources'
$archive = Join-Path $env:TEMP $name
$extract = Join-Path $env:TEMP ('reson-libmpv-' + $tag)
Invoke-WebRequest -Uri "https://github.com/zhongfly/mpv-winbuild/releases/download/$tag/$name" -OutFile $archive
if ((Get-FileHash $archive -Algorithm SHA256).Hash.ToLowerInvariant() -ne $expected) { throw 'libmpv archive checksum mismatch' }
New-Item -ItemType Directory -Force -Path $extract | Out-Null
& 7z x $archive "-o$extract" -y | Out-Null
if ($LASTEXITCODE -ne 0) { throw 'libmpv extraction failed' }
$dll = Get-ChildItem $extract -Filter 'libmpv-2.dll' -Recurse | Select-Object -First 1
if (-not $dll) { throw 'libmpv DLL absent from verified archive' }
Copy-Item $dll.FullName (Join-Path $resources 'libmpv-2.dll') -Force
$licenses = Join-Path $resources 'licenses'
New-Item -ItemType Directory -Force -Path $licenses | Out-Null
Get-ChildItem $extract -Recurse -File | Where-Object { $_.Name -match 'COPYING|LICENSE|copyright' } | ForEach-Object { Copy-Item $_.FullName (Join-Path $licenses $_.Name) -Force }
Write-Host 'Verified, pinned LGPL native audio installed.'
