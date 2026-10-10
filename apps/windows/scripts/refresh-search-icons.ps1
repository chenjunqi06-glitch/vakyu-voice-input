param()
$ErrorActionPreference = 'Stop'
# Windows 10 搜索索引可能继续引用原 BMP 路径，不能只删文件；按当前 ICO 重建精确的产品图片。
$localRoot = [Environment]::GetFolderPath('LocalApplicationData')
$cacheRoot = [IO.Path]::GetFullPath((Join-Path $localRoot 'Packages\Microsoft.Windows.Search_cw5n1h2txyewy\LocalState\AppIconCache'))
$iconPath = 'C:\Program Files\Qingjian\vakyu.ico'
if (-not (Test-Path -LiteralPath $cacheRoot -PathType Container) -or -not (Test-Path -LiteralPath $iconPath -PathType Leaf)) { return }
$backupRoot = [IO.Path]::GetFullPath((Join-Path $localRoot ('Qingjian\icon-cache-backups\'+[Guid]::NewGuid().ToString('N'))))
$names = @('{6D809377-6AF0-444B-8957-A3773F02200E}_Qingjian_qingjian-settings_exe', '{6D809377-6AF0-444B-8957-A3773F02200E}_Qingjian_unins000_exe')
Add-Type -AssemblyName System.Drawing

function New-SearchBitmap([int]$Size) {
    # .NET Framework 的 Icon.ToBitmap 对 PNG 压缩 ICO 会产生噪点，直接解码 ICO 内的原始 PNG。
    $ico = [IO.File]::ReadAllBytes($iconPath)
    if ($ico.Length -lt 22 -or [BitConverter]::ToUInt16($ico, 2) -ne 1) { throw 'Invalid product ICO' }
    $count = [BitConverter]::ToUInt16($ico, 4)
    $frames = @(for ($index = 0; $index -lt $count; $index++) {
        $entry = 6 + $index * 16
        if ($entry + 16 -gt $ico.Length) { throw 'Invalid ICO directory' }
        $length = [BitConverter]::ToUInt32($ico, $entry + 8)
        $offset = [BitConverter]::ToUInt32($ico, $entry + 12)
        if ($length -lt 8 -or [long]$offset + $length -gt $ico.Length) { continue }
        if ([BitConverter]::ToString($ico, $offset, 8) -eq '89-50-4E-47-0D-0A-1A-0A') {
            [pscustomobject]@{Length=$length;Offset=$offset}
        }
    })
    $frame = $frames | Sort-Object Length -Descending | Select-Object -First 1
    if ($null -eq $frame) { throw 'Product ICO has no supported PNG image' }
    $png = [byte[]]::new($frame.Length)
    [Array]::Copy($ico, $frame.Offset, $png, 0, $frame.Length)
    $stream = [IO.MemoryStream]::new($png, $false)
    $source = [Drawing.Bitmap]::FromStream($stream)
    $bitmap = [Drawing.Bitmap]::new($Size, $Size, [Drawing.Imaging.PixelFormat]::Format32bppArgb)
    $graphics = [Drawing.Graphics]::FromImage($bitmap)
    try {
        $graphics.CompositingMode = [Drawing.Drawing2D.CompositingMode]::SourceCopy
        $graphics.InterpolationMode = [Drawing.Drawing2D.InterpolationMode]::HighQualityBicubic
        $graphics.PixelOffsetMode = [Drawing.Drawing2D.PixelOffsetMode]::HighQuality
        $graphics.DrawImage($source, [Drawing.Rectangle]::new(0, 0, $Size, $Size))
        $graphics.Dispose()
        $graphics = $null
        $pixels = $bitmap.LockBits([Drawing.Rectangle]::new(0, 0, $Size, $Size), [Drawing.Imaging.ImageLockMode]::ReadOnly, [Drawing.Imaging.PixelFormat]::Format32bppArgb)
        try {
            $bytes = [byte[]]::new(150 + $Size * $Size * 4)
            $bytes[0] = 0x42; $bytes[1] = 0x4d
            # 保持 Search 原有 BITMAPV5、BGRA 位掩码及正高度的行顺序，保留透明边缘。
            [BitConverter]::GetBytes([uint32]$bytes.Length).CopyTo($bytes, 2)
            [BitConverter]::GetBytes([uint32]150).CopyTo($bytes, 10)
            [BitConverter]::GetBytes([uint32]124).CopyTo($bytes, 14)
            [BitConverter]::GetBytes([int]$Size).CopyTo($bytes, 18)
            [BitConverter]::GetBytes([int]$Size).CopyTo($bytes, 22)
            [BitConverter]::GetBytes([uint16]1).CopyTo($bytes, 26)
            [BitConverter]::GetBytes([uint16]32).CopyTo($bytes, 28)
            [BitConverter]::GetBytes([uint32]3).CopyTo($bytes, 30)
            [BitConverter]::GetBytes([uint32]0x00ff0000).CopyTo($bytes, 54)
            [BitConverter]::GetBytes([uint32]0x0000ff00).CopyTo($bytes, 58)
            [BitConverter]::GetBytes([uint32]0x000000ff).CopyTo($bytes, 62)
            [BitConverter]::GetBytes([uint32]4278190080).CopyTo($bytes, 66)
            [BitConverter]::GetBytes([uint32]0x57696e20).CopyTo($bytes, 70)
            [Array]::Copy($bytes, 54, $bytes, 138, 12)
            for ($row = 0; $row -lt $Size; $row++) {
                $pointer = [IntPtr]::Add($pixels.Scan0, $row * $pixels.Stride)
                [Runtime.InteropServices.Marshal]::Copy($pointer, $bytes, 150 + ($Size - 1 - $row) * $Size * 4, $Size * 4)
            }
            return ,$bytes
        } finally { $bitmap.UnlockBits($pixels) }
    } finally {
        if ($null -ne $graphics) { $graphics.Dispose() }
        $bitmap.Dispose(); $source.Dispose(); $stream.Dispose()
    }
}

foreach ($scale in Get-ChildItem -LiteralPath $cacheRoot -Directory) {
    if ($scale.Name -notmatch '^\d+$' -or ($scale.Attributes -band [IO.FileAttributes]::ReparsePoint)) { continue }
    $size = [int][Math]::Round(96 * [int]$scale.Name / 100.0)
    if ($size -lt 32 -or $size -gt 384) { continue }
    $bytes = New-SearchBitmap $size
    foreach ($name in $names) {
        $target = [IO.Path]::GetFullPath((Join-Path $scale.FullName $name))
        if (-not $target.StartsWith($cacheRoot+'\', [StringComparison]::OrdinalIgnoreCase)) { throw 'Unexpected search cache path' }
        if (Test-Path -LiteralPath $target) {
            $item = Get-Item -LiteralPath $target
            if ($item.PSIsContainer -or ($item.Attributes -band [IO.FileAttributes]::ReparsePoint)) { continue }
            $existing = [IO.File]::ReadAllBytes($target)
            if ([Convert]::ToBase64String($existing) -eq [Convert]::ToBase64String($bytes)) { continue }
            New-Item -ItemType Directory -Path $backupRoot -Force | Out-Null
            Copy-Item -LiteralPath $target -Destination (Join-Path $backupRoot ($scale.Name+'-'+$name+'.bmp'))
        }
        $temporary = Join-Path $scale.FullName ('vakyu-'+[Guid]::NewGuid().ToString('N')+'.tmp')
        try {
            [IO.File]::WriteAllBytes($temporary, $bytes)
            # 先核验临时图片再原子替换，生成失败时不破坏现有缓存。
            $check = [Drawing.Bitmap]::FromFile($temporary)
            try { if ($check.Width -ne $size -or $check.Height -ne $size) { throw 'Unexpected search icon dimensions' } }
            finally { $check.Dispose() }
            if (Test-Path -LiteralPath $target -PathType Leaf) {
                [IO.File]::Replace($temporary, $target, [System.Management.Automation.Language.NullString]::Value)
            } else {
                [IO.File]::Move($temporary, $target)
            }
        } finally {
            if (Test-Path -LiteralPath $temporary -PathType Leaf) { Remove-Item -LiteralPath $temporary }
        }
    }
}
