; Postium Mail Windows 安装包（Inno Setup）
; 参数（ISCC /D... /F...）：AppVer / ArchAllowed / SourceExe /F 输出名
#ifndef AppVer
#define AppVer "0.0.0"
#endif
#ifndef ArchAllowed
#define ArchAllowed "x64compatible"
#endif
#ifndef SourceExe
#define SourceExe "postium-mail.exe"
#endif

[Setup]
AppId={{7B3C9E2A-5D64-4F1B-9A87-2C5E8D4B9A10}
AppName=Postium Mail
AppVersion={#AppVer}
AppPublisher=spartajet
DefaultDirName={autopf}\Postium Mail
DefaultGroupName=Postium Mail
UninstallDisplayIcon={app}\postium-mail.exe
ArchitecturesAllowed={#ArchAllowed}
ArchitecturesInstallIn64BitMode=x64compatible
OutputDir=.
Compression=lzma2/max
SolidCompression=yes
WizardStyle=modern
SetupIconFile=..\..\..\crates\postium-ui\icons\icon.ico
PrivilegesRequired=lowest

[Files]
Source: "{#SourceExe}"; DestDir: "{app}"; DestName: "postium-mail.exe"; Flags: ignoreversion

[Icons]
Name: "{group}\Postium Mail"; Filename: "{app}\postium-mail.exe"
Name: "{autodesktop}\Postium Mail"; Filename: "{app}\postium-mail.exe"

[Run]
Filename: "{app}\postium-mail.exe"; Description: "启动 Postium Mail"; Flags: nowait postinstall skipifsilent
