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
$speechModels = Get-Content (Join-Path $PSScriptRoot 'speech-models.json') -Raw -Encoding UTF8 | ConvertFrom-Json
$translation = Get-Content (Join-Path $PSScriptRoot 'translation-sources.json') -Raw -Encoding UTF8 | ConvertFrom-Json
$translationModels = @(Get-Content (Join-Path $PSScriptRoot 'translation-models.json') -Raw -Encoding UTF8 | ConvertFrom-Json)
if (@($translation.packages).Count -eq 0 -or $translationModels.Count -eq 0) {
    throw "离线翻译组件尚未固定版本：先在能上网的构建机运行 py -3.12 pin_translation.py --cache <与本次相同的缓存目录>，再提交 translation-*.json"
}
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
foreach ($package in @($sources.packages) + @($translation.packages)) {
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
# 离线翻译模型由 pin_translation.py 从固定版本的官方权重转换到缓存目录，这里逐个按清单校验后复制。
$translateCache = Join-Path $cache 'translate-models'
foreach ($model in $translationModels) {
    $relative = $model.path -replace '^models/translate/', ''
    $source = Join-Path $translateCache $relative
    if (-not (Test-Path -LiteralPath $source -PathType Leaf)) {
        throw "缺少离线翻译模型文件：$relative；请用同一缓存目录重新运行 pin_translation.py"
    }
    if ((Get-Item -LiteralPath $source).Length -ne $model.size -or
        (Get-FileHash -LiteralPath $source).Hash -ne $model.sha256) { throw "离线翻译模型校验失败：$relative" }
    $target = Join-Path $output $model.path
    New-Item -ItemType Directory -Path (Split-Path -Parent $target) -Force | Out-Null
    Copy-Item -LiteralPath $source -Destination $target
}
# 自然人声（Piper 英语模型）：归档哈希已固定；只取模型和英语音标数据，再按清单校验关键文件。
$speechArchive = Get-VerifiedArchive $sources.speech_model.url $sources.speech_model.archive_sha256 'speech-en.tar.bz2'
$speechDir = Join-Path $output 'models/speech/en'
& (Join-Path $pythonDir 'python.exe') -I -B (Join-Path $PSScriptRoot 'extract_speech.py') $speechArchive $sources.speech_model.archive_root $speechDir
if ($LASTEXITCODE -ne 0) { throw '自然人声模型解压失败' }
foreach ($model in $speechModels) {
    $path = Join-Path $output $model.path
    if (-not (Test-Path -LiteralPath $path -PathType Leaf) -or
        (Get-Item -LiteralPath $path).Length -ne $model.size -or
        (Get-FileHash -LiteralPath $path).Hash -ne $model.sha256) { throw "自然人声模型校验失败：$($model.path)" }
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
Copy-Item (Join-Path $PSScriptRoot 'translation-sources.json') (Join-Path $output 'translation-sources.json')
Copy-Item (Join-Path $PSScriptRoot 'translation-models.json') (Join-Path $output 'translation-models.json')
Copy-Item (Join-Path $PSScriptRoot 'speech-models.json') (Join-Path $output 'speech-models.json')
$lock = @(@($sources.packages) + @($translation.packages) | ForEach-Object { "$($_.name)==$($_.version) --hash=sha256:$($_.sha256)" })
[IO.File]::WriteAllLines((Join-Path $output 'requirements-win64.lock'), $lock, [Text.Encoding]::ASCII)

# 默认 Windows 音频后端不需要 ASIO，避免额外分发未使用的 Steinberg 组件。
$asio = Join-Path $siteDir '_sounddevice_data/portaudio-binaries/libportaudio64bit-asio.dll'
if (Test-Path -LiteralPath $asio) { Remove-Item -LiteralPath $asio -Force }

# 预生成非个人 BPE 数据，安装到 Program Files 后只读运行。
$env:PYTHONDONTWRITEBYTECODE = '1'
& (Join-Path $pythonDir 'python.exe') -B -c 'import sys; from pathlib import Path; from bpe_vocab import prepare_bpe_vocab; assert prepare_bpe_vocab(Path(sys.argv[1])) is not None' $modelDir
if ($LASTEXITCODE -ne 0) { throw 'BPE 预生成失败' }
# 离线翻译原生库必须能在随包 Python 里加载，否则安装后才发现翻译不可用。
& (Join-Path $pythonDir 'python.exe') -I -B -c 'import ctranslate2, sentencepiece'
if ($LASTEXITCODE -ne 0) { throw '离线翻译依赖无法在随包 Python 中加载' }
& (Join-Path $pythonDir 'python.exe') -I -B -c 'import sherpa_onnx; assert hasattr(sherpa_onnx, "OfflineTts")'
if ($LASTEXITCODE -ne 0) { throw '随包 sherpa-onnx 不支持语音合成，自然人声不可用' }
$licenseDir = Join-Path $output 'licenses'
New-Item -ItemType Directory -Path $licenseDir -Force | Out-Null
$notice = @('离线翻译模型来自 Helsinki-NLP OPUS-MT，经 CTranslate2 转换为 int8，未修改模型内容：')
foreach ($model in $translation.models) {
    $notice += "- $($model.repo)（$($model.license)，版本 $($model.revision)）：$($model.upstream)"
}
$notice += 'Tiedemann, J. & Thottingal, S. (2020). OPUS-MT — Building open translation services for the World. EAMT 2020.'
[IO.File]::WriteAllLines((Join-Path $licenseDir 'OPUS-MT-NOTICE.txt'), $notice, (New-Object Text.UTF8Encoding $true))
$speechNotice = @(
    '英语示范的自然人声来自 Piper 语音模型 en_US libritts_r medium（Rhasspy，MIT），经 sherpa-onnx 打包，未修改模型内容：',
    "- $($sources.speech_model.upstream)",
    '- 训练数据 LibriTTS-R（CC BY 4.0）：Koizumi, Y. et al. (2023). LibriTTS-R: A Restored Multi-Speaker Text-to-Speech Corpus. Interspeech 2023. http://www.openslr.org/141/',
    '- 该模型由 Piper en_US lessac medium 微调而来；音标数据来自 espeak-ng（GPL-3.0-or-later），只保留英语词典。'
)
[IO.File]::WriteAllLines((Join-Path $licenseDir 'PIPER-VOICE-NOTICE.txt'), $speechNotice, (New-Object Text.UTF8Encoding $true))
Copy-Item (Join-Path $pythonDir 'LICENSE.txt') (Join-Path $licenseDir 'Python-LICENSE.txt')
Get-ChildItem -LiteralPath $siteDir -Directory -Filter '*.dist-info' | ForEach-Object {
    $packageName = $_.Name
    Get-ChildItem -LiteralPath $_.FullName -Recurse -File | Where-Object { $_.Name -match '^(LICENSE|COPYING|NOTICE)' } | ForEach-Object {
        Copy-Item -LiteralPath $_.FullName -Destination (Join-Path $licenseDir "$packageName-$($_.Name)")
    }
}
Write-Host "干净语音组件已准备：$output"
