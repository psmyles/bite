; Inno Setup script for Bite.
;
; Do not run this directly - invoke packaging\build-windows-installer.ps1, which builds the
; release binaries, reads product.json (the canonical source of product identity) and passes the
; values below as /D defines. Defaults are provided so the script can also be opened standalone
; in the Inno Setup IDE for editing.
;
; Requires Inno Setup 6 (ISCC.exe). https://jrsoftware.org/isdl.php

#ifndef MyAppName
  #define MyAppName "Bite"
#endif
#ifndef MyAppVersion
  #define MyAppVersion "0.0.0"
#endif
#ifndef MyAppPublisher
  #define MyAppPublisher "Chandan Singh"
#endif
#ifndef MyAppCopyright
  #define MyAppCopyright "Copyright (c) 2026 Chandan Singh"
#endif
#ifndef MyAppExe
  #define MyAppExe "bite-gui.exe"
#endif
#ifndef MyAppCliExe
  #define MyAppCliExe "bite.exe"
#endif
#ifndef MyAppProgId
  #define MyAppProgId "Bite.Workflow"
#endif
#ifndef MyAppTypeName
  #define MyAppTypeName "Bite Workflow"
#endif
#ifndef MyAppExtension
  #define MyAppExtension ".bite"
#endif
#ifndef MyAppUrl
  #define MyAppUrl "https://github.com/psmyles/bite"
#endif

[Setup]
; A stable GUID keeps upgrades and uninstall tied to the same app across versions. It is the
; one value here that must never change.
AppId={{7F3A9C41-5D82-4E17-9B6C-2A8E4D1F0C93}
AppName={#MyAppName}
AppVersion={#MyAppVersion}
AppPublisher={#MyAppPublisher}
AppPublisherURL={#MyAppUrl}
AppSupportURL={#MyAppUrl}
AppUpdatesURL={#MyAppUrl}/releases
VersionInfoVersion={#MyAppVersion}
VersionInfoCopyright={#MyAppCopyright}
; Unelevated, {autopf} resolves to {localappdata}\Programs - the same per-user location the
; previous installer used, which is where the Unity integration (unity-tools\BiteBatchTool.cs)
; looks for bite.exe. An elevated install lands in Program Files instead.
DefaultDirName={autopf}\{#MyAppName}
DefaultGroupName={#MyAppName}
DisableProgramGroupPage=yes
; Let the user pick all-users (admin) or just-me (no elevation) at install time.
PrivilegesRequired=lowest
PrivilegesRequiredOverridesAllowed=dialog
AllowNoIcons=yes
LicenseFile=..\LICENSE
OutputDir=..\dist
OutputBaseFilename={#MyAppName}-Windows-{#MyAppVersion}-Setup
SetupIconFile=..\build\icon.ico
UninstallDisplayIcon={app}\{#MyAppExe}
UninstallDisplayName={#MyAppName}
Compression=lzma2/max
SolidCompression=yes
WizardStyle=modern
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible

[Languages]
Name: "english"; MessagesFile: "compiler:Default.isl"

[Tasks]
Name: "associate"; GroupDescription: "File associations:"; \
  Description: "Associate {#MyAppExtension} workflow files with {#MyAppName}"
Name: "addtopath"; GroupDescription: "Command line:"; \
  Description: "Add {#MyAppName} to my PATH, so ""{#MyAppCliExe}"" runs from any terminal"
Name: "desktopicon"; GroupDescription: "Additional icons:"; \
  Description: "Create a desktop shortcut"; Flags: unchecked

[Files]
; The editor and the CLI sit in the same directory, which is how each finds the definitions and
; the bundled ImageMagick beside it (see Magick::discover and load_registry).
Source: "..\target\release\{#MyAppExe}"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\target\release\{#MyAppCliExe}"; DestDir: "{app}"; Flags: ignoreversion
; Node and format definitions ship as plain JSON so users can add their own.
Source: "..\node-definitions-v2\*.json"; DestDir: "{app}\node-definitions-v2"; Flags: ignoreversion
Source: "..\format-definitions-v2\*.json"; DestDir: "{app}\format-definitions-v2"; Flags: ignoreversion
; Bundled ImageMagick. Both binaries resolve <exe dir>\magick\magick.exe first, so no PATH
; install of ImageMagick is needed.
Source: "..\resources\win\magick\*"; DestDir: "{app}\magick"; \
  Flags: ignoreversion recursesubdirs createallsubdirs
; The ImageMagick license requires its notice to travel with the binary.
Source: "..\LICENSE"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\THIRD_PARTY_LICENSES"; DestDir: "{app}"; Flags: ignoreversion
; The user-facing documents only. docs\node-authoring-guide.md is deliberately not shipped: it
; still describes the v1 definition format and is queued for a rewrite.
Source: "..\docs\getting-started.md"; DestDir: "{app}\docs"; Flags: ignoreversion
Source: "..\docs\expression-language.md"; DestDir: "{app}\docs"; Flags: ignoreversion
; Example workflows, so a new install has something to open.
Source: "..\examples\*.bite"; DestDir: "{app}\examples"; Flags: ignoreversion skipifsourcedoesntexist

[Icons]
Name: "{group}\{#MyAppName}"; Filename: "{app}\{#MyAppExe}"
Name: "{group}\Uninstall {#MyAppName}"; Filename: "{uninstallexe}"
Name: "{autodesktop}\{#MyAppName}"; Filename: "{app}\{#MyAppExe}"; Tasks: desktopicon

[Registry]
; Register a ProgID and add it to the .bite Open-With list rather than seizing the extension.
; HKA resolves to HKLM (all-users) or HKCU (just-me) to match the chosen install scope.
Root: HKA; Subkey: "Software\Classes\{#MyAppProgId}"; ValueType: string; ValueName: ""; \
  ValueData: "{#MyAppTypeName}"; Flags: uninsdeletekey; Tasks: associate
Root: HKA; Subkey: "Software\Classes\{#MyAppProgId}\DefaultIcon"; ValueType: string; ValueName: ""; \
  ValueData: "{app}\{#MyAppExe},0"; Tasks: associate
Root: HKA; Subkey: "Software\Classes\{#MyAppProgId}\shell\open\command"; ValueType: string; ValueName: ""; \
  ValueData: """{app}\{#MyAppExe}"" ""%1"""; Tasks: associate
Root: HKA; Subkey: "Software\Classes\{#MyAppExtension}\OpenWithProgIds"; ValueType: string; \
  ValueName: "{#MyAppProgId}"; ValueData: ""; Flags: uninsdeletevalue; Tasks: associate

[Run]
Filename: "{app}\{#MyAppExe}"; Description: "Launch {#MyAppName}"; \
  Flags: nowait postinstall skipifsilent

[Code]
// The PATH entry, so `bite run workflow.bite` works from any terminal - what the previous
// installer's custom NSIS macro did. Only the per-user PATH is touched: an all-users install
// would need the machine variable, and appending to that from a setup that may be running
// unelevated is not safe. The entry is removed on uninstall, which NSIS never did.

const
  EnvironmentKey = 'Environment';

function PathDirs(const Path: string): TArrayOfString;
var
  Remaining, Item: string;
  Count, Position: Integer;
begin
  SetArrayLength(Result, 0);
  Remaining := Path;
  Count := 0;
  while Remaining <> '' do
  begin
    Position := Pos(';', Remaining);
    if Position = 0 then
    begin
      Item := Remaining;
      Remaining := '';
    end
    else
    begin
      Item := Copy(Remaining, 1, Position - 1);
      Remaining := Copy(Remaining, Position + 1, Length(Remaining) - Position);
    end;
    Item := Trim(Item);
    if Item <> '' then
    begin
      SetArrayLength(Result, Count + 1);
      Result[Count] := Item;
      Count := Count + 1;
    end;
  end;
end;

// True when Directory is already listed in Path. Compared case-insensitively and without any
// trailing slash, so a second install does not add a duplicate entry.
function PathContains(const Path, Directory: string): Boolean;
var
  Dirs: TArrayOfString;
  Wanted, Item: string;
  I: Integer;
begin
  Result := False;
  Wanted := Lowercase(RemoveBackslashUnlessRoot(Directory));
  Dirs := PathDirs(Path);
  for I := 0 to GetArrayLength(Dirs) - 1 do
  begin
    Item := Lowercase(RemoveBackslashUnlessRoot(Dirs[I]));
    if Item = Wanted then
    begin
      Result := True;
      Exit;
    end;
  end;
end;

procedure AddToUserPath(const Directory: string);
var
  Path: string;
begin
  if not RegQueryStringValue(HKCU, EnvironmentKey, 'Path', Path) then
    Path := '';
  if PathContains(Path, Directory) then
    Exit;
  if (Path <> '') and (Copy(Path, Length(Path), 1) <> ';') then
    Path := Path + ';';
  // REG_EXPAND_SZ: an existing PATH commonly holds %USERPROFILE% and friends, and rewriting it
  // as a plain string would freeze those at today's values.
  RegWriteExpandStringValue(HKCU, EnvironmentKey, 'Path', Path + Directory);
end;

procedure RemoveFromUserPath(const Directory: string);
var
  Path, Rebuilt, Wanted, Item: string;
  Dirs: TArrayOfString;
  I: Integer;
begin
  if not RegQueryStringValue(HKCU, EnvironmentKey, 'Path', Path) then
    Exit;
  Wanted := Lowercase(RemoveBackslashUnlessRoot(Directory));
  Dirs := PathDirs(Path);
  Rebuilt := '';
  for I := 0 to GetArrayLength(Dirs) - 1 do
  begin
    Item := Dirs[I];
    if Lowercase(RemoveBackslashUnlessRoot(Item)) <> Wanted then
    begin
      if Rebuilt <> '' then
        Rebuilt := Rebuilt + ';';
      Rebuilt := Rebuilt + Item;
    end;
  end;
  if Rebuilt = Path then
    Exit;
  if Rebuilt = '' then
    RegDeleteValue(HKCU, EnvironmentKey, 'Path')
  else
    RegWriteExpandStringValue(HKCU, EnvironmentKey, 'Path', Rebuilt);
end;

procedure CurStepChanged(CurStep: TSetupStep);
begin
  // ssPostInstall, so the install directory exists and the task selection is final. Inno
  // broadcasts WM_SETTINGCHANGE itself when the wizard closes, so open Explorer windows and
  // new terminals pick the change up without a sign-out.
  if (CurStep = ssPostInstall) and WizardIsTaskSelected('addtopath') then
    AddToUserPath(ExpandConstant('{app}'));
end;

procedure CurUninstallStepChanged(CurUninstallStep: TUninstallStep);
begin
  if CurUninstallStep = usUninstall then
    RemoveFromUserPath(ExpandConstant('{app}'));
end;
