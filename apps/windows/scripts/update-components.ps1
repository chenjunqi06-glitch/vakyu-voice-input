param(
    [Parameter(Mandatory=$true)][string]$ManifestPath,
    [Parameter(Mandatory=$true)][string]$ResultPath,
    [switch]$Plan,
    [string]$InvocationId,
    [string]$RuntimeRoot,
    [switch]$Apply
)
$ErrorActionPreference = 'Stop'
$installRoot = 'C:\Program Files\Qingjian'
$clsid = '{4FDCA82D-E923-49BF-9E75-BB906B93B8BB}'
$registry64 = "HKLM:\Software\Classes\CLSID\$clsid\InprocServer32"
$registry32 = "HKLM:\Software\Classes\WOW6432Node\CLSID\$clsid\InprocServer32"
$manifestFile = (Resolve-Path -LiteralPath $ManifestPath).Path
$resultFile = [IO.Path]::GetFullPath($ResultPath)
$manifest = Get-Content -LiteralPath $manifestFile -Raw | ConvertFrom-Json
if ($manifest.brand -and $manifest.brand -ne 'Vakyu') { throw 'Unsupported display brand' }
$updateBrand = $manifest.brand -eq 'Vakyu'
$callerRuntimeRoot = Join-Path ([Environment]::GetFolderPath('LocalApplicationData')) 'Qingjian\voice-runtime\app'
if (-not $Apply) {
    # 必须在提权前确定原用户目录，避免另一个管理员账户的 LOCALAPPDATA 改变目标。
    if ($RuntimeRoot -and [IO.Path]::GetFullPath($RuntimeRoot) -ne $callerRuntimeRoot) { throw 'Runtime root must belong to the calling user' }
    $RuntimeRoot = $callerRuntimeRoot
}
$old64 = (Get-ItemProperty -LiteralPath $registry64).'(default)'
$old32 = (Get-ItemProperty -LiteralPath $registry32).'(default)'

function File-Hash([string]$Path) {
    if (Test-Path -LiteralPath $Path -PathType Leaf) {
        return (Get-FileHash -LiteralPath $Path -Algorithm SHA256).Hash
    }
    return ''
}

function Runtime-Target([string]$Name) {
    if ([string]::IsNullOrWhiteSpace($RuntimeRoot) -or -not [IO.Path]::IsPathRooted($RuntimeRoot)) { throw 'Runtime update requires the original caller runtime root' }
    $root = [IO.Path]::GetFullPath($RuntimeRoot).TrimEnd('\')
    if (-not $root.EndsWith('\Qingjian\voice-runtime\app', [StringComparison]::OrdinalIgnoreCase)) { throw 'Invalid runtime root' }
    if ($Name -notmatch '^[a-zA-Z_][a-zA-Z0-9_-]*\.py$' -or $Name -match '^(CON|PRN|AUX|NUL|COM[0-9]|LPT[0-9])\.py$') { throw 'Invalid runtime Python filename' }
    $target = [IO.Path]::GetFullPath((Join-Path $root $Name))
    if ([IO.Path]::GetDirectoryName($target) -ne $root) { throw 'Runtime target escaped the fixed app directory' }
    # 提权写入前禁止目录联接或符号链接把已检查的目标引向其他位置。
    $current = $target
    while ($current) {
        if (Test-Path -LiteralPath $current) {
            $item = Get-Item -LiteralPath $current -Force
            if (($item.Attributes -band [IO.FileAttributes]::ReparsePoint) -ne 0) { throw 'Runtime path must not contain reparse points' }
        }
        $parent = [IO.Path]::GetDirectoryName($current)
        if ($parent -eq $current) { break }
        $current = $parent
    }
    if (-not (Test-Path -LiteralPath $root -PathType Container)) { throw 'Installed voice runtime app directory is missing' }
    if ((Test-Path -LiteralPath $target) -and -not (Test-Path -LiteralPath $target -PathType Leaf)) { throw 'Runtime target must be a file' }
    return $target
}

function Build-Plan {
    if (($null -ne $manifest.x64) -ne ($null -ne $manifest.x86)) {
        throw 'Both TSF architectures must be supplied together'
    }
    $items = @()
    foreach ($role in @('server', 'settings', 'appIcon', 'x64', 'x86', 'icon')) {
        $source = $manifest.$role
        if ($null -eq $source) { continue }
        if ($source.sha256 -notmatch '^[a-fA-F0-9]{64}$') { throw "Invalid SHA256 for $role" }
        $sourcePath = (Resolve-Path -LiteralPath $source.path).Path
        if ((File-Hash $sourcePath) -ne $source.sha256) { throw "Source hash mismatch: $role" }
        $currentPath = switch ($role) {
            'server' { Join-Path $installRoot 'qingjian-server.exe' }
            'settings' { Join-Path $installRoot 'qingjian-settings.exe' }
            'appIcon' { Join-Path $installRoot 'vakyu.ico' }
            'x64' { $old64 }
            'x86' { $old32 }
            'icon' { Join-Path ([Environment]::GetFolderPath('CommonApplicationData')) 'Qingjian\qingjian.ico' }
        }
        $targetPath = $currentPath
        if ($role -in @('x64','x86')) {
            $targetPath = Join-Path $installRoot ('qingjian_tsf-bridge1-' + $source.sha256.Substring(0,12).ToLowerInvariant() + '-' + $role + '.dll')
        }
        $items += [pscustomobject]@{
            role=$role;source=$sourcePath;target=$targetPath;previous=$currentPath
            sha256=$source.sha256;changed=((File-Hash $currentPath) -ne $source.sha256);existed=(Test-Path -LiteralPath $currentPath -PathType Leaf)
        }
    }
    $runtimeNames = @{}
    foreach ($source in @($manifest.voiceRuntime)) {
        if ($null -eq $source) { continue }
        $name = [string]$source.name
        $targetPath = Runtime-Target $name
        if ($runtimeNames.ContainsKey($name)) { throw 'Duplicate runtime Python filename' }
        $runtimeNames[$name] = $true
        if ($source.sha256 -notmatch '^[a-fA-F0-9]{64}$') { throw "Invalid SHA256 for runtime: $name" }
        $sourcePath = (Resolve-Path -LiteralPath $source.path).Path
        if ((File-Hash $sourcePath) -ne $source.sha256) { throw "Source hash mismatch: runtime $name" }
        $items += [pscustomobject]@{
            role='runtime';name=$name;source=$sourcePath;target=$targetPath;previous=$targetPath
            sha256=$source.sha256;changed=((File-Hash $targetPath) -ne $source.sha256);existed=(Test-Path -LiteralPath $targetPath -PathType Leaf)
        }
    }
    if (@($items | Where-Object {$_.role -eq 'runtime' -and $_.changed}).Count -gt 0 -and $null -eq $manifest.server) { throw 'Runtime changes require a server component in the same manifest' }
    if ($items.Count -eq 0) { throw 'Manifest has no components' }
    return $items
}

$components = @(Build-Plan)
$changed = @($components | Where-Object { $_.changed })
if ($Plan) {
    [ordered]@{components=$components;brand=$manifest.brand;dllChanged=(@($changed | Where-Object {$_.role -in @('x64','x86')}).Count -gt 0)} | ConvertTo-Json -Depth 5
    exit 0
}

# 仅替换安装文件的子进程提权，外层以原用户权限重新启动后台。
if (-not $Apply) {
    if ($changed.Count -eq 0 -and -not $updateBrand) {
        New-Item -ItemType Directory -Path ([IO.Path]::GetDirectoryName($resultFile)) -Force | Out-Null
        [ordered]@{ok=$true;changed=@();restartServer=$false;dllChanged=$false;unchanged=$true} | ConvertTo-Json | Set-Content -LiteralPath $resultFile -Encoding UTF8
        try { & (Join-Path $PSScriptRoot 'refresh-icons.ps1') } catch { Write-Warning $_.Exception.Message }
        Get-Content -LiteralPath $resultFile
        exit 0
    }
    $caller = New-Object Security.Principal.WindowsPrincipal([Security.Principal.WindowsIdentity]::GetCurrent())
    if ($caller.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)) {
        throw 'Run the updater from a normal PowerShell window; installation elevation is automatic'
    }
    $InvocationId = [Guid]::NewGuid().ToString('N')
    $setup = Start-Process -FilePath 'C:\Windows\System32\WindowsPowerShell\v1.0\powershell.exe' -Verb RunAs -WindowStyle Hidden -Wait -PassThru -ArgumentList @(
        '-NoProfile','-ExecutionPolicy','Bypass','-File',('"'+$PSCommandPath+'"'),
        '-ManifestPath',('"'+$manifestFile+'"'),'-ResultPath',('"'+$resultFile+'"'),'-InvocationId',$InvocationId,'-RuntimeRoot',('"'+$RuntimeRoot+'"'),'-Apply'
    )
    if (Test-Path -LiteralPath $resultFile) {
        $result = Get-Content -LiteralPath $resultFile -Raw | ConvertFrom-Json
        if ($result.invocationId -ne $InvocationId) { throw 'Installer did not produce a result for this invocation' }
        if ($result.restartServer) {
            # 不把安装器临时日志过滤传给后台，后台按用户配置选择日志级别。
            $installerLogFilter = [Environment]::GetEnvironmentVariable('RUST_LOG','Process')
            try {
                [Environment]::SetEnvironmentVariable('RUST_LOG',$null,'Process')
                Start-Process -FilePath (Join-Path $installRoot 'qingjian-server.exe') -WorkingDirectory $installRoot -WindowStyle Hidden
            } finally { [Environment]::SetEnvironmentVariable('RUST_LOG',$installerLogFilter,'Process') }
        }
        try { & (Join-Path $PSScriptRoot 'refresh-icons.ps1') } catch { Write-Warning $_.Exception.Message }
        Get-Content -LiteralPath $resultFile
    } else { throw 'Installer did not write its result file' }
    if ($setup.ExitCode -ne 0) { exit $setup.ExitCode }
    exit 0
}

if ([string]::IsNullOrWhiteSpace($InvocationId)) { throw 'Apply requires an invocation identifier' }
$principal = New-Object Security.Principal.WindowsPrincipal([Security.Principal.WindowsIdentity]::GetCurrent())
if (-not $principal.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)) { throw 'Apply requires an elevated process' }
$backupRoot = Join-Path $env:LOCALAPPDATA ('Qingjian\updates\' + (Get-Date -Format 'yyyyMMdd-HHmmss') + '-' + [Guid]::NewGuid().ToString('N').Substring(0,8))
$guard = $null
$registered = $false
$stoppedServer = $false
$replaced = @()
$brand = $null

function Read-Exact($Stream, [byte[]]$Buffer) {
    $offset = 0
    while ($offset -lt $Buffer.Length) {
        $read = $Stream.ReadAsync($Buffer, $offset, $Buffer.Length - $offset)
        if (-not $read.Wait(1500)) { throw 'Server update acknowledgement timed out' }
        if ($read.Result -eq 0) { throw 'Server closed update acknowledgement' }
        $offset += $read.Result
    }
}

function Write-PipeMessage($Stream, [string]$Json) {
    $body = [Text.Encoding]::UTF8.GetBytes($Json)
    $prefix = [BitConverter]::GetBytes([uint32]$body.Length)
    $Stream.Write($prefix, 0, 4)
    $Stream.Write($body, 0, $body.Length)
    $Stream.Flush()
}

function Read-UpdateAcknowledgement($Stream) {
    $length = New-Object byte[] 4
    Read-Exact $Stream $length
    $count = [BitConverter]::ToUInt32($length, 0)
    if ($count -gt 65536) { throw 'Invalid update acknowledgement length' }
    $response = New-Object byte[] $count
    Read-Exact $Stream $response
    $message = [Text.Encoding]::UTF8.GetString($response) | ConvertFrom-Json
    if ($null -eq $message.Update -or $message.Update.session -ne 0) { throw 'Unexpected update acknowledgement' }
    if ($null -ne $message.Update.voice) {
        $refusal = New-Object InvalidOperationException('Server refused update preparation; voice has not safely stopped')
        $refusal.Data['QingjianUpdateRefused'] = $true
        throw $refusal
    }
}

function Prepare-Server {
    $stream = New-Object IO.Pipes.NamedPipeClientStream('.', 'qingjian', [IO.Pipes.PipeDirection]::InOut, [IO.Pipes.PipeOptions]::Asynchronous)
    try {
        $stream.Connect(1000)
        Write-PipeMessage $stream '"PrepareUpdate"'
        Read-UpdateAcknowledgement $stream
        return 'cancelled_and_flushed'
    } catch {
        if ($_.Exception.Data['QingjianUpdateRefused']) { throw }
    } finally { $stream.Dispose() }
    # 旧后台的 CloseSession 也会落盘；同管道随后 Poll 的回包确认前序操作已完成。
    $legacyStream = New-Object IO.Pipes.NamedPipeClientStream('.', 'qingjian', [IO.Pipes.PipeDirection]::InOut, [IO.Pipes.PipeOptions]::Asynchronous)
    try {
        $legacyStream.Connect(1000)
        Write-PipeMessage $legacyStream '{"CloseSession":{"session":0}}'
        Write-PipeMessage $legacyStream '{"Poll":{"session":0}}'
        Read-UpdateAcknowledgement $legacyStream
        return 'legacy_flushed'
    } catch { return 'unavailable' } finally { $legacyStream.Dispose() }
}

function Register-Tsf([string]$Tool, [string]$Dll) {
    $registration = Start-Process -FilePath $Tool -ArgumentList @('/s',('"'+$Dll+'"')) -WindowStyle Hidden -Wait -PassThru
    if ($registration.ExitCode -ne 0) { throw "TSF registration failed: $($registration.ExitCode)" }
}

function Stop-InstalledProcess([string]$Path) {
    foreach ($running in Get-Process -Name ([IO.Path]::GetFileNameWithoutExtension($Path)) -ErrorAction SilentlyContinue) {
        if ($running.Path -ne $Path) { continue }
        Stop-Process -Id $running.Id -Force
        if (-not $running.WaitForExit(5000)) { throw "Process did not stop: $($running.Id)" }
    }
}

function Loaded-OlderBridges {
    $approved = @((Get-ItemProperty -LiteralPath $registry64).'(default)', (Get-ItemProperty -LiteralPath $registry32).'(default)')
    foreach ($process in Get-Process) {
        try {
            $loaded = @($process.Modules | Where-Object {
                $_.FileName.StartsWith($installRoot + '\', [StringComparison]::OrdinalIgnoreCase) -and
                $_.ModuleName -like 'qingjian_tsf*.dll' -and $approved -notcontains $_.FileName
            })
            if ($loaded.Count -gt 0) {
                [pscustomobject]@{pid=$process.Id;app=$process.ProcessName;components=@($loaded | ForEach-Object {$_.ModuleName})}
            }
        } catch { }
    }
}

try {
    New-Item -ItemType Directory -Path ([IO.Path]::GetDirectoryName($resultFile)) -Force | Out-Null
    $createdGuard = $false
    $guard = [Threading.Mutex]::new($false, 'Global\QingjianInstaller', [ref]$createdGuard)
    if (-not $createdGuard) { throw 'Another Vakyu installation is in progress' }
    # 拿到安装锁后重新读取，避免计划检查与真正替换之间被另一轮安装更改。
    $old64 = (Get-ItemProperty -LiteralPath $registry64).'(default)'
    $old32 = (Get-ItemProperty -LiteralPath $registry32).'(default)'
    $components = @(Build-Plan)
    $changed = @($components | Where-Object { $_.changed })
    if ($changed.Count -eq 0 -and -not $updateBrand) {
        [ordered]@{ok=$true;invocationId=$InvocationId;changed=@();restartServer=$false;dllChanged=$false;unchanged=$true} | ConvertTo-Json | Set-Content -LiteralPath $resultFile -Encoding UTF8
        exit 0
    }
    New-Item -ItemType Directory -Path $backupRoot | Out-Null
    [ordered]@{x64=$old64;x86=$old32;components=$components} | ConvertTo-Json -Depth 5 | Set-Content -LiteralPath (Join-Path $backupRoot 'previous.json') -Encoding UTF8
    $prepared = 'not_required'
    if (@($changed.role) -contains 'server' -or @($changed.role) -contains 'runtime') {
        $prepared = Prepare-Server
        Stop-InstalledProcess (Join-Path $installRoot 'qingjian-server.exe')
        $stoppedServer = $true
    }
    foreach ($component in $changed) {
        if ($component.role -in @('server','settings')) {
            Copy-Item -LiteralPath $component.previous -Destination (Join-Path $backupRoot ([IO.Path]::GetFileName($component.previous)))
            Stop-InstalledProcess $component.previous
            if ($component.role -eq 'server') { $stoppedServer = $true }
            $replaced += $component
            Copy-Item -LiteralPath $component.source -Destination $component.target -Force
        } elseif ($component.role -eq 'runtime') {
            $targetPath = Runtime-Target $component.name
            if ($targetPath -ne $component.target) { throw 'Runtime target changed after planning' }
            $runtimeBackup = Join-Path $backupRoot 'runtime'
            New-Item -ItemType Directory -Path $runtimeBackup -Force | Out-Null
            if ($component.existed) {
                Copy-Item -LiteralPath $component.previous -Destination (Join-Path $runtimeBackup $component.name)
            }
            $replaced += $component
            Copy-Item -LiteralPath $component.source -Destination $targetPath -Force
        } elseif ($component.role -in @('icon','appIcon')) {
            if ($component.role -eq 'icon' -and -not (Test-Path -LiteralPath $component.target -PathType Leaf)) { throw 'Registered icon compatibility target is missing' }
            if ($component.existed) {
                Copy-Item -LiteralPath $component.previous -Destination (Join-Path $backupRoot ([IO.Path]::GetFileName($component.previous)))
            }
            $replaced += $component
            Copy-Item -LiteralPath $component.source -Destination $component.target -Force
        } else {
            if ((Test-Path -LiteralPath $component.target) -and (File-Hash $component.target) -ne $component.sha256) { throw 'TSF filename already exists with different contents' }
            if (-not (Test-Path -LiteralPath $component.target)) { Copy-Item -LiteralPath $component.source -Destination $component.target }
        }
        if ((File-Hash $component.target) -ne $component.sha256) { throw "Installed hash mismatch: $($component.role)" }
    }
    foreach ($component in $changed | Where-Object {$_.role -in @('x64','x86')}) {
        $registered = $true
        $registrationTool = if ($component.role -eq 'x64') {'C:\Windows\System32\regsvr32.exe'} else {'C:\Windows\SysWOW64\regsvr32.exe'}
        Register-Tsf $registrationTool $component.target
        $registrationKey = if ($component.role -eq 'x64') {$registry64} else {$registry32}
        if ((Get-ItemProperty -LiteralPath $registrationKey).'(default)' -ne $component.target) { throw 'TSF registration verification failed' }
    }
    $dllChanged = @($changed | Where-Object {$_.role -in @('x64','x86')}).Count -gt 0
    $oldApps = if ($dllChanged) {@(Loaded-OlderBridges)} else {@()}
    $brand = if ($updateBrand) { & (Join-Path $PSScriptRoot 'update-brand.ps1') -BackupRoot $backupRoot } else { $null }
    [ordered]@{ok=$true;invocationId=$InvocationId;changed=@($changed.role);restartServer=$stoppedServer;dllChanged=$dllChanged;prepared=$prepared;backup=$backupRoot;oldApps=@($oldApps);brand=$brand} | ConvertTo-Json -Depth 6 | Set-Content -LiteralPath $resultFile -Encoding UTF8
} catch {
    $failure = $_.Exception.Message
    $rollback = @()
    if ($null -ne $brand) {
        try { & (Join-Path $PSScriptRoot 'update-brand.ps1') -BackupRoot $backupRoot -Rollback }
        catch { $rollback += $_.Exception.Message }
    }
    $rollbackComponents = @($replaced)
    [array]::Reverse($rollbackComponents)
    foreach ($component in $rollbackComponents) {
        try {
            if ($component.role -eq 'runtime') {
                $targetPath = Runtime-Target $component.name
                if ($targetPath -ne $component.target) { throw 'Runtime rollback target changed' }
                if ($component.existed) {
                    $backupPath = Join-Path (Join-Path $backupRoot 'runtime') $component.name
                    Copy-Item -LiteralPath $backupPath -Destination $targetPath -Force
                    if ((File-Hash $targetPath) -ne (File-Hash $backupPath)) { throw 'Runtime rollback hash mismatch' }
                } elseif (Test-Path -LiteralPath $targetPath -PathType Leaf) {
                    # 仅移除本轮新增的明确单个 .py 文件，不递归删除目录。
                    Remove-Item -LiteralPath $targetPath -Force
                    if (Test-Path -LiteralPath $targetPath) { throw 'New runtime file could not be rolled back' }
                }
            } elseif ($component.role -eq 'appIcon' -and -not $component.existed) {
                if (Test-Path -LiteralPath $component.target -PathType Leaf) {
                    Remove-Item -LiteralPath $component.target -Force
                    if (Test-Path -LiteralPath $component.target) { throw 'New application icon could not be rolled back' }
                }
            } else {
                Copy-Item -LiteralPath (Join-Path $backupRoot ([IO.Path]::GetFileName($component.previous))) -Destination $component.previous -Force
            }
        } catch { $rollback += $_.Exception.Message }
    }
    if ($registered) {
        foreach ($registration in @(@('C:\Windows\System32\regsvr32.exe',$old64),@('C:\Windows\SysWOW64\regsvr32.exe',$old32))) {
            try { Register-Tsf $registration[0] $registration[1] } catch { $rollback += $_.Exception.Message }
        }
        try {
            if ((Get-ItemProperty -LiteralPath $registry64).'(default)' -ne $old64) { $rollback += 'x64 registration was not restored' }
            if ((Get-ItemProperty -LiteralPath $registry32).'(default)' -ne $old32) { $rollback += 'x86 registration was not restored' }
        } catch { $rollback += $_.Exception.Message }
    }
    [ordered]@{ok=$false;invocationId=$InvocationId;error=$failure;rollbackErrors=@($rollback);restartServer=($stoppedServer -and $rollback.Count -eq 0);backup=$backupRoot} | ConvertTo-Json -Depth 4 | Set-Content -LiteralPath $resultFile -Encoding UTF8
    exit 1
} finally {
    if ($null -ne $guard) { $guard.Dispose() }
}
