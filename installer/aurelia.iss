; Aurelia per-user Windows installer (adapted from Inkwell's installer.iss).
; Build with scripts/pipeline.py, which passes the defines below.
; Requires Inno Setup 6.3 or newer.

#define MyAppName "Aurelia"
#define MyAppPublisher "LeTrollologist"
#define MyAppURL "https://github.com/LeTrollologist/Aurelia"
#define MyAppExeName "aurelia.exe"

#ifndef MyAppVersion
  #define MyAppVersion "2.1.0"
#endif
#ifndef MyAppVersionNumeric
  #define MyAppVersionNumeric "2.1.0.0"
#endif
; Staging folder produced by the pipeline (contains aurelia.exe + docs).
#ifndef MySourceDir
  #define MySourceDir "..\dist\Aurelia\staging"
#endif
#ifndef MyOutputDir
  #define MyOutputDir "..\dist"
#endif

[Setup]
; Aurelia has its own AppId (never reuse another product's).
AppId={{8E4C1F6A-2B47-4E9D-9C3A-5D1B7A9E2C64}
AppName={#MyAppName}
AppVersion={#MyAppVersion}
AppVerName={#MyAppName} {#MyAppVersion}
AppPublisher={#MyAppPublisher}
AppPublisherURL={#MyAppURL}
AppSupportURL={#MyAppURL}/issues
AppUpdatesURL={#MyAppURL}/releases
DefaultDirName={localappdata}\Programs\{#MyAppName}
DefaultGroupName={#MyAppName}
DisableProgramGroupPage=yes
LicenseFile=..\LICENSE.md
PrivilegesRequired=lowest
OutputDir={#MyOutputDir}
OutputBaseFilename=Aurelia-Setup-{#MyAppVersion}
SetupIconFile=..\assets\icon.ico
UninstallDisplayIcon={app}\aurelia.ico
UninstallDisplayName={#MyAppName}
Compression=lzma2/ultra64
SolidCompression=yes
WizardStyle=modern
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
MinVersion=10.0
VersionInfoVersion={#MyAppVersionNumeric}
VersionInfoCompany={#MyAppPublisher}
VersionInfoDescription={#MyAppName} installer
VersionInfoProductName={#MyAppName}
VersionInfoProductVersion={#MyAppVersionNumeric}
CloseApplications=yes
RestartApplications=no
UsePreviousAppDir=yes
UsePreviousTasks=yes
ChangesAssociations=yes
SetupLogging=yes
AllowNoIcons=yes

[Tasks]
Name: "associate"; Description: "Open Aurelia projects with Aurelia (.aur, .hcv)"; GroupDescription: "File associations:"; Flags: checkedonce
Name: "desktopicon"; Description: "{cm:CreateDesktopIcon}"; GroupDescription: "{cm:AdditionalIcons}"; Flags: unchecked

[Files]
Source: "{#MySourceDir}\aurelia.exe"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\assets\icon.ico"; DestDir: "{app}"; DestName: "aurelia.ico"; Flags: ignoreversion
Source: "{#MySourceDir}\README.md"; DestDir: "{app}"; Flags: ignoreversion
Source: "{#MySourceDir}\INSTALL.md"; DestDir: "{app}"; Flags: ignoreversion
Source: "{#MySourceDir}\LICENSE.md"; DestDir: "{app}"; Flags: ignoreversion
Source: "{#MySourceDir}\PRIVACY.md"; DestDir: "{app}"; Flags: ignoreversion
Source: "{#MySourceDir}\SECURITY.md"; DestDir: "{app}"; Flags: ignoreversion
Source: "{#MySourceDir}\ROADMAP.md"; DestDir: "{app}"; Flags: ignoreversion

[Icons]
Name: "{group}\Aurelia"; Filename: "{app}\{#MyAppExeName}"; IconFilename: "{app}\aurelia.ico"
Name: "{group}\{cm:UninstallProgram,Aurelia}"; Filename: "{uninstallexe}"
Name: "{autodesktop}\Aurelia"; Filename: "{app}\{#MyAppExeName}"; IconFilename: "{app}\aurelia.ico"; Tasks: desktopicon

[Run]
Filename: "{app}\{#MyAppExeName}"; Description: "{cm:LaunchProgram,Aurelia}"; Flags: nowait postinstall skipifsilent

[Registry]
; Optional .aur / .hcv association. Removed on uninstall (see [Code]).
Root: HKCU; Subkey: "Software\Classes\.aur"; ValueType: string; ValueName: ""; ValueData: "Aurelia.Project"; Tasks: associate
Root: HKCU; Subkey: "Software\Classes\.hcv"; ValueType: string; ValueName: ""; ValueData: "Aurelia.Project"; Tasks: associate
Root: HKCU; Subkey: "Software\Classes\Aurelia.Project"; ValueType: string; ValueName: ""; ValueData: "Aurelia project"; Flags: uninsdeletekey; Tasks: associate
Root: HKCU; Subkey: "Software\Classes\Aurelia.Project\DefaultIcon"; ValueType: string; ValueName: ""; ValueData: "{app}\aurelia.ico"; Tasks: associate
Root: HKCU; Subkey: "Software\Classes\Aurelia.Project\shell\open\command"; ValueType: string; ValueName: ""; ValueData: """{app}\{#MyAppExeName}"" ""%1"""; Tasks: associate
Root: HKCU; Subkey: "Software\Classes\Applications\aurelia.exe"; ValueType: string; ValueName: "FriendlyAppName"; ValueData: "Aurelia"; Tasks: associate
Root: HKCU; Subkey: "Software\Classes\Applications\aurelia.exe\shell\open\command"; ValueType: string; ValueName: ""; ValueData: """{app}\{#MyAppExeName}"" ""%1"""; Flags: uninsdeletekey; Tasks: associate

[Code]
procedure CurUninstallStepChanged(CurUninstallStep: TUninstallStep);
var
  Value: String;
begin
  { Only clear an extension association if it still points at Aurelia. If the user
    picked another editor after installing, leave their choice alone. }
  if CurUninstallStep = usPostUninstall then
  begin
    if RegQueryStringValue(HKCU, 'Software\Classes\.aur', '', Value) and
       (CompareText(Value, 'Aurelia.Project') = 0) then
      RegDeleteValue(HKCU, 'Software\Classes\.aur', '');
    if RegQueryStringValue(HKCU, 'Software\Classes\.hcv', '', Value) and
       (CompareText(Value, 'Aurelia.Project') = 0) then
      RegDeleteValue(HKCU, 'Software\Classes\.hcv', '');
  end;
end;
