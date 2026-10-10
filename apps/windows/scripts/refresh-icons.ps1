param([switch]$NotifyOnly)
$ErrorActionPreference = 'Stop'
# 文件更新后通知 Shell；不重启 Explorer，仅修复本产品的搜索图标缓存。
if (-not ('Vakyu.ShellIconCache' -as [type])) {
    Add-Type -TypeDefinition @'
using System;
using System.Runtime.InteropServices;
namespace Vakyu {
    public static class ShellIconCache {
        [DllImport("shell32.dll", CharSet = CharSet.Unicode, EntryPoint = "SHChangeNotify")]
        public static extern void PathChanged(uint events, uint flags, string path, IntPtr unused);
        [DllImport("shell32.dll", EntryPoint = "SHChangeNotify")]
        public static extern void AssociationsChanged(uint events, uint flags, IntPtr first, IntPtr second);
    }
}
'@
}
$installRoot = 'C:\Program Files\Qingjian'
foreach ($name in @('qingjian-settings.exe', 'qingjian-server.exe', 'vakyu.ico')) {
    $path = Join-Path $installRoot $name
    if (Test-Path -LiteralPath $path -PathType Leaf) {
        [Vakyu.ShellIconCache]::PathChanged(0x2000, 0x2005, $path, [IntPtr]::Zero)
    }
}
foreach ($root in @([Environment]::GetFolderPath('CommonPrograms'), [Environment]::GetFolderPath('Programs'))) {
    $directory = Join-Path $root 'Vakyu'
    if (-not (Test-Path -LiteralPath $directory -PathType Container)) { continue }
    foreach ($link in Get-ChildItem -LiteralPath $directory -Filter '*.lnk' -File) {
        [Vakyu.ShellIconCache]::PathChanged(0x2000, 0x2005, $link.FullName, [IntPtr]::Zero)
    }
    [Vakyu.ShellIconCache]::PathChanged(0x1000, 0x2005, $directory, [IntPtr]::Zero)
}
[Vakyu.ShellIconCache]::AssociationsChanged(0x08000000, 0x2000, [IntPtr]::Zero, [IntPtr]::Zero)
if (-not $NotifyOnly) {
    & (Join-Path $PSScriptRoot 'refresh-search-icons.ps1')
    # 应由未提权的外层更新器调用，让当前登录用户的搜索/图标缓存刷新。
    $refresh = Join-Path $env:WINDIR 'System32\ie4uinit.exe'
    if (Test-Path -LiteralPath $refresh -PathType Leaf) {
        Start-Process -FilePath $refresh -ArgumentList '-show' -WindowStyle Hidden -Wait
    }
}
