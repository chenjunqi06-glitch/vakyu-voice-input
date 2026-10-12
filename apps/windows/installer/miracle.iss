; Vakyu Windows x64 离线安装包。全部内容来自 build.ps1 白名单暂存目录。
; TSF DLL 按版本并排安装；只停止本产品进程及其语音子进程，不关闭用户应用。
; 沿用安装目录与 AppId 以兼容升级，公开包不用本机证书或 uiAccess。

#ifndef AppVersion
  #error 请通过 build.ps1 传入 AppVersion
#endif
#ifndef AppVersionNumeric
  #error 请通过 build.ps1 传入 AppVersionNumeric
#endif
#ifndef StageRoot
  #error 请通过 build.ps1 传入 StageRoot
#endif
#ifndef OutputDirectory
  #error 请通过 build.ps1 传入 OutputDirectory
#endif
#define TsfDll "qingjian_tsf-" + AppVersion + ".dll"
#define TsfDll32 "qingjian_tsf-" + AppVersion + "-x86.dll"
; 从当前随包安装器的固定版本资源取值，不能把手写版本与更新后的运行库混用。
#define VCRuntimeX64MS 0
#define VCRuntimeX64LS 0
#define VCRuntimeX86MS 0
#define VCRuntimeX86LS 0
#define VCRuntimeX64Version GetVersionNumbers(StageRoot + "\prerequisites\vc_redist.x64.exe", VCRuntimeX64MS, VCRuntimeX64LS)
#define VCRuntimeX86Version GetVersionNumbers(StageRoot + "\prerequisites\vc_redist.x86.exe", VCRuntimeX86MS, VCRuntimeX86LS)
#if VCRuntimeX64Version == "" || VCRuntimeX86Version == ""
  #error 无法读取随包 VC++ 运行库版本
#endif

[Setup]
AppId={{A7E3C1F2-5B94-4D6A-9C0E-2F8B1D3A6E70}
AppName=Vakyu
AppVersion={#AppVersion}
AppPublisher=Vakyu
VersionInfoVersion={#AppVersionNumeric}
DefaultDirName={autopf}\Qingjian
DefaultGroupName=Vakyu
DisableProgramGroupPage=yes
ArchitecturesAllowed=x64os
ArchitecturesInstallIn64BitMode=x64os
; 发布支持 Windows 10 22H2 / Windows 11 的原生 x64 系统。
MinVersion=10.0.19045
PrivilegesRequired=admin
CloseApplications=no
RestartApplications=no
OutputDir={#OutputDirectory}
OutputBaseFilename=vakyu-{#AppVersion}-windows-x86_64-setup
SetupIconFile={#StageRoot}\payload\vakyu.ico
UninstallDisplayIcon={app}\vakyu.ico
Compression=lzma2/fast
LZMADictionarySize=32768
LZMANumBlockThreads=1
SolidCompression=yes
WizardStyle=modern
SetupLogging=yes
LicenseFile={#StageRoot}\payload\licenses\INSTALL-LICENSE.txt

[Languages]
Name: "chs"; MessagesFile: "compiler:Languages\ChineseSimplified.isl"
Name: "en"; MessagesFile: "compiler:Default.isl"

[Messages]
chs.FinishedLabel=Vakyu 安装完成。普通语音：在输入框按住左 Alt 说话，松开后填入文字。%n%n口语跟读：在设置打开“启用跟读练习”，配置自己的翻译 API 密钥，再按住右 Ctrl＋右 Alt。输入法可通过 Win＋空格切换。%n%n升级后，已加载旧输入法组件的应用可能需要重新打开；全局语音后台会立即启动。
en.FinishedLabel=Vakyu is installed. Hold Left Alt in a text field to dictate, then release to insert text.%n%nFor speaking practice, enable Reading practice and enter your own translation API key in Settings, then hold Right Ctrl + Right Alt. Use Win + Space to select the keyboard.%n%nAfter an upgrade, applications that loaded an older keyboard component may need to reopen. Global voice input starts immediately.

[CustomMessages]
chs.OpenSettings=打开 Vakyu 设置
en.OpenSettings=Open Vakyu Settings

[Files]
; 先缓存 VC++ 依赖；PrepareToInstall 检查安装返回码，失败时不动现有程序。
Source: "{#StageRoot}\prerequisites\vc_redist.x64.exe"; Flags: dontcopy
Source: "{#StageRoot}\prerequisites\vc_redist.x86.exe"; Flags: dontcopy
; regserver 由 Inno 处理注册失败与卸载反注册，不忽略 regsvr32 返回码。
Source: "{#StageRoot}\payload\{#TsfDll}"; DestDir: "{app}"; Flags: ignoreversion uninsrestartdelete regserver 64bit
Source: "{#StageRoot}\payload\{#TsfDll32}"; DestDir: "{app}"; Flags: ignoreversion uninsrestartdelete regserver 32bit
Source: "{#StageRoot}\payload\*"; DestDir: "{app}"; Excludes: "qingjian_tsf*.dll,qingjian-server.exe"; Flags: ignoreversion recursesubdirs createallsubdirs uninsrestartdelete
; Server 最后落地，防止旧 DLL 提前启动新进程占用正在替换的资源。
Source: "{#StageRoot}\payload\qingjian-server.exe"; DestDir: "{app}"; Flags: ignoreversion uninsrestartdelete

[Icons]
Name: "{group}\Vakyu 设置"; Filename: "{app}\qingjian-settings.exe"; IconFilename: "{app}\vakyu.ico"
Name: "{group}\卸载 Vakyu"; Filename: "{uninstallexe}"
; 每个登录用户的 Explorer 启动自己的后台，不把服务运行成管理员或系统账号。
Name: "{commonstartup}\Vakyu Server"; Filename: "{app}\qingjian-server.exe"; WorkingDir: "{app}"; IconFilename: "{app}\vakyu.ico"

[Run]
; 机器级 COM 注册与用户键盘列表是两件事；只给原登录用户启用此条目，不切换默认键盘。
Filename: "{app}\qingjian-server.exe"; Parameters: "--enable-user-profile"; WorkingDir: "{app}"; Flags: runasoriginaluser runhidden waituntilterminated skipifsilent; StatusMsg: "为当前用户启用 Vakyu 输入法…"
Filename: "{sys}\ie4uinit.exe"; Parameters: "-show"; Flags: runhidden runasoriginaluser waituntilterminated skipifdoesntexist
Filename: "{app}\qingjian-server.exe"; WorkingDir: "{app}"; Flags: runasoriginaluser runhidden nowait skipifsilent; Check: ShouldStartServer
Filename: "{app}\qingjian-settings.exe"; WorkingDir: "{app}"; Description: "{cm:OpenSettings}"; Flags: postinstall unchecked runasoriginaluser nowait skipifsilent

[UninstallRun]
Filename: "{sys}\schtasks.exe"; Parameters: "/delete /tn ""Qingjian Server"" /f"; Flags: runhidden; RunOnceId: "DelLogonTask"
Filename: "{sys}\taskkill.exe"; Parameters: "/im qingjian-server.exe /t /f"; Flags: runhidden; RunOnceId: "KillServer"
Filename: "{sys}\taskkill.exe"; Parameters: "/im qingjian-server.old-*.exe /t /f"; Flags: runhidden; RunOnceId: "KillRetiredServer"
Filename: "{sys}\taskkill.exe"; Parameters: "/im qingjian-settings.exe /f"; Flags: runhidden; RunOnceId: "KillSettings"

[InstallDelete]
Type: files; Name: "{userstartup}\Qingjian Server.lnk"
Type: files; Name: "{commonstartup}\青简 Server.lnk"
Type: files; Name: "{commonstartup}\Qingjian Server.lnk"
Type: files; Name: "{userstartup}\Miracle Server.lnk"
Type: files; Name: "{commonstartup}\Miracle Server.lnk"
Type: files; Name: "{commonprograms}\青简\青简设置.lnk"
Type: files; Name: "{commonprograms}\青简\卸载青简.lnk"
Type: files; Name: "{commonprograms}\Miracle\Miracle 设置.lnk"
Type: files; Name: "{commonprograms}\Miracle\卸载 Miracle.lnk"
Type: files; Name: "{app}\data\model\model.safetensors"
Type: files; Name: "{app}\data\model\config.json"
Type: files; Name: "{app}\data\model\vocab.json"
Type: filesandordirs; Name: "{app}\codes"
; 旧安装包混入的 macOS 附属文件（._*），不是词库，只会让每次启动报错。
Type: files; Name: "{app}\data\generated\dicts\._*"
Type: files; Name: "{app}\data\generated\codes\._*"

[UninstallDelete]
Type: files; Name: "{app}\qingjian_tsf-*.dll"
Type: files; Name: "{app}\qingjian-server.old-*.exe"

[Code]
var
  RuntimeRestartRequired: Boolean;
  InstallCommitted: Boolean;
  RetiredServer, RetiredDll, RetiredDll32: String;

function CreateMutex(Attributes: Longint; InitialOwner: BOOL; Name: String): THandle;
  external 'CreateMutexW@kernel32.dll stdcall';

procedure HoldInstallerMutex;
begin
  if CreateMutex(0, False, 'Global\QingjianInstaller') = 0 then
    Log('建立安装互斥体失败');
end;

function InitializeSetup: Boolean;
begin
  HoldInstallerMutex;
  Result := True;
end;

function InitializeUninstall: Boolean;
begin
  HoldInstallerMutex;
  Result := True;
end;

function RegistryHasVCRuntime(const Root: HKEY; const Architecture: String; RequiredMS, RequiredLS: Cardinal): Boolean;
var
  Key: String;
  Installed, Major, Minor, Build, Revision, VersionMS, VersionLS: Cardinal;
begin
  Result := False;
  Key := 'SOFTWARE\Microsoft\VisualStudio\14.0\VC\Runtimes\' + Architecture;
  if not RegQueryDWordValue(Root, Key, 'Installed', Installed) then Exit;
  if Installed <> 1 then Exit;
  if not RegQueryDWordValue(Root, Key, 'Major', Major) then Exit;
  if not RegQueryDWordValue(Root, Key, 'Minor', Minor) then Exit;
  if not RegQueryDWordValue(Root, Key, 'Bld', Build) then Exit;
  if not RegQueryDWordValue(Root, Key, 'Rbld', Revision) then Exit;
  if (Major > 65535) or (Minor > 65535) or (Build > 65535) or (Revision > 65535) then Exit;
  VersionMS := (Major shl 16) or Minor;
  VersionLS := (Build shl 16) or Revision;
  Result := (VersionMS > RequiredMS) or ((VersionMS = RequiredMS) and (VersionLS >= RequiredLS));
  if Result then
    Log('VC++ ' + Architecture + ' 已安装版本满足要求：' + IntToStr(Major) + '.' + IntToStr(Minor) + '.' + IntToStr(Build) + '.' + IntToStr(Revision));
end;

function HasCompatibleVCRuntime(const Architecture: String): Boolean;
begin
  { 微软不同版本曾使用不同注册表视图，只读取对应架构的已安装标志及完整版本号。 }
  if Architecture = 'x64' then
    Result := RegistryHasVCRuntime(HKLM64, Architecture, {#VCRuntimeX64MS}, {#VCRuntimeX64LS}) or
      RegistryHasVCRuntime(HKLM32, Architecture, {#VCRuntimeX64MS}, {#VCRuntimeX64LS})
  else
    Result := RegistryHasVCRuntime(HKLM32, Architecture, {#VCRuntimeX86MS}, {#VCRuntimeX86LS}) or
      RegistryHasVCRuntime(HKLM64, Architecture, {#VCRuntimeX86MS}, {#VCRuntimeX86LS});
end;

function InstallVCRuntime(const Architecture: String): String;
var
  Name: String;
  ExitCode: Integer;
begin
  Result := '';
  if HasCompatibleVCRuntime(Architecture) then
  begin
    Log('跳过已满足要求的 VC++ ' + Architecture + ' 运行库');
    Exit;
  end;
  Name := 'vc_redist.' + Architecture + '.exe';
  ExtractTemporaryFile(Name);
  if not Exec(ExpandConstant('{tmp}\') + Name, '/install /quiet /norestart', '',
    SW_HIDE, ewWaitUntilTerminated, ExitCode) then
    Result := '无法启动 Microsoft Visual C++ 运行库安装程序（' + Architecture + '）。错误：' + IntToStr(ExitCode)
  else if (ExitCode = 3010) or (ExitCode = 1641) then
    RuntimeRestartRequired := True
  { 0x80070666 是 ERROR_PRODUCT_VERSION(1638) 的 HRESULT；不能笼统忽略所有失败。 }
  else if ((ExitCode = 1638) or (ExitCode = -2147023258)) and HasCompatibleVCRuntime(Architecture) then
    Log('运行库返回已安装版本，注册表复核通过：' + Architecture)
  else if ExitCode <> 0 then
    Result := 'Microsoft Visual C++ 运行库安装失败（' + Architecture + '），错误码：' + IntToStr(ExitCode) + '。请完成 Windows 更新或重启后重新安装。';
  Log('VC++ ' + Architecture + ' result: ' + IntToStr(ExitCode));
end;

function NeedRestart: Boolean;
begin
  Result := RuntimeRestartRequired;
end;

procedure KillProcess(const Image: String; const IncludeChildren: Boolean);
var
  ResultCode: Integer;
  Parameters: String;
begin
  Parameters := '/im ' + Image + ' /f';
  if IncludeChildren then Parameters := Parameters + ' /t';
  Exec(ExpandConstant('{sys}\taskkill.exe'), Parameters, '', SW_HIDE, ewWaitUntilTerminated, ResultCode);
end;

function RetireFile(const Name, Prefix, Extension: String): String;
var
  Path: String;
begin
  Result := '';
  Path := ExpandConstant('{app}\') + Name;
  if FileExists(Path) then
  begin
    Result := ExpandConstant('{app}\') + Prefix + IntToStr(Random(1000000)) + Extension;
    if not RenameFile(Path, Result) then
    begin
      Log('改名旧文件失败: ' + Path);
      Result := '';
    end;
  end;
end;

function PrepareToInstall(var NeedsRestart: Boolean): String;
begin
  Result := InstallVCRuntime('x64');
  if Result <> '' then Exit;
  Result := InstallVCRuntime('x86');
  if Result <> '' then Exit;
  RetiredServer := RetireFile('qingjian-server.exe', 'qingjian-server.old-', '.exe');
  KillProcess('qingjian-server.exe', True);
  KillProcess('qingjian-server.old-*.exe', True);
  KillProcess('qingjian-settings.exe', False);
  RetiredDll := RetireFile('{#TsfDll}', 'qingjian_tsf-{#AppVersion}.old-', '.dll');
  RetiredDll32 := RetireFile('{#TsfDll32}', 'qingjian_tsf-{#AppVersion}-x86.old-', '.dll');
  Result := '';
end;

procedure RestoreRetired(const Retired, Name: String);
var
  Path: String;
begin
  Path := ExpandConstant('{app}\') + Name;
  if (Retired <> '') and FileExists(Retired) and not FileExists(Path) then
    if not RenameFile(Retired, Path) then Log('恢复旧文件失败: ' + Path);
end;

procedure DeinitializeSetup;
begin
  if not InstallCommitted then
  begin
    RestoreRetired(RetiredServer, 'qingjian-server.exe');
    RestoreRetired(RetiredDll, '{#TsfDll}');
    RestoreRetired(RetiredDll32, '{#TsfDll32}');
  end;
end;

procedure DeleteStaleFiles(const Pattern: String; const KeepCurrentDlls: Boolean);
var
  Dir, Path: String;
  Found: TFindRec;
begin
  Dir := ExpandConstant('{app}');
  if FindFirst(Dir + '\' + Pattern, Found) then
  begin
    try
      repeat
        if not KeepCurrentDlls or ((CompareText(Found.Name, '{#TsfDll}') <> 0) and (CompareText(Found.Name, '{#TsfDll32}') <> 0)) then
        begin
          Path := Dir + '\' + Found.Name;
          if not DeleteFile(Path) then RestartReplace(Path, '');
        end;
      until not FindNext(Found);
    finally
      FindClose(Found);
    end;
  end;
end;

procedure CurStepChanged(CurStep: TSetupStep);
var
  ExitCode: Integer;
begin
  if CurStep = ssPostInstall then
  begin
    if not Exec(ExpandConstant('{sys}\icacls.exe'), '"' + ExpandConstant('{app}') + '" /grant *S-1-15-2-1:(OI)(CI)RX /T /C /Q', '', SW_HIDE, ewWaitUntilTerminated, ExitCode) then
      RaiseException('无法配置输入法目录的应用访问权限。')
    else if ExitCode <> 0 then
      RaiseException('配置输入法目录权限失败，错误码：' + IntToStr(ExitCode));
    Exec(ExpandConstant('{sys}\schtasks.exe'), '/delete /tn "Qingjian Server" /f', '', SW_HIDE, ewWaitUntilTerminated, ExitCode);
    InstallCommitted := True;
    DeleteStaleFiles('qingjian_tsf*.dll', True);
    DeleteStaleFiles('qingjian-server.old-*.exe', False);
  end;
end;

procedure CurUninstallStepChanged(CurUninstallStep: TUninstallStep);
begin
  if CurUninstallStep = usUninstall then
    RetiredServer := RetireFile('qingjian-server.exe', 'qingjian-server.old-', '.exe');
end;

function ShouldStartServer: Boolean;
var
  Index: Integer;
begin
  Result := True;
  for Index := 1 to ParamCount do
    if CompareText(ParamStr(Index), '/NOAUTOSTART') = 0 then Result := False;
end;
