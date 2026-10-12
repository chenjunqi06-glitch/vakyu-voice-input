param([Parameter(Mandatory=$true)][string]$BackupRoot, [switch]$Rollback)
$ErrorActionPreference = 'Stop'
$installRoot = 'C:\Program Files\Qingjian'
$programsRoot = 'C:\ProgramData\Microsoft\Windows\Start Menu\Programs'
$startupRoot = Join-Path $programsRoot 'Startup'
$brandRoot = Join-Path $programsRoot 'Vakyu'
$registryPath = 'HKLM:\Software\Microsoft\Windows\CurrentVersion\Uninstall\{A7E3C1F2-5B94-4D6A-9C0E-2F8B1D3A6E70}_is1'
$iconPath = Join-Path $installRoot 'vakyu.ico'
$backup = Join-Path $BackupRoot 'brand'

function Restore-Brand {
    $failures = @()
    $previousRegistry = Get-Content -LiteralPath (Join-Path $backup 'uninstall-before.json') -Raw | ConvertFrom-Json
    $previousLinks = @(Get-Content -LiteralPath (Join-Path $backup 'links-before.json') -Raw | ConvertFrom-Json)
    $createdLinks = @(Get-Content -LiteralPath (Join-Path $backup 'links-written.json') -Raw | ConvertFrom-Json)
    foreach ($path in $createdLinks) {
        try {
            if (-not ([IO.Path]::GetFullPath($path)).StartsWith($programsRoot+'\',[StringComparison]::OrdinalIgnoreCase) -or [IO.Path]::GetExtension($path) -ne '.lnk') { throw 'Invalid shortcut rollback target' }
            if (Test-Path -LiteralPath $path -PathType Leaf) { Remove-Item -LiteralPath $path }
        } catch { $failures += $_.Exception.Message }
    }
    foreach ($saved in $previousLinks) {
        try {
            if (-not ([IO.Path]::GetFullPath($saved.path)).StartsWith($programsRoot+'\',[StringComparison]::OrdinalIgnoreCase) -or [IO.Path]::GetExtension($saved.path) -ne '.lnk') { throw 'Invalid shortcut restore target' }
            Copy-Item -LiteralPath $saved.backup -Destination $saved.path -Force
        } catch { $failures += $_.Exception.Message }
    }
    foreach ($key in @('DisplayName','DisplayIcon','Publisher')) {
        try { Set-ItemProperty -LiteralPath $registryPath -Name $key -Value $previousRegistry.$key }
        catch { $failures += $_.Exception.Message }
    }
    if ($failures.Count -gt 0) { throw ('Brand rollback failed: '+($failures -join '; ')) }
    try { & (Join-Path $PSScriptRoot 'refresh-icons.ps1') -NotifyOnly } catch { Write-Warning $_.Exception.Message }
}

if ($Rollback) { Restore-Brand; return }
foreach ($file in @('qingjian-settings.exe','qingjian-server.exe','unins000.exe','vakyu.ico')) {
    if (-not (Test-Path -LiteralPath (Join-Path $installRoot $file) -PathType Leaf)) { throw "Missing installed product file: $file" }
}
New-Item -ItemType Directory -Path $backup -Force | Out-Null
$registry = Get-ItemProperty -LiteralPath $registryPath
if ([IO.Path]::GetFullPath($registry.InstallLocation).TrimEnd('\') -ne $installRoot) { throw 'Uninstall entry belongs to a different installation' }
$registryBefore = [ordered]@{}
foreach ($key in @('DisplayName','DisplayIcon','Publisher')) { $registryBefore[$key] = $registry.$key }
$registryBefore | ConvertTo-Json | Set-Content -LiteralPath (Join-Path $backup 'uninstall-before.json') -Encoding UTF8
$shell = New-Object -ComObject WScript.Shell
$links = @(
    @{path=(Join-Path $brandRoot 'Vakyu 设置.lnk'); target=(Join-Path $installRoot 'qingjian-settings.exe'); description='Vakyu 设置'},
    @{path=(Join-Path $brandRoot '卸载 Vakyu.lnk'); target=(Join-Path $installRoot 'unins000.exe'); description='卸载 Vakyu'},
    @{path=(Join-Path $startupRoot 'Vakyu Server.lnk'); target=(Join-Path $installRoot 'qingjian-server.exe'); description='Vakyu 输入法'}
)
$oldLinks = @(
    @{path=(Join-Path $programsRoot 'Miracle\Miracle 设置.lnk'); target=(Join-Path $installRoot 'qingjian-settings.exe')},
    @{path=(Join-Path $programsRoot 'Miracle\卸载 Miracle.lnk'); target=(Join-Path $installRoot 'unins000.exe')},
    @{path=(Join-Path $programsRoot '青简\青简设置.lnk'); target=(Join-Path $installRoot 'qingjian-settings.exe')},
    @{path=(Join-Path $programsRoot '青简\卸载青简.lnk'); target=(Join-Path $installRoot 'unins000.exe')},
    @{path=(Join-Path $startupRoot 'Miracle Server.lnk'); target=(Join-Path $installRoot 'qingjian-server.exe')},
    @{path=(Join-Path $startupRoot '青简 Server.lnk'); target=(Join-Path $installRoot 'qingjian-server.exe')}
)
$savedLinks = @()
# 先核对全部快捷方式归属并备份，避免影响相似名称的其他程序。
foreach ($item in @($links) + @($oldLinks)) {
    if (-not (Test-Path -LiteralPath $item.path)) { continue }
    if (($shell.CreateShortcut($item.path)).TargetPath -ne $item.target) { throw "Shortcut belongs to a different application: $($item.path)" }
    $saved = Join-Path $backup ('link-' + $savedLinks.Count + '.lnk')
    Copy-Item -LiteralPath $item.path -Destination $saved
    $savedLinks += @{path=$item.path; backup=$saved}
}
$savedLinks | ConvertTo-Json -Depth 3 | Set-Content -LiteralPath (Join-Path $backup 'links-before.json') -Encoding UTF8
$links.path | ConvertTo-Json | Set-Content -LiteralPath (Join-Path $backup 'links-written.json') -Encoding UTF8
$written = @()
try {
    New-Item -ItemType Directory -Path $brandRoot -Force | Out-Null
    foreach ($item in $links) {
        $written += $item.path
        $link = $shell.CreateShortcut($item.path)
        $link.TargetPath = $item.target
        $link.WorkingDirectory = $installRoot
        $link.IconLocation = $iconPath + ',0'
        $link.Description = $item.description
        $link.Save()
        $verify = $shell.CreateShortcut($item.path)
        if ($verify.TargetPath -ne $item.target -or $verify.IconLocation -ne ($iconPath+',0')) { throw 'Shortcut verification failed' }
    }
    Set-ItemProperty -LiteralPath $registryPath -Name DisplayName -Value 'Vakyu'
    Set-ItemProperty -LiteralPath $registryPath -Name Publisher -Value 'Vakyu'
    Set-ItemProperty -LiteralPath $registryPath -Name DisplayIcon -Value $iconPath
    foreach ($item in $oldLinks) {
        if (Test-Path -LiteralPath $item.path -PathType Leaf) { Remove-Item -LiteralPath $item.path }
    }
    $verify = Get-ItemProperty -LiteralPath $registryPath
    if ($verify.DisplayName -ne 'Vakyu' -or $verify.DisplayIcon -ne $iconPath) { throw 'Uninstall brand verification failed' }
    try { & (Join-Path $PSScriptRoot 'refresh-icons.ps1') -NotifyOnly } catch { Write-Warning $_.Exception.Message }
    [pscustomobject]@{ok=$true; displayName=$verify.DisplayName; links=@($links.path); backup=$backup}
} catch {
    $originalError = $_
    try { Restore-Brand } catch { throw ($originalError.Exception.Message+'; '+$_.Exception.Message) }
    throw $originalError
}
