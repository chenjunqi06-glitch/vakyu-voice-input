<#
.SYNOPSIS
    从固定哈希的官方归档生成干净的离线语音组件，不读取个人配置或运行目录。
#>
[CmdletBinding()]
param(
    [Parameter(Mandatory=$true)][string]$OutputDirectory,
    [Parameter(Mandatory=$true)][string]$CacheDirectory
)
$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName System.IO.Compression.FileSystem
$sources = Get-Content (Join-Path $PSScriptRoot 'voice-sources.json') -Raw -Encoding UTF8 | ConvertFrom-Json
$models = Get-Content (Join-Path $PSScriptRoot 'voice-models.json') -Raw -Encoding UTF8 | ConvertFrom-Json
$output = [IO.Path]::GetFullPath($OutputDirectory)
$cache = [IO.Path]::GetFullPath($CacheDirectory)
if (Test-Path -LiteralPath $output) {
    if (Get-ChildItem -LiteralPath $output -Force | Select-Object -First 1) {
        throw "输出目录必须为空，避免旧文件进入分发包：$output"
    }
}
New-Item -ItemType Directory -Path $output,$cache -Force | Out-Null

function Get-VerifiedArchive([string]$Url, [string]$Sha256, [string]$Name) {
    $path = Join-Path $cache $Name
    if (-not (Test-Path -LiteralPath $path)) {
        Write-Host "下载 $Name"
        Invoke-WebRequest -Uri $Url -OutFile $path -UseBasicParsing
    }
    if ((Get-FileHash -LiteralPath $path -Algorithm SHA256).Hash -ne $Sha256) {
        throw "归档校验失败：$Name；请检查缓存文件，未继续解压。"
    }
    return $path
}

$pythonZip = Get-VerifiedArchive $sources.python.url $sources.python.sha256 'python-embed.zip'
$pythonDir = Join-Path $output 'python'
[IO.Compression.ZipFile]::ExtractToDirectory($pythonZip, $pythonDir)
$siteDir = Join-Path $pythonDir 'Lib/site-packages'
New-Item -ItemType Directory -Path $siteDir -Force | Out-Null
foreach ($package in $sources.packages) {
    $filename = [IO.Path]::GetFileName(([Uri]$package.url).AbsolutePath)
    $wheel = Get-VerifiedArchive $package.url $package.sha256 $filename
    [IO.Compression.ZipFile]::ExtractToDirectory($wheel, $siteDir)
}
# Embeddable Python 采用封闭路径，不依赖 PATH、系统 Python 或开发环境。
[IO.File]::WriteAllText((Join-Path $pythonDir 'python312._pth'), "python312.zip`n.`nLib/site-packages`n../app`n", [Text.Encoding]::ASCII)

$modelArchive = Get-VerifiedArchive $sources.model.url $sources.model.archive_sha256 'streaming.tar.bz2'
$modelDir = Join-Path $output 'models/streaming'
New-Item -ItemType Directory -Path $modelDir -Force | Out-Null
$archiveRoot = 'sherpa-onnx-x-asr-480ms-streaming-zipformer-transducer-zh-en-punct-int8-2026-06-05'
foreach ($model in $models) {
    $name = [IO.Path]::GetFileName($model.path)
    # 用随包 Python 解 bzip2；Windows 的 tar 版本可能依赖未安装的 bzip2.exe。
    & (Join-Path $pythonDir 'python.exe') -I -B -c 'import sys,tarfile,shutil; a=tarfile.open(sys.argv[1]); src=a.extractfile(sys.argv[2]); out=open(sys.argv[3],"wb"); shutil.copyfileobj(src,out); out.close(); src.close(); a.close()' $modelArchive "$archiveRoot/$name" (Join-Path $modelDir $name)
    if ($LASTEXITCODE -ne 0) { throw "模型解压失败：$name" }
    $path = Join-Path $output $model.path
    if ((Get-Item -LiteralPath $path).Length -ne $model.size -or
        (Get-FileHash -LiteralPath $path).Hash -ne $model.sha256) { throw "模型校验失败：$name" }
}
$appDir = Join-Path $output 'app'
New-Item -ItemType Directory -Path $appDir -Force | Out-Null
$appSource = Join-Path $PSScriptRoot '../server/voice-runtime'
Get-ChildItem -LiteralPath $appSource -File -Filter '*.py' |
    Where-Object { $_.Name -notmatch '(^test|_tests[.]py$|^benchmark)' } |
    Copy-Item -Destination $appDir
Copy-Item (Join-Path $appSource 'README.md') $appDir
Copy-Item (Join-Path $appSource 'requirements.txt') $appDir
Copy-Item (Join-Path $PSScriptRoot 'voice-sources.json') (Join-Path $output 'sources.json')
Copy-Item (Join-Path $PSScriptRoot 'voice-models.json') (Join-Path $output 'models-manifest.json')
$lock = @($sources.packages | ForEach-Object { "$($_.name)==$($_.version) --hash=sha256:$($_.sha256)" })
[IO.File]::WriteAllLines((Join-Path $output 'requirements-win64.lock'), $lock, [Text.Encoding]::ASCII)

# 默认 Windows 音频后端不需要 ASIO，避免额外分发未使用的 Steinberg 组件。
$asio = Join-Path $siteDir '_sounddevice_data/portaudio-binaries/libportaudio64bit-asio.dll'
if (Test-Path -LiteralPath $asio) { Remove-Item -LiteralPath $asio -Force }

# 预生成非个人 BPE 数据，安装到 Program Files 后只读运行。
$env:PYTHONDONTWRITEBYTECODE = '1'
& (Join-Path $pythonDir 'python.exe') -B -c 'import sys; from pathlib import Path; from bpe_vocab import prepare_bpe_vocab; assert prepare_bpe_vocab(Path(sys.argv[1])) is not None' $modelDir
if ($LASTEXITCODE -ne 0) { throw 'BPE 预生成失败' }
$licenseDir = Join-Path $output 'licenses'
New-Item -ItemType Directory -Path $licenseDir -Force | Out-Null
Copy-Item (Join-Path $pythonDir 'LICENSE.txt') (Join-Path $licenseDir 'Python-LICENSE.txt')
Get-ChildItem -LiteralPath $siteDir -Directory -Filter '*.dist-info' | ForEach-Object {
    $packageName = $_.Name
    Get-ChildItem -LiteralPath $_.FullName -Recurse -File | Where-Object { $_.Name -match '^(LICENSE|COPYING|NOTICE)' } | ForEach-Object {
        Copy-Item -LiteralPath $_.FullName -Destination (Join-Path $licenseDir "$packageName-$($_.Name)")
    }
}
Write-Host "干净语音组件已准备：$output"
