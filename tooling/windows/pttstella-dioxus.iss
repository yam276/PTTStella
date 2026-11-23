#ifndef AppVersion
  #error AppVersion is required
#endif
#ifndef BootstrapperSHA256
  #error A verified bootstrapper hash is required
#endif

[Setup]
AppId=io.github.yam276.pttstella
AppName=PTTStella
AppVersion={#AppVersion}
AppPublisher=yam276
AppPublisherURL=https://github.com/yam276/PTTStella
DefaultDirName={localappdata}\Programs\PTTStella
PrivilegesRequired=lowest
SetupArchitecture=x64
ArchitecturesAllowed=x64compatible and not arm64
ArchitecturesInstallIn64BitMode=x64compatible
MinVersion=10.0
OutputDir={#OutputDir}
OutputBaseFilename={#OutputName}
SetupIconFile={#PackageDir}\PTTStella.ico
UninstallDisplayIcon={app}\pttstella.exe
VersionInfoVersion={#NumericVersion}
Compression=lzma2
SolidCompression=yes
WizardStyle=modern
DisableProgramGroupPage=yes
CloseApplications=yes
RestartApplications=no
SetupMutex=PTTStellaSetup

[Languages]
Name: "english"; MessagesFile: "compiler:Default.isl,{#SourcePath}\installer.en.isl"
Name: "chinesetraditional"; MessagesFile: "compiler:Languages\ChineseTraditional.isl,{#SourcePath}\installer.zh-TW.isl"

[Files]
Source: "{#BootstrapperPath}"; DestName: "MicrosoftEdgeWebview2Setup.exe"; Flags: dontcopy
Source: "{#PackageDir}\*"; DestDir: "{app}"; Flags: ignoreversion recursesubdirs createallsubdirs

[Icons]
Name: "{userprograms}\PTTStella"; Filename: "{app}\pttstella.exe"

[Code]
function ReadWebViewVersion(const RootKey: HKEY; var Value: String): Boolean;
begin
  Result := RegQueryStringValue(RootKey,
    'Software\Microsoft\EdgeUpdate\Clients\{F3017226-FE2A-4295-8BDF-00C3A9A7E4C5}', 'pv', Value);
end;

function RunWebViewBootstrapper(var ResultCode: Integer): Boolean;
var
  Path: String;
begin
  WizardForm.PreparingLabel.Caption := CustomMessage('WebViewInstalling');
  ExtractTemporaryFile('MicrosoftEdgeWebview2Setup.exe');
  Path := ExpandConstant('{tmp}\MicrosoftEdgeWebview2Setup.exe');
  if CompareText(GetSHA256OfFile(Path), '{#BootstrapperSHA256}') <> 0 then
    RaiseException(CustomMessage('WebViewHashFailed'));
  Result := Exec(Path, '/silent /install', ExpandConstant('{tmp}'),
    SW_HIDE, ewWaitUntilTerminated, ResultCode);
end;

#include "webview2.iss"

function PrepareToInstall(var NeedsRestart: Boolean): String;
begin
  Result := EnsureWebViewRuntime(NeedsRestart);
end;

function UpdateReadyMemo(Space, NewLine, MemoUserInfoInfo, MemoDirInfoInfo,
  MemoTypeInfo, MemoComponentsInfo, MemoGroupInfo, MemoTasksInfo: String): String;
begin
  Result := MemoDirInfoInfo;
  if not HasWebViewRuntime then
    Result := Result + NewLine + NewLine + CustomMessage('WebViewRequired');
end;
