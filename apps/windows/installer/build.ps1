<#
.SYNOPSIS
    构建可分发的 Vakyu Windows x64 离线安装包。
.DESCRIPTION
    使用固定目标目录、白名单资源与独立分发输入，不读取用户配置或密钥。
    语音、数据、VC++ 运行库和第三方声明默认来自工作区 outputs/vakyu-distribution-input。
    -SkipBuild 仍检查二进制架构和 uiAccess=false，不能把本机自签版本误装进公开包。
#>
[CmdletBinding()]
param(
    [switch]$SkipBuild,
    [switch]$StageOnly,
    [switch]$Sign,
    [string]$X64BuildDirectory,
    [string]$X86BuildDirectory,
    [string]$SettingsRuntimeDirectory,
    [string]$VoiceRuntimeDirectory,
    [string]$DataDirectory,
    [string]$VCRedistDirectory,
    [string]$NoticesDirectory,
    [string]$PackageVersion,
    [ValidatePattern('^distribution-stage-[A-Za-z0-9-]+$')][string]$StageName = ('distribution-stage-' + [Guid]::NewGuid().ToString('N')),
    [ValidateRange(1, 4)][int]$Jobs = 2
)

$ErrorActionPreference = 'Stop'
$Repo = (Resolve-Path (Join-Path $PSScriptRoot '..\..\..')).Path
$DistributionInput = Join-Path (Split-Path $Repo -Parent) 'outputs\vakyu-distribution-input'
$InstallerRoot = Join-Path $Repo 'target\installer'
$Stage = Join-Path $InstallerRoot $StageName
$Payload = Join-Path $Stage 'payload'
$Prerequisites = Join-Path $Stage 'prerequisites'
if ($Sign) { throw '公开分发包不接受本机自签证书。请移除 -Sign；本机调试请单独使用 sign-local.ps1。' }
if (-not $X64BuildDirectory) { $X64BuildDirectory = Join-Path $Repo 'target\x86_64-pc-windows-msvc\release' }
if (-not $X86BuildDirectory) { $X86BuildDirectory = Join-Path $Repo 'target\i686-pc-windows-msvc\release' }
if (-not $SettingsRuntimeDirectory) { $SettingsRuntimeDirectory = $X64BuildDirectory }
if (-not $VoiceRuntimeDirectory) { $VoiceRuntimeDirectory = Join-Path $DistributionInput 'voice-runtime' }
if (-not $DataDirectory) { $DataDirectory = Join-Path $DistributionInput 'data' }
if (-not $VCRedistDirectory) { $VCRedistDirectory = Join-Path $DistributionInput 'prerequisites' }
if (-not $NoticesDirectory) { $NoticesDirectory = Join-Path $DistributionInput 'notices' }

function Require-File([string]$Path) {
    if (-not (Test-Path -LiteralPath $Path -PathType Leaf) -or (Get-Item -LiteralPath $Path).Length -eq 0) {
        throw "缺少必需文件或文件为空：$Path"
    }
}

function Copy-RequiredFile([string]$Source, [string]$Destination) {
    Require-File $Source
    New-Item -ItemType Directory -Path (Split-Path $Destination -Parent) -Force | Out-Null
    Copy-Item -LiteralPath $Source -Destination $Destination -Force
}

function Copy-CleanTree([string]$Source, [string]$Destination) {
    if (-not (Test-Path -LiteralPath $Source -PathType Container)) { throw "缺少必需目录：$Source" }
    $sourceRoot = (Resolve-Path -LiteralPath $Source).Path.TrimEnd('\')
    if ((Get-Item -LiteralPath $sourceRoot).Attributes -band [IO.FileAttributes]::ReparsePoint) {
        throw "分发输入不能是链接目录：$sourceRoot"
    }
    $entries = @(Get-ChildItem -LiteralPath $sourceRoot -Recurse -Force)
    foreach ($entry in $entries) {
        if ($entry.Attributes -band [IO.FileAttributes]::ReparsePoint) { throw "分发输入不能含链接：$($entry.FullName)" }
        if ($entry.PSIsContainer) { continue }
        $relative = $entry.FullName.Substring($sourceRoot.Length + 1)
        if ($relative -match '(^|\\)(__pycache__|tests?|logs?|recordings?|\.git)(\\|$)' -or $relative -match '\.(pyc|pyo|wav|mp3|log)$') { continue }
        if ($entry.Name -match '^(\.env.*|config\.toml|.*\.(pfx|p12|key))$') { throw "分发输入含私人配置或凭据文件：$relative" }
        # py.typed / __init__.py 等运行库文件允许为空；固定必需入口仍用 Require-File 验证。
        $targetFile = Join-Path $Destination $relative
        New-Item -ItemType Directory -Path (Split-Path $targetFile -Parent) -Force | Out-Null
        Copy-Item -LiteralPath $entry.FullName -Destination $targetFile -Force
    }
}

function Assert-PeArchitecture([string]$Path, [uint16]$Expected) {
    Require-File $Path
    $stream = [IO.File]::OpenRead($Path)
    $reader = New-Object IO.BinaryReader($stream)
    try {
        if ($reader.ReadUInt16() -ne 0x5a4d) { throw "不是 Windows PE 文件：$Path" }
        $stream.Position = 0x3c
        $offset = $reader.ReadUInt32()
        $stream.Position = $offset
        if ($reader.ReadUInt32() -ne 0x4550 -or $reader.ReadUInt16() -ne $Expected) {
            throw "二进制架构错误：$Path"
        }
    } finally { $reader.Dispose() }
}

$env:QINGJIAN_UIACCESS = '0'
if (-not $SkipBuild) {
    Write-Host '构建 x64 Server / 设置 / TSF，以及 x86 TSF（uiAccess=false）…' -ForegroundColor Cyan
    $buildProcess = [Diagnostics.Process]::GetCurrentProcess()
    $oldPriority = $buildProcess.PriorityClass
    Push-Location $Repo
    try {
        $buildProcess.PriorityClass = 'BelowNormal'
        cargo build --release --locked --jobs $Jobs --target x86_64-pc-windows-msvc -p qingjian-windows-server -p qingjian-windows-tsf -p qingjian-windows-settings
        if ($LASTEXITCODE -ne 0) { throw "x64 cargo build 失败：$LASTEXITCODE" }
        cargo build --release --locked --jobs $Jobs --target i686-pc-windows-msvc -p qingjian-windows-tsf
        if ($LASTEXITCODE -ne 0) { throw "x86 cargo build 失败：$LASTEXITCODE" }
    } finally {
        $buildProcess.PriorityClass = $oldPriority
        Pop-Location
    }
}

$server = Join-Path $X64BuildDirectory 'qingjian-server.exe'
Assert-PeArchitecture $server 0x8664
Assert-PeArchitecture (Join-Path $X64BuildDirectory 'qingjian-settings.exe') 0x8664
Assert-PeArchitecture (Join-Path $X64BuildDirectory 'qingjian_tsf.dll') 0x8664
Assert-PeArchitecture (Join-Path $X86BuildDirectory 'qingjian_tsf.dll') 0x14c
# MSVC 嵌入的 manifest 是 UTF-8 XML；同时扫描 UTF-16，拒绝拿错本机 uiAccess 产物。
$serverBytes = [IO.File]::ReadAllBytes($server)
$manifestText = [Text.Encoding]::UTF8.GetString($serverBytes) + [Text.Encoding]::Unicode.GetString($serverBytes)
if ($manifestText -match 'uiAccess\s*=\s*["'']true["'']' -or $manifestText -notmatch 'uiAccess\s*=\s*["'']false["'']') {
    throw 'Server manifest 必须为 uiAccess=false。请使用 QINGJIAN_UIACCESS=0 重新构建，不能分发本机自签 uiAccess 产物。'
}
$serverBytes = $null
$manifestText = $null

$versionLine = Get-Content (Join-Path $Repo 'apps\windows\server\Cargo.toml') -Encoding UTF8 | Where-Object { $_ -match '^\s*version\s*=\s*"(.+)"' } | Select-Object -First 1
if (-not ($versionLine -match '"(.+)"')) { throw '未找到 Windows Server 版本号' }
$Version = $Matches[1]
if ($PackageVersion) { $Version = $PackageVersion }
elseif ($Version.EndsWith('-dev')) {
    $revision = git -C $Repo rev-parse --short HEAD 2>$null
    if ($LASTEXITCODE -eq 0 -and $revision) {
        if (git -C $Repo status --porcelain 2>$null) { $revision = "$revision+" }
        $Version = "$Version-$revision"
    }
}
if ($Version -notmatch '^\d+\.\d+\.\d+(?:[.+-][A-Za-z0-9.+-]+)?$') { throw "不支持的安装包版本：$Version" }
$VersionNumeric = $Version -replace '[-+].*$', ''
if ($VersionNumeric -notmatch '^\d+\.\d+\.\d+(\.\d+)?$') { throw '安装包数字版本必须是三段或四段数字' }
$TsfDll = "qingjian_tsf-$Version.dll"
$TsfDll32 = "qingjian_tsf-$Version-x86.dll"

# 每次使用新的 stage，保留已验证产物，不递归删除上一次的完整安装内容。
$stageAbsolute = [IO.Path]::GetFullPath($Stage)
$installerAbsolute = [IO.Path]::GetFullPath($InstallerRoot).TrimEnd('\') + '\'
if (-not $stageAbsolute.StartsWith($installerAbsolute, [StringComparison]::OrdinalIgnoreCase) -or $stageAbsolute -eq $installerAbsolute.TrimEnd('\')) { throw '无效的暂存目录' }
New-Item -ItemType Directory -Path $InstallerRoot -Force | Out-Null
foreach ($outputParent in @($Repo, (Join-Path $Repo 'target'), $InstallerRoot)) {
    if ((Get-Item -LiteralPath $outputParent).Attributes -band [IO.FileAttributes]::ReparsePoint) { throw "安装包输出路径不能经过链接：$outputParent" }
}
if (Test-Path -LiteralPath $Stage) {
    throw '暂存目录已存在，请省略 StageName 生成新目录，或提供另一个新目录名。'
}
New-Item -ItemType Directory -Path $Payload, $Prerequisites -Force | Out-Null
Copy-RequiredFile $server (Join-Path $Payload 'qingjian-server.exe')
Copy-RequiredFile (Join-Path $X64BuildDirectory 'qingjian-settings.exe') (Join-Path $Payload 'qingjian-settings.exe')
Copy-RequiredFile (Join-Path $X64BuildDirectory 'qingjian_tsf.dll') (Join-Path $Payload $TsfDll)
Copy-RequiredFile (Join-Path $X86BuildDirectory 'qingjian_tsf.dll') (Join-Path $Payload $TsfDll32)
Copy-RequiredFile (Join-Path $Repo 'assets\icon\windows\vakyu.ico') (Join-Path $Payload 'vakyu.ico')

$runtimeList = Get-Content (Join-Path $PSScriptRoot 'settings-runtime.txt') -Encoding UTF8 | ForEach-Object { $_.Trim() } | Where-Object { $_ -and -not $_.StartsWith('#') }
foreach ($name in $runtimeList) {
    if ($name -match '[/\\:]|^\.') { throw "运行时白名单不能含路径：$name" }
    $source = Join-Path $SettingsRuntimeDirectory $name
    $destination = Join-Path $Payload $name
    if (Test-Path -LiteralPath $source -PathType Container) { Copy-CleanTree $source $destination }
    else { Copy-RequiredFile $source $destination }
}

foreach ($name in @('dict.qj', 'lm.qj', 'glossary-en.qj', 'glossary-ja.qj', 'glossary-zh.qj', 'glossary-es.qj', 'english.tsv')) {
    Copy-RequiredFile (Join-Path $DataDirectory $name) (Join-Path $Payload "data\generated\$name")
}
foreach ($subdir in @('dicts', 'codes')) {
    $tables = @(Get-ChildItem -LiteralPath (Join-Path $DataDirectory $subdir) -Filter '*.qj' -File | Where-Object { -not $_.Name.StartsWith('._') })
    if ($tables.Count -eq 0) { throw "缺少随包词表：$subdir\*.qj" }
    foreach ($table in $tables) { Copy-RequiredFile $table.FullName (Join-Path $Payload "data\generated\$subdir\$($table.Name)") }
}
Copy-RequiredFile (Join-Path $Repo 'assets\stroke\LICENSE-CNS11643.txt') (Join-Path $Payload 'data\generated\codes\LICENSE-CNS11643.txt')
foreach ($asset in @('emoji\emoji-zh.tsv', 'emoji\emoji-en.tsv', 'levels\levels-en.tsv', 'levels\levels-ja.tsv', 'wubi\wubi86.tsv', 'sample\dict.tsv')) {
    Copy-RequiredFile (Join-Path $Repo "assets\$asset") (Join-Path $Payload "assets\$asset")
}
# 本地整句重排模型为可选增强，基础输入与随包中英语音不依赖它。
foreach ($model in @('hanzhang-zhiwei\hanzhang-zhiwei-small.qjm', 'hanzhang-tongbian\hanzhang-tongbian-small.qjm')) {
    $source = Join-Path $Repo "data\models\$model"
    if (Test-Path -LiteralPath $source -PathType Leaf) { Copy-RequiredFile $source (Join-Path $Payload "data\models\$model") }
}

foreach ($required in @('python\python.exe', 'python\pythonw.exe', 'python\python312.dll', 'python\python312.zip', 'python\python312._pth', 'app\main.py', 'app\translate_main.py', 'models\streaming\encoder.int8.onnx', 'models\streaming\decoder.onnx', 'models\streaming\joiner.int8.onnx', 'models\streaming\tokens.txt', 'models\translate\zh-en\model.bin', 'models\translate\en-zh\model.bin')) {
    Require-File (Join-Path $VoiceRuntimeDirectory $required)
}
Assert-PeArchitecture (Join-Path $VoiceRuntimeDirectory 'python\python.exe') 0x8664
foreach ($folder in @('python', 'app', 'models', 'licenses')) {
    Copy-CleanTree (Join-Path $VoiceRuntimeDirectory $folder) (Join-Path $Payload "voice-runtime\$folder")
}
foreach ($metadata in @('sources.json', 'models-manifest.json', 'translation-sources.json', 'translation-models.json', 'requirements-win64.lock')) {
    Copy-RequiredFile (Join-Path $VoiceRuntimeDirectory $metadata) (Join-Path $Payload "voice-runtime\$metadata")
}
Require-File (Join-Path $NoticesDirectory 'INSTALL-LICENSE.txt')
Copy-CleanTree $NoticesDirectory (Join-Path $Payload 'licenses')
if (@(Get-ChildItem (Join-Path $Payload 'licenses') -File -Recurse).Count -eq 0) { throw '第三方声明目录不能为空' }
foreach ($architecture in @('x64', 'x86')) {
    $redist = Join-Path $VCRedistDirectory "vc_redist.$architecture.exe"
    Require-File $redist
    $signature = Get-AuthenticodeSignature -LiteralPath $redist
    if ($signature.Status -ne 'Valid' -or $signature.SignerCertificate.Subject -notmatch 'Microsoft Corporation') {
        throw "VC++ 运行库必须有有效的 Microsoft 签名：$redist"
    }
    Copy-RequiredFile $redist (Join-Path $Prerequisites "vc_redist.$architecture.exe")
}

$manifest = @(Get-ChildItem -LiteralPath $Payload -File -Recurse | Sort-Object FullName | ForEach-Object {
    [ordered]@{ path = $_.FullName.Substring($Payload.Length + 1).Replace('\', '/'); bytes = $_.Length; sha256 = (Get-FileHash -LiteralPath $_.FullName -Algorithm SHA256).Hash.ToLowerInvariant() }
})
[ordered]@{ product = 'Vakyu'; version = $Version; architecture = 'x64'; ui_access = $false; files = $manifest } | ConvertTo-Json -Depth 5 | Set-Content -LiteralPath (Join-Path $Payload 'distribution-manifest.json') -Encoding UTF8
Write-Host "离线安装内容已准备：$Payload（$($manifest.Count) 个文件）" -ForegroundColor Cyan
if ($StageOnly) { return }

$iscc = $env:QINGJIAN_ISCC
if (-not $iscc) {
    $candidates = @("${env:ProgramFiles}\Inno Setup 7\ISCC.exe", "${env:ProgramFiles(x86)}\Inno Setup 7\ISCC.exe", "${env:ProgramFiles}\Inno Setup 6\ISCC.exe", "${env:ProgramFiles(x86)}\Inno Setup 6\ISCC.exe")
    $iscc = $candidates | Where-Object { Test-Path -LiteralPath $_ } | Select-Object -First 1
}
if (-not $iscc) { $iscc = (Get-Command iscc.exe -ErrorAction SilentlyContinue).Source }
if (-not $iscc) { throw '找不到 ISCC.exe，请安装 Inno Setup 或用 QINGJIAN_ISCC 指定其路径。' }
& $iscc "/DAppVersion=$Version" "/DAppVersionNumeric=$VersionNumeric" "/DStageRoot=$Stage" "/DOutputDirectory=$InstallerRoot" (Join-Path $PSScriptRoot 'miracle.iss')
if ($LASTEXITCODE -ne 0) { throw "Inno Setup 编译失败：$LASTEXITCODE" }
$output = Join-Path $InstallerRoot "vakyu-$Version-windows-x86_64-setup.exe"
Require-File $output
$checksum = (Get-FileHash -LiteralPath $output -Algorithm SHA256).Hash.ToLowerInvariant()
"$checksum  $([IO.Path]::GetFileName($output))" | Set-Content -LiteralPath "$output.sha256" -Encoding ASCII
Write-Host "安装包完成：$output" -ForegroundColor Green
